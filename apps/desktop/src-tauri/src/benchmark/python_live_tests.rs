//! Explicit opt-in integration check: all submitted Python runs in real Docker.
use super::*;

#[test]
#[ignore = "requires approved real Docker; intentionally reaches the 60-second case deadline"]
fn scoring_python_live_case_deadline() {
    let image = std::env::var("CORTEXOS_PYTHON_IMAGE").expect("explicit image required");
    assert!(valid_image(&image));
    let mut case = super::super::dataset::builtin_version(2)
        .unwrap()
        .cases
        .into_iter()
        .find(|c| c.case_id == "code-001")
        .unwrap();
    case.case_id = "custom-live-case-deadline".into();
    if let Expected::Python { tests, .. } = &mut case.expected {
        *tests = vec![tests[0].clone(); 32];
    }
    super::super::personalized::cases::seal(&mut case).unwrap();
    let started = Instant::now();
    let result = score(
        &case,
        "def sum_numbers(xs):\n    while True:\n        pass",
        Some(&image),
        None,
    );
    let elapsed = started.elapsed();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-docker-live");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("case-deadline.json"),
        serde_json::to_vec_pretty(
            &json!({"expected":"Execution Error","elapsedMs":elapsed.as_millis(),"score":result}),
        )
        .unwrap(),
    )
    .unwrap();
    println!(
        "case deadline: {:?}, {}ms",
        result.state,
        elapsed.as_millis()
    );
    assert_eq!(result.state, State::ExecutionError, "{}", result.details);
    assert!(elapsed >= Duration::from_secs(55) && elapsed < Duration::from_secs(75));
    assert!(
        result.details["ownedContainers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["removed"] == true)
    );
    assert!(result.metrics["assertionsRun"].as_u64().unwrap() < 32);
}

