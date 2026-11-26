use super::*;
use crate::benchmark::preflight::{Catalog, Parameters};

fn database() -> (Database, String) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-slice3")
        .join(new_id());
    let db = Database::open(&path).unwrap();
    let workspace = db.create_workspace("synthetic slice3 fixture").unwrap().id;
    (db, workspace)
}
fn intent(db: &Database, workspace: &str, case: &str) -> Intent {
    db.benchmark_intent(
        workspace,
        StartInput {
            local_keep_alive_seconds: None,
            personalized_dataset: None,
            dataset_version: 1,
            python_image: None,
            launch_key: new_id(),
            development_only: true,
            dataset_hash: db.benchmark_datasets(workspace).unwrap()[0]
                .content_hash
                .clone(),
            case_id: case.into(),
            model: ModelRequest {
                model: "synthetic-fixture".into(),
                expected_digest: Some("a".repeat(64)),
                parameters: Parameters {
                    num_ctx: 4096,
                    num_predict: 512,
                    temperature: 0.3,
                    timeout_seconds: 30,
                },
                requirements: vec!["completion".into()],
            },
        },
    )
    .unwrap()
}
fn prepare(db: &Database, workspace: &str, intent: &Intent) -> AppResult<Prepared> {
    // Synthetic metadata, not a real Provider check or an experimental result.
    let inspection = Preflight {
        status: "ready_for_probe",
        checked_at: now(),
        request: intent.input.model.clone(),
        catalog: Catalog {
            provider_id: "ollama",
            checked_at: now(),
            version: Some("synthetic-fixture".into()),
            models: vec![],
            loaded: Some(vec![]),
            warnings: vec![],
        },
        inspection: None,
        issues: vec![],
    };
    db.prepare_benchmark(workspace, &new_id(), intent, &inspection)
}

