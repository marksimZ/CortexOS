//! Aider CLI gate only. Python 3.12 tools remain separate from Python 3.13 scoring.
use super::*;
pub(crate) const VERSION: &str = "0.86.2";
pub(super) const IMAGE: &str = "sha256:82b98ea28b075414873b52a67228fd9c0cb2b25ee564db0a3d7d673699f6cc3e";
const LOCK_HASH: &str = "c956f9f95672c9303a7a0ed23aaa18822d2762f62435b5d0efe7d38f26cdd1c6";
const PROBE: &str = include_str!("../../../../../../scripts/harness/aider_probe.py");
const HOME: &str = "rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534";
const COPY: &str = "rw,noexec,nosuid,nodev,size=2m,mode=0700,uid=65534,gid=65534";
pub(crate) fn definition() -> Value {
    json!({"version":VERSION,"imageId":IMAGE,"lockHash":LOCK_HASH,"python":"3.12","probeVersion":"aider-program-1","probeHash":crate::benchmark::dataset::hash(PROBE.as_bytes()),"wheelHash":"64f6a0c66c9f4633ad9f479bca3e64ebcba02b9da03c6b604b74a44736b2416e","source":"https://pypi.org/pypi/aider-chat/0.86.2/json"})
}
pub(crate) fn security_policy() -> Value {
    json!({"id":"docker-default","scope":"aider_runtime_only","innerSandbox":"not_verified"})
}
fn metadata(endpoint: &str, image: &str) -> Result<Value, String> {
    if !valid_image(image) || image != IMAGE {
        return Err("invalid_container_image".into());
    }
    let v = docker_json(command(
        &args(&[
            "--host",
            endpoint,
            "image",
            "inspect",
            image,
            "--format",
            "{{json .}}",
        ]),
        None,
        Duration::from_secs(5),
        None,
    )?)?;
    if v["Id"] != image
        || v["Os"] != "linux"
        || v["Architecture"] != "amd64"
        || v["Config"]["Labels"]["com.cortexos.harness"] != "aider"
        || v["Config"]["Labels"]["com.cortexos.aider.version"] != VERSION
        || v["Config"]["Labels"]["com.cortexos.runtime.policy"] != "aider-runtime-1"
        || v["Config"]["Volumes"]
            .as_object()
            .is_some_and(|m| !m.is_empty())
        || !v["Config"]["Env"].as_array().is_some_and(|a| {
            a.iter().any(|e| {
                e.as_str()
                    .is_some_and(|s| s.starts_with("PYTHON_VERSION=3.12."))
            })
        })
    {
        return Err("aider_runtime_image_mismatch".into());
    }
    Ok(v)
}
pub(crate) fn environment_for(
    image: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    let env = harness_environment(image, Some(cancel))?;
    metadata(
        env["endpoint"].as_str().ok_or("local_docker_required")?,
        image,
    )?;
    Ok(env)
}
pub(crate) fn images() -> Result<Value, String> {
    let endpoint = docker_json(command(
        &args(&[
            "context",
            "inspect",
            "--format",
            "{{json .Endpoints.docker.Host}}",
        ]),
        None,
        Duration::from_secs(5),
        None,
    )?)?;
    let endpoint = endpoint
        .as_str()
        .filter(|s| s.starts_with("npipe://") || s.starts_with("unix://"))
        .ok_or("local_docker_required")?;
    let r = command(
        &args(&[
            "--host",
            endpoint,
            "image",
            "ls",
            "--filter",
            "label=com.cortexos.runtime.policy=aider-runtime-1",
            "--no-trunc",
            "--format",
            "{{.ID}}",
        ]),
        None,
        Duration::from_secs(5),
        None,
    )?;
    if r.status != 0 || r.termination.is_some() {
        return Err("docker_unavailable".into());
    }
    let ids = String::from_utf8_lossy(&r.output);
    let mut images = vec![];
    for id in ids
        .lines()
        .filter(|s| valid_image(s))
        .take(16)
        .collect::<std::collections::BTreeSet<_>>()
    {
        if metadata(endpoint, id).is_ok() {
            images.push(json!({"imageId":id,"version":VERSION}));
        }
    }
    Ok(json!({"images":images,"downloads":false}))
}
pub(crate) fn observe(
    endpoint: &str,
    image: &str,
    owner: &str,
    name: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    if uuid::Uuid::parse_str(owner).is_err()
        || name
            .strip_prefix("cortexos-bench-")
            .is_none_or(|s| uuid::Uuid::parse_str(s).is_err())
    {
        return Err("invalid_container_owner".into());
    }
    metadata(endpoint, image)?;
    let mut a = create_args(endpoint, image, name);
    let at = a
        .iter()
        .position(|s| s == image)
        .ok_or("invalid_container_image")?;
    a.splice(
        at..at,
        [
            "--label".into(),
            format!("com.cortexos.validation={owner}"),
            "--label=com.cortexos.purpose=harness_aider_runtime".into(),
            format!("--tmpfs=/workspace:{COPY}"),
            format!("--tmpfs=/home/cortexos:{HOME}"),
        ],
    );
    *a.last_mut().unwrap() = PROBE.into();
    let r = command(&a, None, Duration::from_secs(5), None)?;
    if r.status != 0 || r.termination.is_some() {
        return Err(r.termination.unwrap_or("container_create_failed".into()));
    }
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    let v = docker_json(command(
        &args(&[
            "--host",
            endpoint,
            "inspect",
            name,
            "--format",
            "{{json .}}",
        ]),
        None,
        Duration::from_secs(5),
        Some(cancel),
    )?)?;
    if !configuration_valid(&v)
        || !codex_policy::matches(&v, codex_policy::DEFAULT)
        || v["Config"]["Image"] != image
        || v["Config"]["Labels"]["com.cortexos.validation"] != owner
        || v["Config"]["Labels"]["com.cortexos.purpose"] != "harness_aider_runtime"
        || v["HostConfig"]["Tmpfs"]["/workspace"] != COPY
        || v["HostConfig"]["Tmpfs"]["/home/cortexos"] != HOME
        || v["Config"]["Cmd"] != json!(["-I", "-S", "-u", "-c", PROBE])
    {
        return Err("container_limits_not_confirmed".into());
    }
    let r = command(
        &args(&[
            "--host",
            endpoint,
            "start",
            "--attach",
            "--interactive",
            name,
        ]),
        None,
        Duration::from_secs(30),
        Some(cancel),
    )?;
    if let Some(reason) = r.termination {
        return Err(reason);
    }
    if r.status != 0 {
        return Err("aider_probe_failed".into());
    }
    let raw = evaluation_json::decode(
        std::str::from_utf8(&r.output).map_err(|_| "aider_probe_invalid_response")?,
    )
    .map_err(|_| "aider_probe_invalid_response")?;
    let checks = classify(&raw)?;
    Ok(
        json!({"checks":checks,"raw":raw,"definition":definition(),"imageId":image,"verifiedConfiguration":{"user":v["Config"]["User"],"hostConfig":v["HostConfig"],"mounts":v["Mounts"]},"policy":{"container":policy(),"checkWallSeconds":30,"commandWallSeconds":8,"securityPolicy":security_policy()},"modelCalls":0,"taskExecution":"Not Run","canRun":false}),
    )
}
fn classify(raw: &Value) -> Result<Vec<Value>, String> {
    if raw["protocol"] != 1
        || raw["modelCalls"] != 0
        || raw["lockHash"] != LOCK_HASH
        || raw["taskExecution"] != "Not Run"
    {
        return Err("aider_probe_invalid_response".into());
    }
    let version = raw["packageVersion"] == VERSION
        && raw["python"] == json!([3, 12])
        && raw["version"]["exitCode"] == 0
        && raw["version"]["stdout"]
            .as_str()
            .is_some_and(|s| s.trim() == format!("__main__.py {VERSION}"));
    let help = raw["help"]["stdout"].as_str().unwrap_or("");
    let options = [
        "--message",
        "--no-git",
        "--no-auto-commits",
        "--no-auto-lint",
        "--no-auto-test",
        "--no-analytics",
        "--no-check-update",
        "--no-restore-chat-history",
        "--config",
        "--env-file",
    ];
    let missing: Vec<_> = options.into_iter().filter(|s| !help.contains(s)).collect();
    let isolation = [
        "nonRoot",
        "noUsableCapabilities",
        "noNewPrivileges",
        "seccompActive",
        "readonlyRoot",
        "networkUnavailable",
        "noDockerSocket",
    ]
    .iter()
    .all(|k| raw["outerChecks"][k] == true);
    Ok(vec![
        json!({"id":"version","state":if version{"passed"}else{"blocked"}}),
        json!({"id":"automation","state":if raw["help"]["exitCode"]==0&&missing.is_empty(){"passed"}else{"blocked"},"missing":missing}),
        json!({"id":"container","state":if isolation{"passed"}else{"blocked"}}),
        json!({"id":"inner_sandbox","state":"not_checked","reason":"no_inner_sandbox_evidence"}),
        json!({"id":"model_channel","state":"not_checked","reason":"not_implemented_in_this_slice"}),
    ])
}