#[test]
#[ignore = "requires approved local Linux Docker and an explicitly pinned Python 3.13 image"]
fn scoring_python_live_docker_limits_and_cleanup() {
    let image = std::env::var("CORTEXOS_PYTHON_IMAGE").expect("explicit local image ID required");
    assert!(valid_image(&image));
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-docker-live");
    std::fs::create_dir_all(&root).unwrap();
    let environment = environment(Some(&image), None).expect("real local Linux Docker required");
    std::fs::write(
        root.join("environment.json"),
        serde_json::to_vec_pretty(&environment).unwrap(),
    )
    .unwrap();
    let case = super::super::dataset::builtin_version(2)
        .unwrap()
        .cases
        .into_iter()
        .find(|c| c.case_id == "code-001")
        .unwrap();
    let scenarios = [
        (
            "correct",
            "def sum_numbers(xs):\n    return sum(xs)",
            State::Passed,
        ),
        ("wrong", "def sum_numbers(xs):\n    return 0", State::Failed),
        (
            "wrong_type",
            "def sum_numbers(xs):\n    return False",
            State::Failed,
        ),
        (
            "mutation",
            "def sum_numbers(xs):\n    xs.clear()\n    return 0",
            State::Failed,
        ),
        (
            "runtime_boundaries",
            r#"def sum_numbers(xs):
    import os, errno, socket
    assert os.getuid() == 65534 and os.getgid() == 65534
    assert not os.path.exists('/var/run/docker.sock')
    status = open('/proc/self/status').read()
    assert 'NoNewPrivs:\t1' in status
    assert int(next(line.split(':')[1] for line in status.splitlines() if line.startswith('CapEff:')), 16) == 0
    assert int(open('/sys/fs/cgroup/memory.max').read()) == 268435456
    assert int(open('/sys/fs/cgroup/pids.max').read()) == 64
    quota, period = open('/sys/fs/cgroup/cpu.max').read().split()
    assert int(quota) == int(period)
    try:
        open('/cortexos-readonly-probe', 'w').close()
    except OSError as error:
        assert error.errno in (errno.EROFS, errno.EACCES)
    else:
        raise AssertionError('root_write_succeeded')
    network = socket.socket()
    network.settimeout(0.2)
    try:
        network.connect(('203.0.113.1', 9))
    except OSError:
        pass
    else:
        raise AssertionError('external_network_connected')
    finally:
        network.close()
    with open('/tmp/cortexos-owned-temporary-file', 'w') as output:
        output.write('temporary container data')
    return sum(xs)"#,
            State::Passed,
        ),
        (
            "output_limit",
            "def sum_numbers(xs):\n    import os\n    os.write(1, b'x' * 131072)\n    return 0",
            State::Failed,
        ),
        (
            "memory_limit",
            "def sum_numbers(xs):\n    allocation = bytearray(512 * 1024 * 1024)\n    return len(allocation)",
            State::Failed,
        ),
        (
            "timeout",
            "def sum_numbers(xs):\n    while True:\n        pass",
            State::Failed,
        ),
        (
            "cancel",
            "def sum_numbers(xs):\n    while True:\n        pass",
            State::Cancelled,
        ),
    ];
    let mut summary = vec![];
    for (name, code, expected) in scenarios {
        let (sender, receiver) = watch::channel(false);
        let mut cancellation = None;
        let mut inspections = vec![];
        let mut final_states = vec![];
        let mut payloads_checked = 0;
        let started = Instant::now();
        let result = score_with(&case, code, Some(&image), |a, input, timeout| {
            if a.iter().any(|v| v == "start") {
                let payload: Value = serde_json::from_slice(input.unwrap()).unwrap();
                assert_eq!(payload.as_object().unwrap().len(), 3);
                assert!(payload.get("expected").is_none());
                payloads_checked += 1;
                if name == "cancel" && cancellation.is_none() {
                    let sender = sender.clone();
                    cancellation = Some(std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1500));
                        let _ = sender.send(true);
                    }));
                }
            }
            let cleanup = a.iter().any(|v| v == "rm");
            if cleanup {
                let endpoint = environment["endpoint"].as_str().unwrap();
                let state = command(
                    &args(&[
                        "--host",
                        endpoint,
                        "inspect",
                        a.last().unwrap(),
                        "--format",
                        "{{json .State}}",
                    ]),
                    None,
                    Duration::from_secs(5),
                    None,
                )
                .and_then(docker_json);
                final_states.push(state.unwrap_or_else(|error| json!({"error":error})));
            }
            let reply = command(
                a,
                input,
                timeout,
                if cleanup { None } else { Some(&receiver) },
            )?;
            if a.iter().any(|v| v == "inspect")
                && !a.iter().any(|v| v == "image")
                && !a.iter().any(|v| v == "context")
                && reply.status == 0
            {
                inspections.push(serde_json::from_slice::<Value>(&reply.output).unwrap());
            }
            Ok(reply)
        });
        if let Some(worker) = cancellation {
            worker.join().unwrap();
        }
        let removed = result.details["ownedContainers"]
            .as_array()
            .is_some_and(|items| {
                !items.is_empty() && items.iter().all(|item| item["removed"] == true)
            });
        let record = json!({"scenario":name,"expected":expected,"actual":result.state,"elapsedMs":started.elapsed().as_millis(),"score":result,"actualContainerInspections":inspections,"statesBeforeRemoval":final_states,"payloadsWithoutExpectedAnswer":payloads_checked,"containersRemoved":removed});
        std::fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        summary.push(json!({"scenario":name,"expected":expected,"actual":result.state,"containersRemoved":removed}));
        std::fs::write(
            root.join("summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        println!(
            "{name}: {:?}, {}ms, cleanup={removed}",
            result.state,
            started.elapsed().as_millis()
        );
        assert_eq!(result.state, expected, "{name}: {}", result.details);
        assert!(removed, "owned containers not cleaned in {name}");
        assert!(!inspections.is_empty() && inspections.iter().all(configuration_valid));
        for container in result.details["ownedContainers"].as_array().unwrap() {
            let absent = command(
                &args(&[
                    "--host",
                    environment["endpoint"].as_str().unwrap(),
                    "inspect",
                    container["name"].as_str().unwrap(),
                ]),
                None,
                Duration::from_secs(5),
                None,
            )
            .unwrap();
            assert_ne!(absent.status, 0, "container still exists after cleanup");
        }
        if name == "output_limit" {
            assert!(
                result.details["assertions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["reason"] == "output_limit")
            );
        }
        if name == "timeout" {
            assert!(
                result.details["assertions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["reason"] == "wall_timeout")
            );
        }
        if name == "memory_limit" {
            assert!(final_states.iter().any(|v| v["OOMKilled"] == true));
        }
    }
}
