//! Fixed offline Codex checks through the existing bounded Docker controller.
use super::*;
pub(crate) const VERSION: &str = "0.166.0";
const BINARY: &str = "b8a11fedf24b2dfb59a70c9e52b030f9a508dcd49cd2dd0d41956f4c785156b8";
const PROBE: &str = include_str!("codex_probe.py");
const COPY: &str = "rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534";
const HOME: &str = "rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534";

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
    let listed = command(
        &args(&[
            "--host",
            endpoint,
            "image",
            "ls",
            "--filter",
            "label=com.cortexos.runtime.policy=codex-runtime-1",
            "--no-trunc",
            "--format",
            "{{.ID}}",
        ]),
        None,
        Duration::from_secs(5),
        None,
    )?;
    if listed.status != 0 || listed.termination.is_some() {
        return Err("docker_unavailable".into());
    }
    let ids: std::collections::BTreeSet<_> = String::from_utf8_lossy(&listed.output)
        .lines()
        .filter(|i| valid_image(i))
        .take(16)
        .map(str::to_owned)
        .collect();
    let mut found = vec![];
    for id in ids {
        if metadata(endpoint, &id).is_ok() {
            found.push(json!({"imageId":id,"version":VERSION}));
        }
    }
    Ok(json!({"images":found,"downloads":false,"modelCalls":0}))
}
fn metadata(endpoint: &str, image: &str) -> Result<Value, String> {
    if !valid_image(image) {
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
    let labels = &v["Config"]["Labels"];
    if v["Id"] != image
        || v["Os"] != "linux"
        || v["Architecture"] != "amd64"
        || labels["com.cortexos.harness"] != "codex"
        || labels["com.cortexos.codex.version"] != VERSION
        || labels["com.cortexos.runtime.policy"] != "codex-runtime-1"
        || v["Config"]["Volumes"]
            .as_object()
            .is_some_and(|o| !o.is_empty())
    {
        return Err("codex_runtime_image_mismatch".into());
    }
    Ok(v)
}
pub(crate) fn environment_for(
    image: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    let env = harness_environment(image, Some(cancel))?;
    metadata(env["endpoint"].as_str().unwrap(), image)?;
    Ok(env)
}
pub(crate) fn observe(
    endpoint: &str,
    image: &str,
    owner: &str,
    name: &str,
    policy_id: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    if uuid::Uuid::parse_str(owner).is_err()
        || name
            .strip_prefix("cortexos-bench-")
            .is_none_or(|v| uuid::Uuid::parse_str(v).is_err())
    {
        return Err("invalid_container_owner".into());
    }
    metadata(endpoint, image)?;
    let mut a = create_args(endpoint, image, name);
    let profile = super::codex_policy::ProfileFile::create(policy_id, owner)?;
    let pos = a
        .iter()
        .position(|s| s == image)
        .ok_or("invalid_container_image")?;
    a.splice(
        pos..pos,
        [
            "--label".into(),
            format!("com.cortexos.validation={owner}"),
            "--label=com.cortexos.purpose=harness_codex_runtime".into(),
            format!("--tmpfs=/workspace:{COPY}"),
            format!("--tmpfs=/home/cortexos:{HOME}"),
            "--env=HOME=/home/cortexos".into(),
            "--env=CODEX_HOME=/home/cortexos/.codex".into(),
        ],
    );
    if let Some(file) = &profile {
        let pos = a
            .iter()
            .position(|s| s == image)
            .ok_or("invalid_container_image")?;
        a.insert(pos, file.argument());
    }
    let work = a.iter().position(|s| s == "--workdir=/").unwrap();
    a[work] = "--workdir=/workspace".into();
    *a.last_mut().unwrap() = PROBE.into();
    // Settle create before any cancellation so the persisted owner can clean it.
    let created = command(&a, None, Duration::from_secs(5), None)?;
    drop(profile);
    if created.status != 0 || created.termination.is_some() {
        return Err(created
            .termination
            .unwrap_or("container_create_failed".into()));
    }
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    let config = docker_json(command(
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
    if !configuration_valid(&config)
        || !super::codex_policy::matches(&config, policy_id)
        || config["Config"]["Image"] != image
        || config["Config"]["Labels"]["com.cortexos.validation"] != owner
        || config["HostConfig"]["Tmpfs"]["/workspace"] != COPY
        || config["HostConfig"]["Tmpfs"]["/home/cortexos"] != HOME
        || config["Config"]["Cmd"] != json!(["-I", "-S", "-u", "-c", PROBE])
    {
        return Err("container_limits_not_confirmed".into());
    }
    let reply = command(
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
    if let Some(reason) = reply.termination {
        return Err(reason);
    }
    if reply.status != 0 {
        return Err("codex_probe_failed".into());
    }
    let raw: Value =
        serde_json::from_slice(&reply.output).map_err(|_| "codex_probe_invalid_response")?;
    let checks = classify(&raw)?;
    let mut limits = policy();
    let object = limits.as_object_mut().unwrap();
    object.remove("caseWallSeconds");
    object.remove("assertionWallSeconds");
    object.insert("checkWallSeconds".into(), json!(30));
    object.insert("probeCommandSeconds".into(), json!(8));
    object.insert("workspaceTmpfsBytes".into(), json!(2097152));
    object.insert("homeTmpfsBytes".into(), json!(4194304));
    object.insert(
        "seccompPolicy".into(),
        super::codex_policy::describe(policy_id)?,
    );
    Ok(
        json!({"checks":checks,"raw":raw,"imageId":image,"binaryHash":BINARY,"cliVersion":VERSION,"policy":limits,"verifiedConfiguration":{"user":config["Config"]["User"],"hostConfig":config["HostConfig"],"mounts":config["Mounts"]},"modelCalls":0,"taskExecution":"Not Run","canRun":false,"sandboxReady":checks.iter().all(|c|c["state"]=="passed")}),
    )
}
fn classify(raw: &Value) -> Result<Vec<Value>, String> {
    if raw["protocol"] != 2 || raw["modelCalls"] != 0 {
        return Err("codex_probe_invalid_response".into());
    }
    let binary = raw["binaryHash"] == BINARY && raw["runtimeFilesVerified"] == true;
    let version = raw["version"]["exitCode"] == 0
        && raw["version"]["stdout"]
            .as_str()
            .is_some_and(|s| s.trim() == format!("codex-cli {VERSION}"));
    let automation = raw["execHelp"]["exitCode"] == 0
        && raw["execHelp"]["stdout"].as_str().is_some_and(|s| {
            [
                "--json",
                "--sandbox",
                "--model",
                "--ephemeral",
                "--skip-git-repo-check",
            ]
            .iter()
            .all(|flag| s.contains(flag))
        });
    let boundary: Option<Value> = raw["sandbox"]["stdout"]
        .as_str()
        .and_then(|s| serde_json::from_str(s.trim()).ok());
    let sandbox = binary
        && version
        && raw["sandbox"]["exitCode"] == 0
        && boundary.as_ref().is_some_and(|b| {
            b["uid"] == 65534
                && [
                    "insideWrite",
                    "outsideDenied",
                    "readonlyRootDenied",
                    "networkDenied",
                    "mountDenied",
                    "bpfDenied",
                    "zeroCapabilities",
                    "noNewPrivileges",
                    "seccompActive",
                    "noDockerSocket",
                ]
                .iter()
                .all(|key| b[key] == true)
        });
    let reason = if raw["sandbox"]["stderr"]
        .as_str()
        .is_some_and(|s| s.contains("No permissions to create a new namespace"))
    {
        "namespace_unavailable"
    } else {
        "sandbox_not_verified"
    };
    Ok(vec![
        json!({"id":"binary","state":if binary{"passed"}else{"blocked"}}),
        json!({"id":"version","state":if version{"passed"}else{"blocked"}}),
        json!({"id":"automation","state":if automation{"passed"}else{"blocked"}}),
        json!({"id":"sandbox","state":if sandbox{"passed"}else{"blocked"},"reason":if sandbox{"verified"}else{reason}}),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_runtime_checks_reject_missing_boundary_proof_and_namespace_failure() {
        let mut r = json!({"protocol":2,"modelCalls":0,"binaryHash":BINARY,"runtimeFilesVerified":true,"version":{"exitCode":0,"stdout":format!("codex-cli {VERSION}\n")},"execHelp":{"exitCode":0,"stdout":"--json --sandbox --model --ephemeral --skip-git-repo-check"},"sandbox":{"exitCode":1,"stdout":"","stderr":"bwrap: No permissions to create a new namespace"}});
        assert_eq!(classify(&r).unwrap()[3]["reason"], "namespace_unavailable");
        r["sandbox"] = json!({"exitCode":0,"stdout":json!({"insideWrite":true,"outsideDenied":false,"uid":65534}).to_string()});
        assert_eq!(classify(&r).unwrap()[3]["state"], "blocked");
        let mut proof = json!({"uid":65534});
        for key in [
            "insideWrite",
            "outsideDenied",
            "readonlyRootDenied",
            "networkDenied",
            "mountDenied",
            "bpfDenied",
            "zeroCapabilities",
            "noNewPrivileges",
            "seccompActive",
            "noDockerSocket",
        ] {
            proof[key] = json!(true);
        }
        r["sandbox"]["stdout"] = json!(proof.to_string());
        assert!(classify(&r).unwrap().iter().all(|c| c["state"] == "passed"));
        r["binaryHash"] = json!("tampered");
        assert_eq!(classify(&r).unwrap()[3]["state"], "blocked");
        r["modelCalls"] = json!(1);
        assert!(classify(&r).is_err());
    }
}
