use super::{
    dataset::{Case, Expected},
    evaluation::{Score, State},
    evaluation_json,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::watch;

const SHIM: &str = include_str!("python_runner.py");
const LIMIT: usize = 65536;
pub fn valid_image(image: &str) -> bool {
    image.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    })
}
pub fn policy() -> Value {
    json!({"version":1,"python":"3.13","network":"none","cpu":1,"memoryBytes":268435456,"pids":64,"caseWallSeconds":60,"assertionWallSeconds":5,"cleanupWallSeconds":5,"outputBytes":LIMIT,"readonlyRoot":true,"user":"65534:65534","noNewPrivileges":true,"capDrop":"ALL","hostMounts":false,"dockerSocket":false,"imagePull":"never"})
}

#[derive(Debug)]
pub(crate) struct Reply {
    pub status: i32,
    pub output: Vec<u8>,
    pub error: Vec<u8>,
    pub termination: Option<String>,
}
/// Bounded native Docker CLI invocation. Never invokes a shell or host Python.
fn command(
    args: &[String],
    input: Option<&[u8]>,
    timeout: Duration,
    cancel: Option<&watch::Receiver<bool>>,
) -> Result<Reply, String> {
    let mut cmd = Command::new("docker");
    cmd.args(args)
        .env_remove("DOCKER_HOST")
        .env_remove("DOCKER_CONTEXT")
        .env_remove("DOCKER_TLS_VERIFY")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().map_err(|_| "docker_unavailable".to_string())?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicUsize::new(0));
    let read = |mut stream: Box<dyn Read + Send>| {
        let data = Arc::new(Mutex::new(Vec::new()));
        let saved = data.clone();
        let exceeded = exceeded.clone();
        let total = total.clone();
        let worker = std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut data = saved.lock().unwrap();
                        let room = LIMIT.saturating_sub(total.fetch_add(n, Ordering::SeqCst));
                        data.extend_from_slice(&chunk[..n.min(room)]);
                        if n > room {
                            exceeded.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            }
        });
        (data, worker)
    };
    let (stdout, out_worker) = read(Box::new(child.stdout.take().ok_or("docker_stdout")?));
    let (stderr, err_worker) = read(Box::new(child.stderr.take().ok_or("docker_stderr")?));
    let writer = if let Some(input) = input {
        let mut stdin = child.stdin.take().ok_or("docker_stdin")?;
        let bytes = input.to_vec();
        Some(std::thread::spawn(move || stdin.write_all(&bytes)))
    } else {
        None
    };
    let start = Instant::now();
    let result = loop {
        if cancel.is_some_and(|c| *c.borrow()) {
            break Err("cancelled".to_string());
        }
        if exceeded.load(Ordering::SeqCst) {
            break Err("output_limit".to_string());
        }
        if start.elapsed() > timeout {
            break Err("wall_timeout".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.code().unwrap_or(-1)),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => break Err("docker_wait_failed".into()),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = writer.map(|w| w.join());
    let _ = out_worker.join();
    let _ = err_worker.join();
    let termination = if exceeded.load(Ordering::SeqCst) {
        Some("output_limit".into())
    } else {
        result.as_ref().err().cloned()
    };
    Ok(Reply {
        status: result.unwrap_or(-1),
        output: stdout.lock().unwrap().clone(),
        error: stderr.lock().unwrap().clone(),
        termination,
    })
}
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}
fn docker_json(reply: Reply) -> Result<Value, String> {
    if reply.status != 0 {
        return Err("docker_environment_error".into());
    }
    serde_json::from_slice(&reply.output).map_err(|_| "docker_invalid_response".into())
}
pub fn environment(
    image: Option<&str>,
    cancel: Option<&watch::Receiver<bool>>,
) -> Result<Value, String> {
    environment_with(image, &mut |a, i, t| command(a, i, t, cancel))
}
fn environment_with(
    image: Option<&str>,
    run: &mut impl FnMut(&[String], Option<&[u8]>, Duration) -> Result<Reply, String>,
) -> Result<Value, String> {
    let endpoint = docker_json(run(
        &args(&[
            "context",
            "inspect",
            "--format",
            "{{json .Endpoints.docker.Host}}",
        ]),
        None,
        Duration::from_secs(5),
    )?)?;
    let endpoint = endpoint
        .as_str()
        .filter(|s| s.starts_with("npipe://") || s.starts_with("unix://"))
        .ok_or("local_docker_required")?;
    if docker_json(run(
        &args(&["--host", endpoint, "info", "--format", "{{json .OSType}}"]),
        None,
        Duration::from_secs(5),
    )?)? != "linux"
    {
        return Err("linux_docker_required".into());
    }
    let image = image
        .filter(|s| valid_image(s))
        .ok_or("select_local_python_image_id")?;
    let inspected = docker_json(run(
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
    )?)?;
    if inspected["Id"] != image
        || inspected["Os"] != "linux"
        || inspected["Config"]["Volumes"]
            .as_object()
            .is_some_and(|v| !v.is_empty())
    {
        return Err("invalid_python_image".into());
    }
    Ok(
        json!({"endpoint":endpoint,"imageId":image,"metadata":{"id":inspected["Id"],"os":inspected["Os"],"architecture":inspected["Architecture"],"repoDigests":inspected["RepoDigests"]},"pythonVersionStatus":"checked_in_each_assertion_container"}),
    )
}
pub(crate) fn create_args(endpoint: &str, image: &str, name: &str) -> Vec<String> {
    args(&[
        "--host",
        endpoint,
        "create",
        "--pull=never",
        "--name",
        name,
        "--network=none",
        "--read-only",
        "--cpus=1",
        "--memory=268435456",
        "--memory-swap=268435456",
        "--pids-limit=64",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--user=65534:65534",
        "--tmpfs=/tmp:rw,noexec,nosuid,size=16m,mode=1777",
        "--shm-size=8m",
        "--ulimit=nofile=64:64",
        "--ulimit=core=0:0",
        "--ulimit=fsize=1048576:1048576",
        "--log-driver=none",
        "--no-healthcheck",
        "--workdir=/",
        "--entrypoint=python3",
        "-i",
        image,
        "-I",
        "-S",
        "-u",
        "-c",
        SHIM,
    ])
}
fn configuration_valid(value: &Value) -> bool {
    let h = &value["HostConfig"];
    value["Config"]["User"] == "65534:65534"
        && h["NetworkMode"] == "none"
        && h["ReadonlyRootfs"] == true
        && h["Privileged"] == false
        && h["NanoCpus"] == 1000000000u64
        && h["Memory"] == 268435456
        && h["MemorySwap"] == 268435456
        && h["PidsLimit"] == 64
        && h["CapDrop"]
            .as_array()
            .is_some_and(|a| a.contains(&json!("ALL")))
        && h["SecurityOpt"].as_array().is_some_and(|a| {
            a.iter().any(|s| {
                s.as_str()
                    .is_some_and(|s| s.starts_with("no-new-privileges"))
            })
        })
        && h["Binds"].as_array().is_none_or(Vec::is_empty)
        && value["Mounts"]
            .as_array()
            .is_some_and(|a| a.iter().all(|m| m["Type"] == "tmpfs"))
}
fn unpack(value: &Value) -> Option<Value> {
    match value["type"].as_str()? {
        "int" if value["value"].is_i64() || value["value"].is_u64() => Some(value["value"].clone()),
        "bool" if value["value"].is_boolean() => Some(value["value"].clone()),
        "str" if value["value"].is_string() => Some(value["value"].clone()),
        "NoneType" if value["value"].is_null() => Some(Value::Null),
        "list" => value["items"]
            .as_array()?
            .iter()
            .map(unpack)
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        _ => None,
    }
}
fn return_type(value: &Value, schema: &Value) -> bool {
    match schema["type"].as_str() {
        Some("integer") => value["type"] == "int",
        Some("boolean") => value["type"] == "bool",
        Some("string") => value["type"] == "str",
        Some("array") => {
            value["type"] == "list"
                && value["items"]
                    .as_array()
                    .is_some_and(|a| a.iter().all(|v| return_type(v, &schema["items"])))
        }
        _ => false,
    }
}

