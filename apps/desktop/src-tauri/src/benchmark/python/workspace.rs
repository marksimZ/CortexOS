//! Restricted task copy transport; shares the existing Docker controller.
use super::*;
const IO: &str = include_str!("workspace_io.py");
const IDLE: &str = "import time; time.sleep(600)";
const TEMP: &str = "rw,noexec,nosuid,nodev,size=2m,mode=0755,uid=0,gid=0";

pub(crate) struct CopyRef<'a> {
    pub endpoint: &'a str,
    pub image: &'a str,
    pub owner: &'a str,
    pub name: &'a str,
}
fn valid(r: &CopyRef<'_>) -> Result<(), String> {
    if !valid_image(r.image)
        || uuid::Uuid::parse_str(r.owner).is_err()
        || r.name
            .strip_prefix("cortexos-bench-")
            .is_none_or(|v| uuid::Uuid::parse_str(v).is_err())
        || !(r.endpoint.starts_with("npipe://") || r.endpoint.starts_with("unix://"))
    {
        return Err("invalid_container_owner".into());
    }
    Ok(())
}
fn checked(r: &CopyRef<'_>) -> Result<Value, String> {
    valid(r)?;
    let config = docker_json(command(
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
    if !configuration_valid(&config)
        || config["Config"]["Labels"]["com.cortexos.validation"] != r.owner
        || config["Config"]["Labels"]["com.cortexos.purpose"] != "harness_workspace"
        || config["Config"]["Image"] != r.image
        || config["HostConfig"]["Tmpfs"]["/workspace"] != TEMP
        || config["Config"]["Cmd"] != json!(["-I", "-S", "-u", "-c", IDLE])
    {
        return Err("container_limits_not_confirmed".into());
    }
    Ok(config)
}
fn io(
    r: &CopyRef<'_>,
    payload: Value,
    root: bool,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    let config = checked(r)?;
    if config["State"]["Running"] != true {
        return Err("workspace_expired".into());
    }
    let input = serde_json::to_vec(&payload).map_err(|_| "workspace_encoding")?;
    if input.len() > 131072 {
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
        Some(&input),
        Duration::from_secs(5),
        Some(cancel),
    )?;
    if let Some(reason) = reply.termination {
        return Err(if reason == "cancelled" {
            reason
        } else {
            format!(
                "workspace_{}_{}",
                if root { "init" } else { "copy" },
                reason
            )
        });
    }
    let value: Value =
        serde_json::from_slice(&reply.output).map_err(|_| "workspace_invalid_response")?;
    if reply.status != 0 || value.get("error").is_some() {
        return Err(value["error"]
            .as_str()
            .unwrap_or("workspace_file_operation_failed")
            .to_string());
    }
    if value["protocol"] != 1 || value["python"] != json!([3, 13]) {
        return Err("workspace_invalid_response".into());
    }
    Ok(value)
}
pub(crate) fn open(
    r: &CopyRef<'_>,
    bundle: &Value,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    valid(r)?;
    let mut a = create_args(r.endpoint, r.image, r.name);
    let pos = a
        .iter()
        .position(|v| v == r.image)
        .ok_or("invalid_container_image")?;
    a.splice(
        pos..pos,
        [
            "--label".into(),
            format!("com.cortexos.validation={}", r.owner),
            "--label=com.cortexos.purpose=harness_workspace".into(),
            format!("--tmpfs=/workspace:{TEMP}"),
        ],
    );
    *a.last_mut().unwrap() = IDLE.into();
    // Wait for the bounded create reply before cancellation so its name is settled for cleanup.
    let created = command(&a, None, Duration::from_secs(5), None)?;
    if created.status != 0 {
        return Err(created
            .termination
            .map(|reason| format!("workspace_create_{reason}"))
            .unwrap_or("container_create_failed".into()));
    }
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    checked(r)?;
    let started = command(
        &args(&["--host", r.endpoint, "start", r.name]),
        None,
        Duration::from_secs(5),
        Some(cancel),
    )?;
    if started.status != 0 {
        return Err(started
            .termination
            .map(|reason| {
                if reason == "cancelled" {
                    reason
                } else {
                    format!("workspace_start_{reason}")
                }
            })
            .unwrap_or("container_start_failed".into()));
    }
    let observed = io(r, json!({"op":"init","bundle":bundle}), true, cancel)?;
    Ok(
        json!({"observed":observed,"policy":{"network":"none","readonlyRoot":true,"user":"65534:65534","cpu":1,"memoryBytes":268435456,"pids":64,"hostMounts":false,"dockerSocket":false,"workspaceTmpfsBytes":2097152,"lifetimeSeconds":600},"startedAt":crate::models::now()}),
    )
}
pub(crate) fn change(
    r: &CopyRef<'_>,
    code: &str,
    task_hash: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    io(
        r,
        json!({"op":"write","code":code,"taskHash":task_hash}),
        false,
        cancel,
    )
}
