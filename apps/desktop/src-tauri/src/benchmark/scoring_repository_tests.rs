use super::*;
use crate::{
    benchmark::{
        preflight::{ModelRequest, Parameters},
        repository::{Prepared, StartInput},
    },
    models::ExecutionPurpose,
};

fn database() -> (Database, String, std::path::PathBuf) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-scoring-v2")
        .join(new_id());
    let db = Database::open(&path).unwrap();
    let workspace = db.create_workspace("synthetic scoring fixture").unwrap().id;
    (db, workspace, path)
}
fn prepare(db: &Database, ws: &str, case_id: &str) -> Prepared {
    let c = db.benchmark_case(ws, "cortex-core-60", 2, case_id).unwrap();
    let input = StartInput {
        local_keep_alive_seconds: None,
        personalized_dataset: None,
        launch_key: new_id(),
        development_only: true,
        dataset_version: 2,
        python_image: None,
        dataset_hash: dataset::builtin_version(2).unwrap().content_hash,
        case_id: case_id.into(),
        model: ModelRequest {
            model: "synthetic-no-model-request".into(),
            expected_digest: Some("a".repeat(64)),
            requirements: c
                .requirements
                .iter()
                .filter(|s| s.as_str() != "python_sandbox")
                .cloned()
                .collect(),
            parameters: Parameters {
                num_ctx: 4096,
                num_predict: 128,
                temperature: 0.0,
                timeout_seconds: 30,
            },
        },
    };
    let intent = db.benchmark_intent(ws, input).unwrap();
    let profile = json!({"request":intent.input.model,"status":"synthetic_fixture_only"});
    db.prepare_benchmark_record(ws, &new_id(), &intent, &profile)
        .unwrap()
}
fn completed(db: &Database, p: &Prepared, text: &str) {
    db.execution_event(
        &p.execution.run,
        ExecutionPurpose::Benchmark,
        "model_request_started",
        json!({"synthetic":true}),
    )
    .unwrap();
    db.execution_finish(&p.execution.run,ExecutionPurpose::Benchmark,text,"completed",None,json!({"finishReason":"stop","evalCount":7,"promptEvalCount":23,"clientTiming":{"clock":"monotonic","firstTextMs":12,"completeMs":25}})).unwrap();
}

