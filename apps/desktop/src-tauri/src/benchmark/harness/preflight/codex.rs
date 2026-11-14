//! Fixed version probe only. No shell, login, task, user config or model invocation.
use crate::{benchmark::dataset::hash, models::new_id};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncReadExt;
const BINARY_LIMIT: u64 = 512 * 1024 * 1024;

fn local_file(path: &Path) -> bool {
    if !path.is_absolute() || path.to_string_lossy().starts_with("\\\\") {
        return false;
    }
    for parent in path.ancestors() {
        let Ok(meta) = std::fs::symlink_metadata(parent) else {
            return false;
        };
        if meta.file_type().is_symlink() {
            return false;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return false;
            }
        }
        if parent.join(".git").exists() {
            return false;
        }
    }
    path.is_file()
}
fn find(paths: &std::ffi::OsStr) -> (Option<PathBuf>, bool) {
    let mut wrapper = false;
    for dir in std::env::split_paths(paths).take(128) {
        if !dir.is_absolute() {
            continue;
        }
        let path = dir.join("codex.exe");
        if local_file(&path) {
            return (Some(path), wrapper);
        }
        wrapper |= ["codex.cmd", "codex.ps1"]
            .iter()
            .any(|name| local_file(&dir.join(name)));
    }
    (None, wrapper)
}
fn fingerprint(path: &Path) -> Result<String, &'static str> {
    let mut file = std::fs::File::open(path).map_err(|_| "cli_read_failed")?;
    if file.metadata().map_err(|_| "cli_read_failed")?.len() > BINARY_LIMIT {
        return Err("cli_too_large");
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|_| "cli_read_failed")?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > BINARY_LIMIT {
            return Err("cli_too_large");
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn parse_version(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let version = text.strip_prefix("codex-cli ")?;
    let core: Vec<_> = version.split(['-', '+']).next()?.split('.').collect();
    (version.len() <= 80
        && version
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-+".contains(&c))
        && core.len() == 3
        && core
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())))
    .then(|| version.to_string())
}
async fn bounded(stream: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::new();
    stream
        .take(4097)
        .read_to_end(&mut out)
        .await
        .map_err(|_| "cli_read_failed")?;
    if out.len() > 4096 {
        return Err("cli_output_limit");
    }
    Ok(out)
}
pub(super) async fn observe() -> Value {
    if !cfg!(windows) {
        return json!({"state":"unsupported","reason":"native_windows_probe_only"});
    }
    let (path, wrapper) = find(&std::env::var_os("PATH").unwrap_or_default());
    let Some(path) = path else {
        return json!({"state":if wrapper{"unknown"}else{"not_found"},"reason":if wrapper{"wrapper_not_executed"}else{"not_in_path"}});
    };
    let before = match fingerprint(&path) {
        Ok(h) => h,
        Err(e) => return json!({"state":"error","reason":e}),
    };
    let temp_root = match std::env::temp_dir().canonicalize() {
        Ok(p) => p,
        Err(_) => return json!({"state":"error","reason":"probe_temp_unavailable"}),
    };
    let scratch = temp_root.join(format!("cortexos-codex-version-{}", new_id()));
    if std::fs::create_dir(&scratch).is_err() {
        return json!({"state":"error","reason":"probe_temp_unavailable"});
    }
    let mut command = tokio::process::Command::new(&path);
    command
        .arg("--version")
        .env_clear()
        .current_dir(&scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for key in ["SystemRoot", "WINDIR"] {
        if let Some(v) = std::env::var_os(key) {
            command.env(key, v);
        }
    }
    for key in [
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "CODEX_HOME",
        "TEMP",
        "TMP",
    ] {
        command.env(key, &scratch);
    }
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut child = command.spawn().map_err(|_| "cli_start_failed")?;
        let stdout = child.stdout.take().ok_or("cli_output_unavailable")?;
        let stderr = child.stderr.take().ok_or("cli_output_unavailable")?;
        let (status, out, _) = tokio::try_join!(
            async { child.wait().await.map_err(|_| "cli_wait_failed") },
            bounded(stdout),
            bounded(stderr)
        )?;
        if !status.success() {
            return Err("cli_version_failed");
        }
        parse_version(&out).ok_or("cli_version_unrecognized")
    })
    .await
    .unwrap_or(Err("cli_version_timeout"));
    // Only the unpredictable directory created above is eligible for removal.
    let cleaned =
        scratch.parent() == Some(temp_root.as_path()) && std::fs::remove_dir_all(&scratch).is_ok();
    let unchanged = fingerprint(&path).is_ok_and(|after| after == before);
    match result {
        Ok(version) if unchanged && cleaned => {
            json!({"state":"detected","version":version,"path":path,"binaryHash":before,"command":["--version"],"config":"empty_temporary_home","cleanup":true,"scope":"host_only"})
        }
        Ok(_) => {
            json!({"state":"error","reason":if !unchanged{"cli_changed_during_probe"}else{"probe_cleanup_failed"},"cleanup":cleaned})
        }
        Err(reason) => {
            json!({"state":"error","reason":reason,"cleanup":cleaned,"outputStored":false,"pathHash":hash(path.to_string_lossy().as_bytes())})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_version_protocol_rejects_extra_output_and_relative_search() {
        assert_eq!(
            parse_version(b"codex-cli 0.123.0\r\n"),
            Some("0.123.0".into())
        );
        assert_eq!(
            parse_version(b"codex-cli 0.123.0-alpha.1"),
            Some("0.123.0-alpha.1".into())
        );
        for invalid in [
            "0.123.0",
            "codex-cli token=secret",
            "codex-cli 0.123.0\nsecret",
            "codex-cli 1",
            "codex-cli 1..2",
        ] {
            assert_eq!(parse_version(invalid.as_bytes()), None);
        }
        assert!(find(std::ffi::OsStr::new(".;relative")).0.is_none());
        assert!(!local_file(Path::new("codex.exe")));
        assert!(!local_file(Path::new("\\\\server\\share\\codex.exe")));
    }
}
