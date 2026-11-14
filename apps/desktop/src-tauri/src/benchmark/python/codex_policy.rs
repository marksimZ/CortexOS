//! Only the reviewed, fixed Codex policy; no caller-supplied profile or path.
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};
pub(crate) const DEFAULT: &str = "docker-default";
pub(crate) const COMPATIBLE: &str = "codex-bwrap-1";
const PROFILE: &str = include_str!("../../../catalogs/codex-seccomp/profile-v1.json");
const HASH: &str = "46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13";
pub(crate) fn describe(id: &str) -> Result<Value, String> {
    match id {
        DEFAULT => Ok(
            json!({"id":DEFAULT,"hash":null,"source":"docker_engine_default","scope":"codex_runtime_only"}),
        ),
        COMPATIBLE if crate::benchmark::dataset::hash(PROFILE.as_bytes()) == HASH => Ok(
            json!({"id":COMPATIBLE,"hash":HASH,"sourceCommit":"4b8214fc4971e06172938a1f1554ccff6647ee80","scope":"codex_runtime_only","extraSyscalls":["unshare","clone","mount","umount2","pivot_root","chroot"]}),
        ),
        _ => Err("未知或指纹不匹配的Codex保护策略，已拒绝检查。".into()),
    }
}
pub(crate) struct ProfileFile(PathBuf);
impl ProfileFile {
    pub(crate) fn create(id: &str, owner: &str) -> Result<Option<Self>, String> {
        describe(id)?;
        if id == DEFAULT {
            return Ok(None);
        }
        Self::create_fixed(owner, "codex", PROFILE, HASH).map(Some)
    }
    // Internal fixed callers only. Public Codex IDs remain limited by describe().
    pub(crate) fn create_fixed(
        owner: &str,
        scope: &str,
        profile: &str,
        digest: &str,
    ) -> Result<Self, String> {
        if !matches!(scope, "codex" | "claude")
            || crate::benchmark::dataset::hash(profile.as_bytes()) != digest
        {
            return Err("保护策略范围或指纹不匹配。".into());
        }
        let root = std::env::temp_dir()
            .canonicalize()
            .map_err(|_| "策略临时目录不可用。")?;
        if uuid::Uuid::parse_str(owner).is_err() {
            return Err("策略文件归属无效。".into());
        }
        let path = root.join(format!("cortexos-{scope}-seccomp-{owner}.json"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "无法创建本次策略文件。")?;
        let guard = Self(path);
        file.write_all(profile.as_bytes())
            .map_err(|_| "无法写入本次策略文件。")?;
        Ok(guard)
    }
    pub(crate) fn argument(&self) -> String {
        format!("--security-opt=seccomp={}", self.0.display())
    }
}
pub(crate) fn cleanup_file(owner: &str) -> Result<(), String> {
    cleanup_fixed_file(owner, "codex", HASH)
}
pub(crate) fn cleanup_fixed_file(owner: &str, scope: &str, digest: &str) -> Result<(), String> {
    if !matches!(scope, "codex" | "claude") {
        return Err("保护策略范围不匹配。".into());
    }
    if uuid::Uuid::parse_str(owner).is_err() {
        return Err("策略文件归属无效。".into());
    }
    let root = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| "策略临时目录不可用。")?;
    let path = root.join(format!("cortexos-{scope}-seccomp-{owner}.json"));
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("无法核对本次策略临时文件。".into()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65536 {
        return Err("策略临时文件类型不匹配，未删除。".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("策略临时文件重定向，未删除。".into());
        }
    }
    let bytes = std::fs::read(&path).map_err(|_| "无法读取本次策略临时文件。")?;
    if crate::benchmark::dataset::hash(&bytes) != digest {
        return Err("策略临时文件指纹不匹配，未删除。".into());
    }
    std::fs::remove_file(path).map_err(|_| "本次策略临时文件尚未清理。".into())
}
impl Drop for ProfileFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub(crate) fn matches(config: &Value, id: &str) -> bool {
    if describe(id).is_err() {
        return false;
    }
    let Some(options) = config["HostConfig"]["SecurityOpt"].as_array() else {
        return false;
    };
    let actual: Vec<_> = options
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|v| v.strip_prefix("seccomp="))
        .collect();
    if id == DEFAULT {
        return actual.is_empty();
    }
    actual.len() == 1
        && serde_json::from_str::<Value>(actual[0]).ok()
            == serde_json::from_str::<Value>(PROFILE).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_policy_is_fixed_and_rejects_disabled_or_modified_filters() {
        assert!(describe(COMPATIBLE).is_ok());
        assert!(describe("unconfined").is_err());
        let mut c = json!({"HostConfig":{"SecurityOpt":["no-new-privileges"]}});
        assert!(matches(&c, DEFAULT));
        assert!(!matches(&c, COMPATIBLE));
        let owner = crate::models::new_id();
        let temporary = ProfileFile::create(COMPATIBLE, &owner).unwrap().unwrap();
        assert!(temporary.0.exists());
        cleanup_file(&owner).unwrap();
        assert!(!temporary.0.exists());
        cleanup_file(&owner).unwrap();
        drop(temporary);
        c["HostConfig"]["SecurityOpt"] = json!(["no-new-privileges", format!("seccomp={PROFILE}")]);
        assert!(matches(&c, COMPATIBLE));
        assert!(!matches(&c, DEFAULT));
        c["HostConfig"]["SecurityOpt"] = json!(["no-new-privileges", "seccomp=unconfined"]);
        assert!(!matches(&c, COMPATIBLE));
        assert!(!matches(&c, DEFAULT));
        let mut changed: Value = serde_json::from_str(PROFILE).unwrap();
        changed["defaultAction"] = json!("SCMP_ACT_ALLOW");
        c["HostConfig"]["SecurityOpt"] = json!([format!("seccomp={changed}")]);
        assert!(!matches(&c, COMPATIBLE));
    }
}
