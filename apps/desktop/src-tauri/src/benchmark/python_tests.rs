use super::*;
#[test]
fn scoring_python_cancellation_is_preserved_before_handshake_in_every_cli_phase() {
    let image = format!("sha256:{}", "a".repeat(64));
    for cancel_at in 0..6 {
        let mut phase = 0;
        let result = score_with(&case(), "not executed", Some(&image), |a, _, _| {
            if a.iter().any(|v| v == "rm") {
                return Ok(reply(json!("removed")));
            }
            let current = phase;
            phase += 1;
            if current == cancel_at {
                return Ok(Reply {
                    status: -1,
                    output: vec![],
                    error: vec![],
                    termination: Some("cancelled".into()),
                });
            }
            if a[0] == "context" {
                Ok(reply(json!("npipe:////./pipe/docker_engine")))
            } else if a.iter().any(|v| v == "info") {
                Ok(reply(json!("linux")))
            } else if a.iter().any(|v| v == "image") {
                Ok(reply(
                    json!({"Id":image,"Os":"linux","Config":{"Volumes":null}}),
                ))
            } else if a.iter().any(|v| v == "create") {
                Ok(reply(json!("created")))
            } else {
                Ok(reply(container()))
            }
        });
        assert_eq!(
            result.state,
            State::Cancelled,
            "phase {cancel_at}: {}",
            result.details
        );
        assert_eq!(result.details["reason"], "cancelled");
    }
}
fn case() -> Case {
    super::super::dataset::builtin_version(2)
        .unwrap()
        .cases
        .into_iter()
        .find(|c| c.case_id == "code-001")
        .unwrap()
}
fn reply(v: Value) -> Reply {
    Reply {
        status: 0,
        output: serde_json::to_vec(&v).unwrap(),
        error: vec![],
        termination: None,
    }
}
fn container() -> Value {
    json!({"Config":{"User":"65534:65534"},"HostConfig":{"NetworkMode":"none","ReadonlyRootfs":true,"Privileged":false,"NanoCpus":1000000000u64,"Memory":268435456,"MemorySwap":268435456,"PidsLimit":64,"CapDrop":["ALL"],"SecurityOpt":["no-new-privileges"],"Binds":null},"Mounts":[]})
}
fn typed(v: &Value) -> Value {
    match v {
        Value::Number(_) => json!({"type":"int","value":v}),
        Value::Bool(_) => json!({"type":"bool","value":v}),
        Value::String(_) => json!({"type":"str","value":v}),
        Value::Array(a) => json!({"type":"list","items":a.iter().map(typed).collect::<Vec<_>>()}),
        _ => json!({"type":"NoneType","value":null}),
    }
}

#[test]
fn scoring_python_controller_records_each_assertion_and_never_sends_gold() {
    let c = case();
    let Expected::Python { tests, .. } = &c.expected else {
        panic!()
    };
    let mut index = 0;
    let mut removed = 0;
    let image = format!("sha256:{}", "a".repeat(64));
    let result = score_with(
        &c,
        "trusted fixture text, not executed",
        Some(&image),
        |a, input, _| {
            if a[0] == "context" {
                return Ok(reply(json!("npipe:////./pipe/docker_engine")));
            }
            if a.iter().any(|s| s == "info") {
                return Ok(reply(json!("linux")));
            }
            if a.iter().any(|s| s == "image") {
                return Ok(reply(
                    json!({"Id":image,"Os":"linux","Config":{"Volumes":null}}),
                ));
            }
            if a.iter().any(|s| s == "create") {
                assert!(a.contains(&"--network=none".into()));
                assert!(!a.iter().any(|v| v == "--volume" || v == "-v"));
                return Ok(reply(json!("created")));
            }
            if a.iter().any(|s| s == "inspect") {
                return Ok(reply(container()));
            }
            if a.iter().any(|s| s == "rm") {
                removed += 1;
                return Ok(reply(json!("removed")));
            }
            let payload: Value = serde_json::from_slice(input.unwrap()).unwrap();
            assert!(payload.get("expected").is_none());
            assert_eq!(payload["args"], json!(tests[index].args));
            let observed = json!({"protocol":1,"python":[3,13],"outcome":"returned","return":typed(&tests[index].output),"arguments":typed(&payload["args"]),"input_unchanged":true,"extra_output":false});
            index += 1;
            Ok(Reply {
                status: 0,
                output: format!(
                    "{}\n{}",
                    json!({"event":"python_ready","protocol":1,"python":[3,13]}),
                    observed
                )
                .into_bytes(),
                error: vec![],
                termination: None,
            })
        },
    );
    assert_eq!(result.state, State::Passed);
    assert_eq!(index, 7);
    assert_eq!(removed, 7);
    assert_eq!(result.details["assertions"].as_array().unwrap().len(), 7);
}