#[test]
fn scoring_storage_binds_version_and_preserves_saved_output_idempotently() {
    let (db, ws, path) = database();
    let chat = db.create_conversation(&ws).unwrap();
    let before = serde_json::to_value(db.bootstrap().unwrap()).unwrap();
    let p = prepare(&db, &ws, "schema-001");
    completed(&db, &p, r#"{"age":28.0,"name":"林禾"}"#);
    db.finish_benchmark(&ws, &p.summary.id).unwrap();
    let detail = db.benchmark_detail(&ws, &p.summary.id).unwrap();
    assert_eq!(detail.case.dataset_version, 2);
    assert_eq!(detail.evaluations.len(), 1);
    assert_eq!(detail.evaluations[0]["details"]["state"], "Passed");
    assert_eq!(
        detail.evaluations[0]["evaluatorVersion"],
        evaluation::ENGINE_VERSION
    );
    assert_eq!(
        detail.evaluations[0]["metrics"]["performance"]["evalCount"],
        7
    );
    assert_eq!(detail.statistics.as_ref().unwrap()["correctness"], 1.0);
    assert!(detail.statistics.as_ref().unwrap()["byDifficulty"]["hard"]["correctness"].is_null());
    assert_eq!(
        detail.statistics.as_ref().unwrap()["byEvaluator"]["python_assertions"]["planned"],
        0
    );
    assert_eq!(
        detail.statistics.as_ref().unwrap()["datasetScoringCoverage"],
        1.0 / 60.0
    );
    db.finish_benchmark(&ws, &p.summary.id).unwrap();
    assert_eq!(
        db.benchmark_detail(&ws, &p.summary.id)
            .unwrap()
            .evaluations
            .len(),
        1
    );
    assert!(
        db.benchmark_detail("other-workspace", &p.summary.id)
            .is_err()
    );
    assert!(
        db.benchmark_statistics("other-workspace", &p.summary.id)
            .is_err()
    );
    assert!(db.messages(&ws, &p.execution.run.conversation_id).is_err());
    assert!(db.run_detail(&ws, &p.execution.run.id).is_err());
    assert_eq!(db.conversations(&ws).unwrap().len(), 1);
    assert_eq!(db.conversations(&ws).unwrap()[0].id, chat.id);
    assert_eq!(
        serde_json::to_value(db.bootstrap().unwrap()).unwrap(),
        before
    );
    let id = p.summary.id.clone();
    let output = detail.output;
    drop(db);
    let db = Database::open(&path).unwrap();
    let reopened = db.benchmark_detail(&ws, &id).unwrap();
    assert_eq!(reopened.output, output);
    assert_eq!(reopened.evaluations.len(), 1);
}

#[test]
fn scoring_storage_native_calls_are_separate_from_text_and_hashed() {
    let (db, ws, _) = database();
    for (native, state) in [(true, "Passed"), (false, "Failed")] {
        let p = prepare(&db, &ws, "tool-002");
        let call =
            json!({"function":{"name":"search_notes","arguments":{"query":"发布清单","limit":3}}});
        if native {
            db.execution_event(
                &p.execution.run,
                ExecutionPurpose::Benchmark,
                "native_tool_calls_received",
                json!({"provider":"ollama","calls":[call]}),
            )
            .unwrap();
        }
        let text = if native {
            String::new()
        } else {
            call.to_string()
        };
        completed(&db, &p, &text);
        db.finish_benchmark(&ws, &p.summary.id).unwrap();
        let d = db.benchmark_detail(&ws, &p.summary.id).unwrap();
        assert_eq!(d.evaluations[0]["details"]["state"], state);
        assert_ne!(
            d.evaluations[0]["outputHash"],
            dataset::hash(text.as_bytes())
        );
        assert_eq!(d.statistics.unwrap()["scored"], 1);
    }
}

#[test]
fn scoring_storage_excluded_states_do_not_become_zero_answers() {
    let (db, ws, _) = database();
    for state in [
        State::Unsupported,
        State::ExecutionError,
        State::ScoringError,
        State::Cancelled,
    ] {
        let p = prepare(&db, &ws, "math-001");
        if state == State::ScoringError {
            completed(&db, &p, "95");
        } else {
            db.execution_finish(
                &p.execution.run,
                ExecutionPurpose::Benchmark,
                "",
                "failed",
                Some("synthetic preflight exclusion"),
                json!({}),
            )
            .unwrap();
        }
        db.save_benchmark_score(
            &ws,
            &p.summary.id,
            Score::state(state, "synthetic_expected_failure"),
        )
        .unwrap();
        let d = db.benchmark_detail(&ws, &p.summary.id).unwrap();
        assert!(d.evaluations[0]["metrics"]["correctness"].is_null());
        let stats = d.statistics.unwrap();
        assert_eq!(stats["scored"], 0);
        assert!(stats["correctness"].is_null());
        assert_eq!(stats["planned"], 1);
        assert_eq!(
            stats["executionSucceeded"],
            usize::from(state == State::ScoringError)
        );
    }
}

#[test]
fn scoring_storage_failure_keeps_original_text_and_retry_does_not_call_model() {
    let (db, ws, _) = database();
    let p = prepare(&db, &ws, "math-001");
    completed(&db, &p, "95");
    db.connection().unwrap().execute_batch("CREATE TRIGGER scoring_failure BEFORE INSERT ON benchmark_evaluations BEGIN SELECT RAISE(ABORT,'synthetic score storage failure');END;").unwrap();
    assert!(db.finish_benchmark(&ws, &p.summary.id).is_err());
    let d = db.benchmark_detail(&ws, &p.summary.id).unwrap();
    assert_eq!(d.output, "95");
    assert!(d.evaluations.is_empty());
    db.connection()
        .unwrap()
        .execute_batch("DROP TRIGGER scoring_failure")
        .unwrap();
    db.finish_benchmark(&ws, &p.summary.id).unwrap();
    let d = db.benchmark_detail(&ws, &p.summary.id).unwrap();
    assert_eq!(d.evaluations.len(), 1);
    assert_eq!(d.output, "95");
    assert_eq!(
        d.execution
            .events
            .iter()
            .filter(|e| e.kind == "model_request_started")
            .count(),
        1
    );
    assert_eq!(
        db.connection()
            .unwrap()
            .query_row("SELECT count(*) FROM knowledge_nodes", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.connection()
            .unwrap()
            .query_row("SELECT count(*) FROM retrieval_results", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    let failed = prepare(&db, &ws, "math-001");
    completed(&db, &failed, "95");
    db.connection().unwrap().execute_batch("CREATE TRIGGER failed_score_again BEFORE INSERT ON benchmark_evaluations BEGIN SELECT RAISE(ABORT,'synthetic score save failure');END;").unwrap();
    assert!(db.finish_benchmark(&ws, &failed.summary.id).is_err());
    db.benchmark_finalize_error(&ws, &failed.summary.id, "synthetic score storage failure")
        .unwrap();
    let d = db.benchmark_detail(&ws, &failed.summary.id).unwrap();
    assert_eq!(d.output, "95");
    assert!(d.evaluations.is_empty());
    assert_eq!(d.statistics.as_ref().unwrap()["states"]["Scoring Error"], 1);
    assert!(d.statistics.as_ref().unwrap()["correctness"].is_null());
}