#[test]
fn benchmark_development_prepare_rolls_back_and_isolates_purposes() {
    let (db, workspace) = database();
    let chat = db.create_conversation(&workspace).unwrap();
    let before = serde_json::to_value(db.bootstrap().unwrap()).unwrap();
    let intent = intent(&db, &workspace, "math-001");
    db.connection().unwrap().execute_batch("CREATE TRIGGER fail_development_snapshot BEFORE INSERT ON context_snapshots BEGIN SELECT RAISE(ABORT,'synthetic disk failure'); END;").unwrap();
    assert!(prepare(&db, &workspace, &intent).is_err());
    for table in [
        "benchmark_runs",
        "benchmark_case_runs",
        "benchmark_attempts",
        "execution_runs",
        "messages",
        "context_snapshots",
    ] {
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        serde_json::to_value(db.bootstrap().unwrap()).unwrap(),
        before
    );
    db.connection()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_development_snapshot")
        .unwrap();
    let prepared = prepare(&db, &workspace, &intent).unwrap();
    let run = &prepared.execution.run;
    assert!(
        db.execution_checkpoint(run, ExecutionPurpose::Chat, "bad", None)
            .is_err()
    );
    let mut forged = run.clone();
    forged.workspace_id = "other".into();
    assert!(
        db.execution_event(&forged, ExecutionPurpose::Benchmark, "bad", json!({}))
            .is_err()
    );
    assert!(db.run_detail(&workspace, &run.id).is_err());
    assert!(db.messages(&workspace, &run.conversation_id).is_err());
    assert_eq!(db.conversations(&workspace).unwrap().len(), 1);
    assert_eq!(db.conversations(&workspace).unwrap()[0].id, chat.id);
    let detail = db
        .benchmark_detail(&workspace, &prepared.summary.id)
        .unwrap();
    assert_eq!(
        detail.execution.snapshot.as_ref().unwrap().request,
        prepared.execution.request
    );
    assert_eq!(
        detail.execution.snapshot.as_ref().unwrap().request_hash,
        dataset::hash(encode(&prepared.execution.request).unwrap().as_bytes())
    );
    assert_eq!(detail.attempt["requestDisposition"], "not_sent");
    assert_eq!(detail.execution.context_manifest["retrievalUsed"], false);
    assert_eq!(detail.execution.context_manifest["historyUsed"], false);
    assert_eq!(
        detail.execution.snapshot.as_ref().unwrap().request["messages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(db.benchmark_detail("other", &prepared.summary.id).is_err());
}

#[test]
fn benchmark_development_scores_saved_output_once_and_ignores_late_writes() {
    let (db, workspace) = database();
    for (case, output, reason, correctness, review) in [
        ("math-001", " 95\n", "stop", json!(1), false),
        ("math-001", "94", "stop", json!(0), false),
        ("math-001", "+95", "stop", json!(0), false),
        ("math-001", "95 apples", "stop", json!(0), false),
        ("math-001", "95", "length", Value::Null, true),
        ("zh-006", "100\n", "stop", json!(1), false),
    ] {
        let intent = intent(&db, &workspace, case);
        let prepared = prepare(&db, &workspace, &intent).unwrap();
        let run = &prepared.execution.run;
        db.execution_event(
            run,
            ExecutionPurpose::Benchmark,
            "model_request_started",
            json!({"source":"synthetic"}),
        )
        .unwrap();
        db.execution_checkpoint(run, ExecutionPurpose::Benchmark, "partial", None)
            .unwrap();
        db.execution_finish(
            run,
            ExecutionPurpose::Benchmark,
            output,
            None,
            "completed",
            None,
            json!({"finishReason":reason}),
        )
        .unwrap();
        db.finish_benchmark(&workspace, &prepared.summary.id)
            .unwrap();
        let before = serde_json::to_value(
            db.benchmark_detail(&workspace, &prepared.summary.id)
                .unwrap(),
        )
        .unwrap();
        db.finish_benchmark(&workspace, &prepared.summary.id)
            .unwrap();
        db.execution_checkpoint(run, ExecutionPurpose::Benchmark, "late", None)
            .unwrap();
        db.execution_event(run, ExecutionPurpose::Benchmark, "late", json!({}))
            .unwrap();
        db.execution_finish(
            run,
            ExecutionPurpose::Benchmark,
            "late",
            None,
            "failed",
            None,
            json!({}),
        )
        .unwrap();
        let detail = db
            .benchmark_detail(&workspace, &prepared.summary.id)
            .unwrap();
        assert_eq!(serde_json::to_value(&detail).unwrap(), before);
        assert_eq!(detail.output, output);
        assert_eq!(detail.evaluations.len(), 1);
        assert_eq!(detail.evaluations[0]["metrics"]["correctness"], correctness);
        assert_eq!(
            detail.evaluations[0]["status"],
            if review { "needs_review" } else { "completed" }
        );
        assert_eq!(
            detail.evaluations[0]["outputHash"],
            dataset::hash(output.as_bytes())
        );
        assert_eq!(detail.attempt["requestDisposition"], "finished");
        assert_eq!(
            db.benchmark_existing(&workspace, &intent)
                .unwrap()
                .unwrap()
                .id,
            prepared.summary.id
        );
        let mut changed = intent.input.clone();
        changed.model.parameters.temperature = 0.8;
        let changed = db.benchmark_intent(&workspace, changed).unwrap();
        assert!(db.benchmark_existing(&workspace, &changed).is_err());
    }
    for table in [
        "knowledge_nodes",
        "context_knowledge_items",
        "retrieval_results",
    ] {
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn benchmark_development_preserves_output_when_score_storage_fails() {
    let (db, workspace) = database();
    let intent = intent(&db, &workspace, "math-001");
    let prepared = prepare(&db, &workspace, &intent).unwrap();
    db.execution_finish(
        &prepared.execution.run,
        ExecutionPurpose::Benchmark,
        "95",
        None,
        "completed",
        None,
        json!({"finishReason":"stop"}),
    )
    .unwrap();
    db.connection().unwrap().execute_batch("CREATE TRIGGER fail_evaluation BEFORE INSERT ON benchmark_evaluations BEGIN SELECT RAISE(ABORT,'synthetic score write failure'); END;").unwrap();
    assert!(
        db.finish_benchmark(&workspace, &prepared.summary.id)
            .is_err()
    );
    let detail = db
        .benchmark_detail(&workspace, &prepared.summary.id)
        .unwrap();
    assert_eq!(detail.output, "95");
    assert_eq!(detail.execution.run.status, "completed");
    assert!(detail.evaluations.is_empty());
    assert_eq!(detail.summary.status, "running");
    db.benchmark_finalize_error(
        &workspace,
        &prepared.summary.id,
        "synthetic score write failure",
    )
    .unwrap();
    let detail = db
        .benchmark_detail(&workspace, &prepared.summary.id)
        .unwrap();
    assert_eq!(detail.summary.status, "failed");
    assert_eq!(detail.attempt["status"], "completed");
    assert_eq!(detail.output, "95");
    assert!(detail.evaluations.is_empty());
    assert_eq!(detail.events.last().unwrap()["kind"], "result_save_failed");
}