#[test]
fn scoring_python_missing_environment_is_not_an_answer_failure() {
    let unavailable = score_with(&case(), "unused", None, |_, _, _| {
        Err("docker_unavailable".into())
    });
    assert_eq!(unavailable.state, State::ExecutionError);
    assert!(unavailable.metrics["correctness"].is_null());
    assert_eq!(unavailable.metrics["assertionsRun"], 0);
    assert!(!valid_image("python:3.13"));
    assert!(!valid_image("sha256:bad"));
    let args = create_args(
        "npipe:////./pipe/docker_engine",
        &format!("sha256:{}", "a".repeat(64)),
        "owned-test",
    );
    for flag in [
        "--pull=never",
        "--network=none",
        "--read-only",
        "--cpus=1",
        "--memory=268435456",
        "--pids-limit=64",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--user=65534:65534",
    ] {
        assert!(args.iter().any(|s| s == flag));
    }
    assert!(
        args.iter()
            .all(|s| !s.contains("docker.sock") && !s.contains("--privileged"))
    );
}

#[test]
fn scoring_python_bool_cannot_be_integer_and_input_mutation_is_observed() {
    assert!(!return_type(
        &json!({"type":"bool","value":true}),
        &json!({"type":"integer"})
    ));
    assert!(!return_type(
        &json!({"type":"tuple","items":[]}),
        &json!({"type":"array","items":{"type":"integer"}})
    ));
    let mut insecure = container();
    insecure["HostConfig"]["NetworkMode"] = json!("bridge");
    assert!(!configuration_valid(&insecure));
    insecure = container();
    insecure["Mounts"] = json!([{"Type":"bind","Source":"sensitive"}]);
    assert!(!configuration_valid(&insecure));
    assert!(return_type(
        &json!({"type":"list","items":[{"type":"int","value":1}]}),
        &json!({"type":"array","items":{"type":"integer"}})
    ));
}

