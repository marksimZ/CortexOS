//! Offline Claude gate. Its fixed reply fixture never contacts a model.
use super::*;
pub(crate) const VERSION: &str = "2.1.309";
const BINARY: &str = "4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658";
const PROBE: &str = include_str!("claude_probe.py");
const SANDBOX: &str = include_str!("../../../../../../scripts/harness/claude_sandbox_probe.py");
const COPY: &str = "rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534";
const HOME: &str = "rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534";
pub(crate) fn definition() -> Value {
    definition_for("claude-program-2")
}
pub(crate) fn definition_for(probe: &str) -> Value {
    let mut value = json!({"version":VERSION,"binaryHash":BINARY,"manifestUrl":format!("https://downloads.claude.ai/claude-code-releases/{VERSION}/manifest.json"),"probeVersion":probe,"runtimePolicy":"claude-runtime-1"});
    if probe == "claude-program-2" {
        value["probeHash"] = json!(crate::benchmark::dataset::hash(PROBE.as_bytes()));
        value["sandboxProbeHash"] = json!(crate::benchmark::dataset::hash(SANDBOX.as_bytes()));
    }
    value
}
fn metadata(endpoint: &str, image: &str) -> Result<bool, String> {
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
        || labels["com.cortexos.harness"] != "claude-code"
        || labels["com.cortexos.claude.version"] != VERSION
        || labels["com.cortexos.claude.sha256"] != BINARY
        || labels["com.cortexos.runtime.policy"] != "claude-runtime-1"
        || v["Config"]["Volumes"]
            .as_object()
            .is_some_and(|o| !o.is_empty())
    {
        return Err("claude_runtime_image_mismatch".into());
    }
    Ok(labels["com.cortexos.claude.dependencies"] == "debian-trixie-bwrap-socat-1")
}
pub(crate) fn environment_for(
    image: &str,
    policy_id: &str,
    cancel: &watch::Receiver<bool>,
) -> Result<Value, String> {
    claude_policy::describe(policy_id)?;
    let env = harness_environment(image, Some(cancel))?;
    let dependencies = metadata(
        env["endpoint"].as_str().ok_or("local_docker_required")?,
        image,
    )?;
    if policy_id == claude_policy::COMPATIBLE && !dependencies {
        return Err("claude_sandbox_dependencies_image_required".into());
    }
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
            "label=com.cortexos.runtime.policy=claude-runtime-1",
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
    let mut found = vec![];
    for id in String::from_utf8_lossy(&r.output)
        .lines()
        .filter(|s| valid_image(s))
        .take(16)
        .collect::<std::collections::BTreeSet<_>>()
    {
        if let Ok(dependencies) = metadata(endpoint, id) {
            found.push(json!({"imageId":id,"version":VERSION,"dependencies":dependencies}));
        }
    }
    Ok(json!({"images":found,"downloads":false,"modelCalls":0}))
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
            .is_none_or(|s| uuid::Uuid::parse_str(s).is_err())
    {
        return Err("invalid_container_owner".into());
    }
    let dependencies = metadata(endpoint, image)?;
    if policy_id == claude_policy::COMPATIBLE && !dependencies {
        return Err("claude_sandbox_dependencies_image_required".into());
    }
    let profile = claude_policy::create(policy_id, owner)?;
    let probe = format!(
        "RUN_SANDBOX = {}\nSANDBOX_SOURCE = {}\n{}",
        if policy_id == claude_policy::COMPATIBLE {
            "True"
        } else {
            "False"
        },
        json!(if policy_id == claude_policy::COMPATIBLE {
            SANDBOX
        } else {
            ""
        }),
        PROBE
    );
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
            "--label=com.cortexos.purpose=harness_claude_runtime".into(),
            format!("--tmpfs=/workspace:{COPY}"),
            format!("--tmpfs=/home/cortexos:{HOME}"),
        ],
    );
    if let Some(profile) = &profile {
        let at = a
            .iter()
            .position(|s| s == image)
            .ok_or("invalid_container_image")?;
        a.insert(at, profile.argument());
    }
    *a.last_mut().unwrap() = probe.clone();
    let created = command(&a, None, Duration::from_secs(5), None)?;
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
        || !claude_policy::matches(&config, policy_id)
        || config["Config"]["Image"] != image
        || config["Config"]["Labels"]["com.cortexos.validation"] != owner
        || config["Config"]["Labels"]["com.cortexos.purpose"] != "harness_claude_runtime"
        || config["HostConfig"]["Tmpfs"]["/workspace"] != COPY
        || config["HostConfig"]["Tmpfs"]["/home/cortexos"] != HOME
        || config["Config"]["Cmd"] != json!(["-I", "-S", "-u", "-c", probe])
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
        return Err("claude_probe_failed".into());
    }
    let raw = evaluation_json::decode(
        std::str::from_utf8(&reply.output).map_err(|_| "claude_probe_invalid_response")?,
    )
    .map_err(|_| "claude_probe_invalid_response")?;
    let checks = classify(&raw)?;
    let ready = checks
        .iter()
        .any(|c| c["id"] == "sandbox" && c["state"] == "passed");
    Ok(
        json!({"checks":checks,"raw":raw,"definition":definition(),"imageId":image,"policy":{"container":policy(),"checkWallSeconds":30,"commandWallSeconds":8,"securityPolicy":claude_policy::describe(policy_id)?,"workspaceTmpfsBytes":2097152,"homeTmpfsBytes":4194304},"verifiedConfiguration":{"user":config["Config"]["User"],"hostConfig":config["HostConfig"],"mounts":config["Mounts"]},"modelCalls":0,"taskExecution":"Not Run","sandboxReady":ready,"canRun":false}),
    )
}
fn classify(raw: &Value) -> Result<Vec<Value>, String> {
    if raw["protocol"] != 2 || raw["modelCalls"] != 0 {
        return Err("claude_probe_invalid_response".into());
    }
    let binary = raw["binaryHash"] == BINARY;
    let version = binary
        && raw["version"]["exitCode"] == 0
        && raw["version"]["stdout"]
            .as_str()
            .is_some_and(|s| s.trim() == format!("{VERSION} (Claude Code)"));
    let automation = version
        && raw["help"]["exitCode"] == 0
        && raw["help"]["stdout"].as_str().is_some_and(|s| {
            [
                "--bare",
                "--output-format",
                "stream-json",
                "--settings",
                "--setting-sources",
                "--strict-mcp-config",
                "--permission-mode",
                "--no-session-persistence",
            ]
            .iter()
            .all(|flag| s.contains(flag))
        })
        && ["control", "budget"].iter().all(|key| {
            raw["parser"][key]["exitCode"] == 1
                && raw["parser"][key]["stdout"] == ""
                && raw["parser"][key]["stderr"]
                    == "error: unknown option '--cortexos-invalid-option'\n"
        });
    let dependencies = ["bwrap", "socat"].iter().all(|key| {
        raw["dependencies"][key]
            .as_str()
            .is_some_and(|s| s.starts_with('/'))
    });
    let namespace = raw["namespace"]["stdout"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let sandbox = &raw["sandbox"];
    let proof = &sandbox["proof"];
    let settings = &sandbox["settings"]["sandbox"];
    let strict = settings["enabled"] == true
        && settings["failIfUnavailable"] == true
        && settings["allowUnsandboxedCommands"] == false
        && settings["enableWeakerNestedSandbox"] == false
        && settings["enableWeakerNetworkIsolation"] == false
        && settings["excludedCommands"] == json!([]);
    let container = &raw["container"];
    let outer = container["uid"] == 65534
        && container["capEff"] == "0000000000000000"
        && container["capBnd"] == "0000000000000000"
        && container["noNewPrivileges"] == "1"
        && container["seccomp"] == "2"
        && container["dockerSocket"] == false;
    let safe = outer
        && binary
        && version
        && automation
        && dependencies
        && strict
        && namespace
            .as_ref()
            .is_some_and(|n| n["newUserNamespace"] == true)
        && sandbox["protocol"] == "claude-sandbox-fixture-2"
        && sandbox["mode"] == "trusted_offline_fixture_not_model"
        && sandbox["modelCalls"] == 0
        && sandbox["passed"] == true
        && sandbox["cli"]["exitCode"] == 0
        && sandbox["protectedFilesUnchanged"] == true
        && sandbox["outerControlsReachable"] == true
        && sandbox["requests"].as_array().is_some_and(|v| v.len() == 2)
        && sandbox["serverErrors"] == json!([])
        && proof["protocol"] == "cortexos-claude-fixed-sandbox-2"
        && proof["uid"] == 65534
        && ["CapEff", "CapPrm", "CapAmb"]
            .iter()
            .all(|key| proof["capabilities"][key] == "0000000000000000")
        && [
            "insideWrite",
            "outsideDenied",
            "homeDenied",
            "readonlyRootDenied",
            "networkDenied",
            "mountDenied",
            "bpfDenied",
            "noUsableCapabilities",
            "nonRoot",
            "nestedUserNamespaceDenied",
            "noNewPrivileges",
            "seccompActive",
            "noDockerSocket",
        ]
        .iter()
        .all(|key| proof["checks"][key] == true);
    Ok(vec![
        json!({"id":"binary","state":if binary{"passed"}else{"blocked"}}),
        json!({"id":"version","state":if version{"passed"}else{"blocked"}}),
        json!({"id":"automation","state":if automation{"passed"}else{"blocked"}}),
        json!({"id":"dependencies","state":if dependencies{"passed"}else{"blocked"},"reason":if dependencies{"present_not_sandbox_proof"}else{"sandbox_dependencies_missing"}}),
        json!({"id":"namespace","state":if namespace.as_ref().is_some_and(|n|n["newUserNamespace"]==true){"passed"}else{"blocked"},"reason":"capability_observation_only"}),
        json!({"id":"sandbox","state":if safe{"passed"}else if sandbox.is_null(){"not_checked"}else{"blocked"},"reason":if safe{"trusted_offline_fixture_not_model"}else{"claude_bash_sandbox_not_verified"}}),
        json!({"id":"model_channel","state":"not_checked","reason":"controlled_anthropic_channel_not_implemented"}),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_gate_never_confuses_flags_dependencies_or_codex_proof_with_safe_execution() {
        let accepted = json!({"exitCode":1,"stdout":"","stderr":"error: unknown option '--cortexos-invalid-option'\n"});
        let mut raw = json!({"protocol":2,"modelCalls":0,"binaryHash":BINARY,"version":{"exitCode":0,"stdout":format!("{VERSION} (Claude Code)\n")},"help":{"exitCode":0,"stdout":"--bare --output-format stream-json --settings --setting-sources --strict-mcp-config --permission-mode --no-session-persistence"},"parser":{"control":accepted,"budget":accepted},"dependencies":{"bwrap":null,"socat":null},"namespace":{"stdout":"{\"newUserNamespace\":false,\"errno\":1}"}});
        let c = classify(&raw).unwrap();
        assert_eq!(c[0]["state"], "passed");
        assert_eq!(c[2]["state"], "passed");
        assert_eq!(c[3]["state"], "blocked");
        assert_eq!(c[5]["state"], "not_checked");
        raw["dependencies"] = json!({"bwrap":"/usr/bin/bwrap","socat":"/usr/bin/socat"});
        raw["namespace"]["stdout"] = json!("{\"newUserNamespace\":true,\"errno\":0}");
        let c = classify(&raw).unwrap();
        assert_eq!(c[3]["state"], "passed");
        assert_eq!(c[5]["state"], "not_checked");
        assert_eq!(c[6]["state"], "not_checked");
        raw["sandbox"] = json!({"passed":true,"modelCalls":0});
        assert_eq!(classify(&raw).unwrap()[5]["state"], "blocked");
        raw["binaryHash"] = json!("tampered");
        assert_eq!(classify(&raw).unwrap()[1]["state"], "blocked");
        raw["modelCalls"] = json!(1);
        assert!(classify(&raw).is_err());
    }
    #[test]
    fn claude_gate_keeps_policy_ids_and_temporary_files_separate() {
        assert!(claude_policy::describe(codex_policy::COMPATIBLE).is_err());
        assert!(codex_policy::describe(claude_policy::COMPATIBLE).is_err());
        assert!(claude_policy::describe("unconfined").is_err());
        let owner = crate::models::new_id();
        let codex = codex_policy::ProfileFile::create(codex_policy::COMPATIBLE, &owner).unwrap();
        let claude = claude_policy::create(claude_policy::COMPATIBLE, &owner).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap();
        let codex_path = root.join(format!("cortexos-codex-seccomp-{owner}.json"));
        let claude_path = root.join(format!("cortexos-claude-seccomp-{owner}.json"));
        assert!(codex_path.exists() && claude_path.exists());
        codex_policy::cleanup_file(&owner).unwrap();
        assert!(!codex_path.exists() && claude_path.exists());
        claude_policy::cleanup(&owner).unwrap();
        assert!(!claude_path.exists());
        drop((codex, claude));
    }
}