pub fn score(
    case: &Case,
    code: &str,
    image: Option<&str>,
    cancel: Option<&watch::Receiver<bool>>,
) -> Score {
    score_with(case, code, image, |a, i, t| {
        command(
            a,
            i,
            t,
            if a.iter().any(|s| s == "rm") {
                None
            } else {
                cancel
            },
        )
    })
}
/// The test seam supplies controlled CLI replies; it never executes submissions.
pub(crate) fn score_with(
    case: &Case,
    code: &str,
    image: Option<&str>,
    run: impl FnMut(&[String], Option<&[u8]>, Duration) -> Result<Reply, String>,
) -> Score {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        score_inner(case, code, image, run)
    }))
    .unwrap_or_else(|_| Score::state(State::ScoringError, "python_controller_panicked"))
}
fn score_inner(
    case: &Case,
    code: &str,
    image: Option<&str>,
    run: impl FnMut(&[String], Option<&[u8]>, Duration) -> Result<Reply, String>,
) -> Score {
    if super::dataset::validate_runtime_case(case).is_err() {
        return Score::state(State::ScoringError, "dataset_or_contract_integrity_error");
    }
    let Expected::Python { function, tests } = &case.expected else {
        return Score::state(State::ScoringError, "invalid_python_standard");
    };
    let Some(cfg) = case.scoring.contract.as_ref() else {
        return Score::state(State::ScoringError, "missing_python_contract");
    };
    if cfg["python_version"] != "3.13"
        || cfg["strict_python_types"] != true
        || cfg["input_mutation"] != "forbidden"
        || tests.is_empty()
    {
        return Score::state(State::ScoringError, "invalid_python_contract");
    }
    let started = Instant::now();
    let mut results = Vec::new();
    let mut containers = Vec::new();
    let mut transport = run;
    let mut run = |a: &[String], i: Option<&[u8]>, timeout: Duration| {
        let cleanup = a.iter().any(|s| s == "rm");
        let remaining = Duration::from_secs(60).saturating_sub(started.elapsed());
        if remaining.is_zero() && !cleanup {
            return Err("case_wall_timeout".into());
        }
        transport(
            a,
            i,
            if cleanup {
                timeout
            } else {
                timeout.min(remaining)
            },
        )
    };
    let outcome: Result<(), (State, String)> = (|| {
        let env = environment_with(image, &mut run).map_err(|e| (State::ExecutionError, e))?;
        let endpoint = env["endpoint"].as_str().unwrap();
        let image = env["imageId"].as_str().unwrap();
        if code.len() > LIMIT {
            return Err((State::Failed, "submission_output_limit".into()));
        }
        for (index, test) in tests.iter().enumerate() {
            if started.elapsed() > Duration::from_secs(60) {
                return Err((State::ExecutionError, "case_wall_timeout".into()));
            }
            let name = format!("cortexos-bench-{}", crate::models::new_id());
            containers.push(json!({"name":name,"removed":false}));
            let observed: Result<Reply, (State, String)> = (|| {
                let created = run(
                    &create_args(endpoint, image, &name),
                    None,
                    Duration::from_secs(5),
                )
                .map_err(|e| (State::ExecutionError, e))?;
                if created.status != 0 {
                    return Err((State::ExecutionError, "container_create_failed".into()));
                }
                let config = docker_json(
                    run(
                        &args(&[
                            "--host",
                            endpoint,
                            "inspect",
                            &name,
                            "--format",
                            "{{json .}}",
                        ]),
                        None,
                        Duration::from_secs(5),
                    )
                    .map_err(|e| (State::ExecutionError, e))?,
                )
                .map_err(|e| (State::ExecutionError, e))?;
                if !configuration_valid(&config) {
                    return Err((
                        State::ExecutionError,
                        "container_limits_not_confirmed".into(),
                    ));
                }
                let payload =
                    serde_json::to_vec(&json!({"code":code,"function":function,"args":test.args}))
                        .map_err(|_| (State::ScoringError, "python_payload_encoding".into()))?;
                run(
                    &args(&[
                        "--host",
                        endpoint,
                        "start",
                        "--attach",
                        "--interactive",
                        &name,
                    ]),
                    Some(&payload),
                    Duration::from_secs(5),
                )
                .map_err(|e| {
                    (
                        if e == "cancelled" {
                            State::Cancelled
                        } else {
                            State::ExecutionError
                        },
                        e,
                    )
                })
            })();
            let cleanup = run(
                &args(&["--host", endpoint, "rm", "--force", "--volumes", &name]),
                None,
                Duration::from_secs(5),
            );
            let removed = cleanup.is_ok_and(|r| r.status == 0);
            containers.last_mut().unwrap()["removed"] = json!(removed);
            if !removed {
                return Err((State::ExecutionError, "container_cleanup_failed".into()));
            }
            let reply = match observed {
                Ok(r) => r,
                Err((State::Failed, reason)) => {
                    results.push(json!({"index":index+1,"passed":false,"reason":reason}));
                    continue;
                }
                Err(e) => return Err(e),
            };
            if reply.status == 125 || reply.status == 126 || reply.status == 127 {
                return Err((State::ExecutionError, "container_start_failed".into()));
            }
            let (ready, body) = std::str::from_utf8(&reply.output)
                .ok()
                .and_then(|s| s.split_once('\n'))
                .ok_or((State::ExecutionError, "python_did_not_start".into()))?;
            let ready = evaluation_json::decode(ready)
                .map_err(|_| (State::ExecutionError, "python_handshake_invalid".into()))?;
            if ready["event"] != "python_ready" || ready["python"] != json!([3, 13]) {
                return Err((State::ExecutionError, "python_runtime_mismatch".into()));
            }
            if let Some(reason) = reply.termination {
                if reason == "cancelled" {
                    return Err((State::Cancelled, reason));
                }
                if ["wall_timeout", "output_limit"].contains(&reason.as_str()) {
                    results.push(json!({"index":index+1,"passed":false,"reason":reason}));
                    continue;
                }
                return Err((State::ExecutionError, reason));
            }
            let observed = match evaluation_json::decode(body) {
                Ok(v) => v,
                Err(_) => {
                    results.push(
                        json!({"index":index+1,"passed":false,"reason":"program_protocol_invalid"}),
                    );
                    continue;
                }
            };
            if observed["protocol"] != 1 || observed["python"] != json!([3, 13]) {
                return Err((State::ExecutionError, "python_runtime_mismatch".into()));
            }
            let returned = unpack(&observed["return"]);
            let arguments = unpack(&observed["arguments"]);
            let types = return_type(&observed["return"], &cfg["return_schema"]);
            let unchanged = observed["input_unchanged"] == true
                && arguments
                    .as_ref()
                    .is_some_and(|a| evaluation_json::equal(a, &json!(test.args)));
            let equal = returned
                .as_ref()
                .is_some_and(|r| evaluation_json::equal(r, &test.output));
            let passed = reply.status == 0
                && observed["outcome"] == "returned"
                && types
                && unchanged
                && equal
                && observed["extra_output"] == false;
            let passed = passed && reply.error.is_empty();
            results.push(json!({"index":index+1,"passed":passed,"typeValid":types,"inputUnchanged":unchanged,"valueCorrect":equal,"observation":observed,"stderrPresent":!reply.error.is_empty()}));
        }
        Ok(())
    })();
    let passed = results.iter().filter(|r| r["passed"] == true).count();
    let mut score = match outcome {
        Ok(()) => Score::state(
            if passed == tests.len() {
                State::Passed
            } else {
                State::Failed
            },
            "python_assertions_completed",
        ),
        Err((state, reason)) => Score::state(state, reason),
    };
    score.metrics["assertionsPlanned"] = json!(tests.len());
    score.metrics["assertionsRun"] = json!(results.len());
    score.metrics["assertionsPassed"] = json!(passed);
    score.details["assertions"] = json!(results);
    score.details["sandboxPolicy"] = policy();
    score.details["imageId"] = json!(image);
    score.details["ownedContainers"] = json!(containers);
    score
}

#[cfg(test)]
#[path = "python_tests.rs"]
mod tests;