fn controlled(c: &Case, mode: &str) -> (Score, usize) {
    let Expected::Python { tests, .. } = &c.expected else {
        panic!()
    };
    let mut index = 0;
    let mut cleaned = 0;
    let image = format!("sha256:{}", "a".repeat(64));
    let score = score_with(c, "fixture text only", Some(&image), |a, input, _| {
        if mode == "panic" {
            panic!("synthetic controller error");
        }
        if a[0] == "context" {
            return Ok(reply(json!(if mode == "remote" {
                "tcp://remote:2375"
            } else {
                "unix:///var/run/docker.sock"
            })));
        }
        if a.iter().any(|s| s == "info") {
            return Ok(reply(json!("linux")));
        }
        if a.iter().any(|s| s == "image") {
            return Ok(reply(
                json!({"Id":image,"Os":"linux","Config":{"Volumes":null}}),
            ));
        }
        if a.iter().any(|s| s == "create") {
            return Ok(reply(json!("created")));
        }
        if a.iter().any(|s| s == "inspect") {
            let mut c = container();
            if mode == "insecure" {
                c["HostConfig"]["ReadonlyRootfs"] = json!(false);
            }
            return Ok(reply(c));
        }
        if a.iter().any(|s| s == "rm") {
            cleaned += 1;
            return if mode == "cleanup_failure" {
                Err("daemon_down".into())
            } else {
                Ok(reply(json!("removed")))
            };
        }
        let payload: Value = serde_json::from_slice(input.unwrap()).unwrap();
        let mut observed = json!({"protocol":1,"python":[3,13],"outcome":"returned","return":typed(&tests[index].output),"arguments":typed(&payload["args"]),"input_unchanged":true,"extra_output":false});
        index += 1;
        match mode {
            "wrong" => observed["return"] = json!({"type":"int","value":-999}),
            "bool" => observed["return"] = json!({"type":"bool","value":false}),
            "mutation" => {
                observed["input_unchanged"] = json!(false);
            }
            "spoof_mutation_flag" => observed["arguments"] = typed(&json!([[], [99]])),
            "extra_output" => observed["extra_output"] = json!(true),
            "program_error" => {
                observed["outcome"] = json!("program_error");
                observed["error_type"] = json!("ValueError");
            }
            _ => (),
        }
        let ready = json!({"event":"python_ready","python":[3,13],"protocol":1});
        Ok(Reply {
            status: if mode == "timeout" { -1 } else { 0 },
            output: format!("{ready}\n{observed}").into_bytes(),
            error: vec![],
            termination: if mode == "timeout" {
                Some("wall_timeout".into())
            } else if mode == "cancelled" {
                Some("cancelled".into())
            } else {
                None
            },
        })
    });
    (score, cleaned)
}

#[test]
fn scoring_python_all_93_controlled_observations_and_failure_modes() {
    let b = super::super::dataset::builtin_version(2).unwrap();
    let mut total = 0;
    for c in b.cases.iter().filter(|c| c.category == "coding") {
        let (score, cleaned) = controlled(c, "normal");
        assert_eq!(score.state, State::Passed, "{}", c.case_id);
        total += cleaned;
    }
    assert_eq!(total, 93);
    for mode in [
        "wrong",
        "bool",
        "mutation",
        "spoof_mutation_flag",
        "extra_output",
        "program_error",
        "timeout",
    ] {
        let (s, cleaned) = controlled(&case(), mode);
        assert_eq!(s.state, State::Failed, "{mode}");
        assert_eq!(cleaned, 7);
        assert_eq!(s.metrics["assertionsPassed"], 0);
    }
    for mode in ["remote", "insecure", "cleanup_failure"] {
        let (s, _) = controlled(&case(), mode);
        assert_eq!(s.state, State::ExecutionError);
        assert!(s.metrics["correctness"].is_null());
    }
    let (s, cleaned) = controlled(&case(), "cancelled");
    assert_eq!(s.state, State::Cancelled);
    assert_eq!(cleaned, 1);
    let (s, _) = controlled(&case(), "panic");
    assert_eq!(s.state, State::ScoringError);
}

#[test]
#[ignore = "Requires explicitly prepared local Docker Linux and CORTEXOS_BENCHMARK_TEST_IMAGE=sha256:...; never pulls an image"]
fn scoring_python_real_docker_types_mutation_and_cleanup() {
    let image =
        std::env::var("CORTEXOS_BENCHMARK_TEST_IMAGE").expect("explicit local image ID required");
    assert!(valid_image(&image));
    for (code, state) in [
        ("def sum_numbers(xs):\n    return sum(xs)\n", State::Passed),
        (
            "def sum_numbers(xs):\n    xs.append(0)\n    return sum(xs)\n",
            State::Failed,
        ),
        ("def sum_numbers(xs):\n    return False\n", State::Failed),
        (
            "def sum_numbers(xs):\n    import os\n    assert not os.path.exists('/var/run/docker.sock')\n    return sum(xs)\n",
            State::Passed,
        ),
        (
            "def sum_numbers(xs):\n    open('/cortexos-forbidden-file', 'w')\n    return sum(xs)\n",
            State::Failed,
        ),
    ] {
        let score = score(&case(), code, Some(&image), None);
        assert_eq!(score.state, state, "{}", score.details);
        assert!(
            score.details["ownedContainers"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["removed"] == true)
        );
    }
}
