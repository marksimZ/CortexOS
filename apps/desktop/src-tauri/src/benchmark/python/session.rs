//! Fixed Codex session IO using the original Docker command/limits and ownership.
use super::*;
use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
const IDLE: &str = "import time; time.sleep(600)";
const IO: &str = include_str!("workspace_io.py");
const RELAY: &str = include_str!("codex_session.py");
const AIDER_RELAY: &str = include_str!("aider_session.py");
const CLAUDE_RELAY: &str = include_str!("claude_session.py");
const COPY: &str = "rw,noexec,nosuid,nodev,size=2m,mode=0755,uid=0,gid=0";
const HOME: &str = "rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534";
pub(crate) use super::workspace::CopyRef;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Codex,
    Claude,
    Aider,
}
impl Kind {
    pub(crate) fn purpose(self) -> &'static str {
        match self {
            Self::Codex => "harness_codex_session",
            Self::Claude => "harness_claude_session",
            Self::Aider => "harness_aider_session",
        }
    }
    fn directory_policy(self) -> &'static str {
        match self {
            Self::Codex => "codex-protected-empty-1",
            Self::Claude => "claude-protected-empty-1",
            Self::Aider => "aider-single-file-1",
        }
    }

    fn relay(self) -> &'static str {
        match self {
            Self::Codex => RELAY,
            Self::Claude => CLAUDE_RELAY,
            Self::Aider => AIDER_RELAY,
        }
    }
    pub(crate) fn policy(self) -> Result<Value, String> {
        if self == Self::Codex {
            return codex_policy::describe(codex_policy::COMPATIBLE);
        }
        if self == Self::Aider {
            return Ok(
                json!({"id":"docker-default","scope":"aider_session_only","innerSandbox":"not_verified"}),
            );
        }
        let mut value = claude_policy::describe(claude_policy::COMPATIBLE)?;
        value["scope"] = json!("claude_session_only");
        Ok(value)
    }
}
pub(crate) fn descriptor_for(kind: Kind) -> Value {
    if kind == Kind::Aider {
        return json!({"version":"aider-stdio-text-1","relayHash":crate::benchmark::dataset::hash(AIDER_RELAY.as_bytes()),"fileIoHash":crate::benchmark::dataset::hash(IO.as_bytes()),"bridgeHash":crate::benchmark::dataset::hash(include_str!("../harness/aider_channel.rs").as_bytes()),"workflowHash":crate::benchmark::dataset::hash(include_str!("../harness/session.rs").as_bytes()),"directoryPolicy":kind.directory_policy(),"fileSizeBytes":1048576,"modelTools":[],"scope":kind.purpose()});
    }
    if kind == Kind::Claude {
        return json!({"version":"claude-stdio-1","relayHash":crate::benchmark::dataset::hash(CLAUDE_RELAY.as_bytes()),"fileIoHash":crate::benchmark::dataset::hash(IO.as_bytes()),"bridgeHash":crate::benchmark::dataset::hash(include_str!("../harness/session/anthropic.rs").as_bytes()),"workflowHash":crate::benchmark::dataset::hash(include_str!("../harness/session.rs").as_bytes()),"directoryPolicy":kind.directory_policy(),"fileSizeBytes":1048576,"modelTools":["Bash"],"scope":"harness_claude_session_only"});
    }
    json!({"version":"codex-stdio-text-2","relayHash":crate::benchmark::dataset::hash(RELAY.as_bytes()),"fileIoHash":crate::benchmark::dataset::hash(IO.as_bytes()),"bridgeHash":crate::benchmark::dataset::hash(include_str!("../harness/session/bridge.rs").as_bytes()),"workflowHash":crate::benchmark::dataset::hash(include_str!("../harness/session.rs").as_bytes()),"directoryPolicy":"codex-protected-empty-1","fileSizeBytes":4194304,"modelTools":["exec_command","write_stdin"],"scope":"harness_codex_session_only"})
}
fn checked(r: &CopyRef<'_>, kind: Kind) -> Result<Value, String> {
    if uuid::Uuid::parse_str(r.owner).is_err()
        || !valid_image(r.image)
        || r.name
            .strip_prefix("cortexos-bench-")
            .is_none_or(|n| uuid::Uuid::parse_str(n).is_err())
    {
        return Err("invalid_container_owner".into());
    }
    let v = docker_json(command(
        &args(&[
            "--host",
            r.endpoint,
            "inspect",
            r.name,
            "--format",
            "{{json .}}",
        ]),
        None,
        Duration::from_secs(5),
        None,
    )?)?;
    if !configuration_valid(&v)
        || !(if kind == Kind::Codex {
            codex_policy::matches(&v, codex_policy::COMPATIBLE)
        } else if kind == Kind::Claude {
            claude_policy::matches(&v, claude_policy::COMPATIBLE)
        } else {
            codex_policy::matches(&v, codex_policy::DEFAULT) && r.image == aider::IMAGE
        })
        || v["Config"]["Image"] != r.image
        || v["Config"]["Labels"]["com.cortexos.validation"] != r.owner
        || v["Config"]["Labels"]["com.cortexos.purpose"] != kind.purpose()
        || v["HostConfig"]["Tmpfs"]["/workspace"] != COPY
        || v["HostConfig"]["Tmpfs"]["/home/cortexos"] != HOME
        || !v["HostConfig"]["Ulimits"].as_array().is_some_and(|a| {
            a.iter().any(|u| {
                u["Name"] == "fsize"
                    && u["Soft"]
                        == if kind == Kind::Codex {
                            4194304
                        } else {
                            1048576
                        }
                    && u["Hard"]
                        == if kind == Kind::Codex {
                            4194304
                        } else {
                            1048576
                        }
            })
        })
        || v["Config"]["Cmd"] != json!(["-I", "-S", "-u", "-c", IDLE])
    {
        return Err("container_limits_not_confirmed".into());
    }
    Ok(v)
}
pub(crate) fn open_for(
    r: &CopyRef<'_>,
    bundle: &Value,
    cancel: &watch::Receiver<bool>,
    kind: Kind,
) -> Result<Value, String> {
    let mut a = create_args(r.endpoint, r.image, r.name);
    // Codex's own SQLite WAL exceeds 1 MiB during startup. Keep the same tmpfs
    // quota; only this session's per-file cap may use up to its 4 MiB home quota.
    let file_limit = a
        .iter()
        .position(|s| s == "--ulimit=fsize=1048576:1048576")
        .ok_or("container_limits_not_confirmed")?;
    if kind == Kind::Codex {
        a[file_limit] = "--ulimit=fsize=4194304:4194304".into();
    }
    let profile = match kind {
        Kind::Codex => codex_policy::ProfileFile::create(codex_policy::COMPATIBLE, r.owner)?,
        Kind::Claude => claude_policy::create(claude_policy::COMPATIBLE, r.owner)?,
        Kind::Aider => None,
    };
    if kind != Kind::Aider && profile.is_none() {
        return Err("policy_missing".into());
    }
    let at = a
        .iter()
        .position(|s| s == r.image)
        .ok_or("invalid_container_image")?;
    a.splice(
        at..at,
        [
            "--label".into(),
            format!("com.cortexos.validation={}", r.owner),
            format!("--label=com.cortexos.purpose={}", kind.purpose()),
            format!("--tmpfs=/workspace:{COPY}"),
            format!("--tmpfs=/home/cortexos:{HOME}"),
        ],
    );
    if let Some(profile) = &profile {
        a.insert(at, profile.argument());
    }
    *a.last_mut().unwrap() = IDLE.into();
    let reply = command(&a, None, Duration::from_secs(5), None)?;
    drop(profile);
    if reply.status != 0 || reply.termination.is_some() {
        return Err("container_create_failed".into());
    }
    let v = checked(r, kind)?;
    let started = command(
        &args(&["--host", r.endpoint, "start", r.name]),
        None,
        Duration::from_secs(5),
        Some(cancel),
    )?;
    if started.status != 0
        || started.termination.is_some()
        || checked(r, kind)?["State"]["Running"] != true
    {
        return Err("container_start_failed".into());
    }
    let observed = io(r, json!({"op":"init","bundle":bundle}), true, cancel, kind)?;
    Ok(
        json!({"observed":observed,"configuration":{"user":v["Config"]["User"],"hostConfig":v["HostConfig"],"mounts":v["Mounts"]}}),
    )
}
fn io(
    r: &CopyRef<'_>,
    mut payload: Value,
    root: bool,
    cancel: &watch::Receiver<bool>,
    kind: Kind,
) -> Result<Value, String> {
    payload["directoryPolicy"] = json!(kind.directory_policy());
    if checked(r, kind)?["State"]["Running"] != true {
        return Err("session_expired".into());
    }
    let bytes = serde_json::to_vec(&payload).map_err(|_| "workspace_encoding")?;
    if bytes.len() > 131072 {
        return Err("workspace_payload_limit".into());
    }
    let reply = command(
        &args(&[
            "--host",
            r.endpoint,
            "exec",
            "-i",
            "--user",
            if root { "0:0" } else { "65534:65534" },
            r.name,
            "python3",
            "-I",
            "-S",
            "-c",
            IO,
        ]),
        Some(&bytes),
        Duration::from_secs(5),
        Some(cancel),
    )?;
    if let Some(reason) = reply.termination {
        return Err(reason);
    }
    let v: Value =
        serde_json::from_slice(&reply.output).map_err(|_| "workspace_invalid_response")?;
    if reply.status != 0 || v.get("error").is_some() {
        return Err(v["error"]
            .as_str()
            .unwrap_or("workspace_file_operation_failed")
            .into());
    }
    Ok(v)
}
pub(crate) fn collect_for(
    r: &CopyRef<'_>,
    cancel: &watch::Receiver<bool>,
    kind: Kind,
) -> Result<Value, String> {
    io(r, json!({"op":"collect"}), false, cancel, kind)
}

async fn line(
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<Option<String>, String> {
    let mut result = Vec::new();
    loop {
        let available = reader.fill_buf().await.map_err(|_| "session_read_failed")?;
        if available.is_empty() {
            return if result.is_empty() {
                Ok(None)
            } else {
                Err("session_truncated_line".into())
            };
        }
        let count = available
            .iter()
            .position(|b| *b == b'\n')
            .map(|n| n + 1)
            .unwrap_or(available.len());
        if result.len() + count > 262144 {
            return Err("session_line_limit".into());
        }
        let done = available[count - 1] == b'\n';
        result.extend_from_slice(&available[..count]);
        reader.consume(count);
        if done {
            return String::from_utf8(result)
                .map(Some)
                .map_err(|_| "session_invalid_utf8".into());
        }
    }
}
/// No caller-supplied program/arguments/endpoint. Callback sees only bounded relay frames.
pub(crate) async fn execute_for<F, Fut>(
    r: &CopyRef<'_>,
    model: &str,
    prompt: &str,
    mut cancel: watch::Receiver<bool>,
    mut event: F,
    kind: Kind,
) -> Result<(), String>
where
    F: FnMut(Value) -> Fut,
    Fut: Future<Output = Result<Option<Value>, String>>,
{
    checked(r, kind)?;
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args(args(&[
        "--host",
        r.endpoint,
        "exec",
        "-i",
        "--user",
        "65534:65534",
        r.name,
        "python3",
        "-I",
        "-S",
        "-u",
        "-c",
        kind.relay(),
    ]))
    .env_remove("DOCKER_HOST")
    .env_remove("DOCKER_CONTEXT")
    .env_remove("DOCKER_TLS_VERIFY")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = cmd.spawn().map_err(|_| "docker_unavailable")?;
    let mut stdin = child.stdin.take().ok_or("docker_stdin")?;
    let mut stdout = BufReader::new(child.stdout.take().ok_or("docker_stdout")?);
    let stderr = child.stderr.take().ok_or("docker_stderr")?;
    let mut error = tokio::spawn(async move {
        let mut bytes = vec![];
        let _ = stderr.take(65537).read_to_end(&mut bytes).await;
        bytes
    });
    let initially_cancelled = *cancel.borrow();
    let result = tokio::select! {
        _=cancel.changed()=>Err("cancelled".to_string()),
        _=tokio::time::sleep(Duration::from_secs(180))=>Err("wall_timeout".to_string()),
        result=async {
            if initially_cancelled{return Err("cancelled".into());}
            let input=format!("{}\n",json!({"model":model,"prompt":prompt}));
            if input.len()>65536{return Err("session_payload_limit".into());}
            stdin.write_all(input.as_bytes()).await.map_err(|_|"session_write_failed")?;
            let mut total=0;let mut count=0;let mut exit=false;
            while let Some(raw)=line(&mut stdout).await? {
                total+=raw.len();count+=1;if total>1048576 || count>256{return Err("session_output_limit".into());}
                let frame=evaluation_json::decode(&raw).map_err(|_|"session_invalid_frame")?;
                if frame["kind"]=="exit" {exit=frame["exitCode"]==0;}
                if let Some(reply)=event(frame).await? {
                    let raw=format!("{reply}\n");if raw.len()>196608{return Err("session_reply_limit".into());}
                    stdin.write_all(raw.as_bytes()).await.map_err(|_|"session_write_failed")?;
                }
            }
            let status=child.wait().await.map_err(|_|"session_wait_failed")?;
            if !status.success() || !exit{return Err(match kind {Kind::Codex=>"codex_task_failed",Kind::Claude=>"claude_task_failed",Kind::Aider=>"aider_task_failed"}.into());}
            Ok(())
        }=>result
    };
    if result.is_err() {
        let _ = child.kill().await;
    }
    match tokio::time::timeout(Duration::from_secs(2), &mut error).await {
        Ok(Ok(bytes)) if !bytes.is_empty() => {
            event(json!({"kind":"diagnostic","source":"docker","raw":String::from_utf8_lossy(&bytes[..bytes.len().min(65536)])})).await?;
            if bytes.len() > 65536 {
                return Err("session_output_limit".into());
            }
        }
        Err(_) => error.abort(),
        _ => {}
    }
    result
}
