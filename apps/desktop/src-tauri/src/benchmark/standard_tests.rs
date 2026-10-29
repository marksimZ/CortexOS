use super::*;
use crate::{
    benchmark::{
        personalized::batch::{BatchInput, ModelChoice},
        preflight::{ModelRequest, Parameters},
        repository::StartInput,
    },
    models::{ExecutionPurpose, new_id},
};
use serde_json::json;
fn directory() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/standard-benchmark")
        .join(new_id())
}
fn model(name: &str) -> ModelChoice {
    ModelChoice {
        kind: "local".into(),
        local: Some(ModelRequest {
            model: name.into(),
            expected_digest: Some("a".repeat(64)),
            parameters: Parameters {
                num_ctx: 4096,
                num_predict: 512,
                temperature: 0.,
                timeout_seconds: 30,
            },
            requirements: vec!["completion".into()],
        }),
    }
}
fn input(db: &Database, w: &str) -> BatchInput {
    BatchInput {
        dataset_id: "cortex-core-60".into(),
        dataset_version: 2,
        dataset_hash: db
            .benchmark_datasets(w)
            .unwrap()
            .iter()
            .find(|d| d.version == 2)
            .unwrap()
            .content_hash
            .clone(),
        models: vec![model("synthetic-A"), model("synthetic-B")],
        max_output_tokens: 512,
        price: None,
        python_image: None,
        case_ids: vec!["math-001".into(), "zh-006".into()],
        repetitions: 2,
        release_models: vec![],
    }
}
#[test]
fn report_paging_is_scoped_bounded_lazy_and_matches_full_statistics() {
    use crate::benchmark::report_read::ReportScope;
    let db = Database::open(&directory()).unwrap();
    let w = db.create_workspace("paged report fixture").unwrap().id;
    let other = db.create_workspace("other").unwrap().id;
    let mut options = input(&db, &w);
    options.case_ids = vec!["math-001".into()];
    options.repetitions = 5;
    let plan = db.plan_standard_batch(&w, options.clone()).unwrap();
    let id = plan["id"].as_str().unwrap();
    db.begin_standard_batch(&w, id, plan["planHash"].as_str().unwrap())
        .unwrap();
    db.set_batch_item(id, 0, "running", None, None).unwrap();
    let intent = db
        .benchmark_intent(
            &w,
            StartInput {
                launch_key: format!("{id}:0"),
                development_only: true,
                dataset_hash: options.dataset_hash,
                case_id: "math-001".into(),
                model: options.models[0].local.clone().unwrap(),
                dataset_version: 2,
                python_image: None,
                personalized_dataset: None,
            },
        )
        .unwrap();
    let p = db
        .prepare_benchmark_record(
            &w,
            &new_id(),
            &intent,
            &json!({"request":intent.input.model}),
        )
        .unwrap();
    db.execution_event(
        &p.execution.run,
        ExecutionPurpose::Benchmark,
        "model_request_started",
        json!({"fixture":true}),
    )
    .unwrap();
    let output = format!("SAVED_BODY_MARKER{}", "x".repeat(200_000));
    db.execution_finish(&p.execution.run,ExecutionPurpose::Benchmark,&output,"completed",None,json!({"evalCount":2,"promptEvalCount":3,"evalDurationNs":100000000,"clientTiming":{"responseMs":500,"firstTextMs":20}})).unwrap();
    db.finish_benchmark_v2(&w, &p.summary.id, None).unwrap();
    db.set_batch_item(id, 0, "finished", Some(&p.summary.id), None)
        .unwrap();
    let full = db.standard_report(&w, id).unwrap();
    let compact = db
        .benchmark_overview(&w, id, ReportScope::Standard)
        .unwrap();
    assert_eq!(compact["pagedResults"], true);
    assert!(compact.get("items").is_none());
    assert!(!compact.to_string().contains("SAVED_BODY_MARKER"));
    for i in 0..2 {
        assert_eq!(
            compact["reports"][i]["statistics"],
            full["reports"][i]["statistics"]
        );
        assert_eq!(
            compact["reports"][i]["performance"],
            full["reports"][i]["performance"]
        );
        assert!(
            compact["reports"][i]["results"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    let location = db
        .benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            Some(0),
            Some(&p.execution.run.id),
            2,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(location["ordinal"], 0);
    assert_eq!(location["page"], 0);
    assert_eq!(location["matchesFilter"], true);
    assert_eq!(
        db.benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            None,
            Some(&p.execution.run.id),
            2,
            &Default::default()
        )
        .unwrap(),
        location
    );
    for (workspace, scope, ordinal, execution) in [
        (&other, ReportScope::Standard, Some(0), None),
        (&w, ReportScope::Personalized, Some(0), None),
        (
            &w,
            ReportScope::Standard,
            Some(1),
            Some(p.execution.run.id.as_str()),
        ),
        (&w, ReportScope::Standard, Some(999), None),
        (&w, ReportScope::Standard, None, Some("unknown-execution")),
    ] {
        assert!(
            db.benchmark_result_location(
                workspace,
                id,
                scope,
                ordinal,
                execution,
                2,
                &Default::default()
            )
            .is_err()
        );
    }
    assert!(
        db.benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            None,
            None,
            2,
            &Default::default()
        )
        .is_err()
    );
    assert!(
        db.benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            Some(0),
            None,
            0,
            &Default::default()
        )
        .is_err()
    );
    let last_location = db
        .benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            Some(9),
            None,
            2,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(last_location["modelIndex"], 1);
    assert_eq!(last_location["page"], 2);
    let first = db
        .benchmark_result_page(&w, id, ReportScope::Standard, 0, 0, 2)
        .unwrap();
    let second = db
        .benchmark_result_page(&w, id, ReportScope::Standard, 0, 1, 2)
        .unwrap();
    let last = db
        .benchmark_result_page(&w, id, ReportScope::Standard, 0, 2, 2)
        .unwrap();
    assert_eq!(first["total"], 5);
    assert_eq!(first["rows"][0]["state"], "Failed");
    assert_eq!(second["rows"][0]["ordinal"], 2);
    assert_eq!(last["rows"].as_array().unwrap().len(), 1);
    assert!(!first.to_string().contains("SAVED_BODY_MARKER"));
    assert!(
        db.benchmark_result_page(&w, id, ReportScope::Standard, 0, 3, 2)
            .is_err()
    );
    assert!(
        db.benchmark_result_page(&w, id, ReportScope::Standard, 2, 0, 20)
            .is_err()
    );
    assert!(
        db.benchmark_result_page(&w, id, ReportScope::Standard, 0, 0, 51)
            .is_err()
    );
    for scope in [ReportScope::Standard, ReportScope::Personalized] {
        assert!(db.benchmark_overview(&other, id, scope).is_err());
        assert!(
            db.benchmark_result_page(&other, id, scope, 0, 0, 20)
                .is_err()
        );
        assert!(db.benchmark_result_detail(&other, id, scope, 0).is_err());
    }
    assert!(
        db.benchmark_overview(&w, id, ReportScope::Personalized)
            .is_err()
    );
    assert!(
        db.benchmark_result_page(&w, id, ReportScope::Personalized, 0, 0, 20)
            .is_err()
    );
    assert!(
        db.benchmark_result_detail(&w, id, ReportScope::Personalized, 0)
            .is_err()
    );
    assert_eq!(
        db.benchmark_result_detail(&w, id, ReportScope::Standard, 0)
            .unwrap()["output"],
        output
    );
    let queued = db
        .benchmark_result_detail(&w, id, ReportScope::Standard, 1)
        .unwrap();
    assert!(queued["execution"].is_null());
    assert!(queued["output"].is_null());
    assert!(queued["case"]["input"]["prompt"].is_string());
    assert!(
        db.benchmark_result_detail(&w, id, ReportScope::Standard, 500)
            .is_err()
    );
    use crate::benchmark::report_read::ResultFilter;
    for (state, expected) in [("Failed", 1), ("Not Run", 4), ("Passed", 0)] {
        let result = db
            .benchmark_filtered_result_page(
                &w,
                id,
                ReportScope::Standard,
                0,
                0,
                20,
                &ResultFilter {
                    state: Some(state.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result["total"], expected);
        assert_eq!(result["plannedTotal"], 5);
    }
    let empty = db
        .benchmark_filtered_result_page(
            &w,
            id,
            ReportScope::Standard,
            0,
            0,
            20,
            &ResultFilter {
                state: Some("Failed".into()),
                repetition: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(empty["total"], 0);
    let excluded = db
        .benchmark_result_location(
            &w,
            id,
            ReportScope::Standard,
            Some(0),
            None,
            2,
            &ResultFilter {
                state: Some("Passed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(excluded["matchesFilter"], false);
    assert!(excluded["page"].is_null());
    assert_eq!(
        db.benchmark_overview(&w, id, ReportScope::Standard)
            .unwrap()["reports"],
        compact["reports"]
    );
    assert!(db.conversations(&w).unwrap().is_empty());
}
#[test]
fn standard_batch_freezes_model_question_repetition_plan_without_cloud_or_profile() {
    let db = Database::open(&directory()).unwrap();
    let w = db.create_workspace("standard fixture").unwrap().id;
    let other = db.create_workspace("other").unwrap().id;
    let baseline = serde_json::to_value(db.bootstrap().unwrap()).unwrap();
    let settings = input(&db, &w);
    let plan = db.plan_standard_batch(&w, settings.clone()).unwrap();
    let id = plan["id"].as_str().unwrap();
    assert_eq!(plan["items"].as_array().unwrap().len(), 8);
    assert_eq!(plan["plan"]["maxRequests"], 8);
    assert!(plan["plan"].get("binding").is_none());
    assert_eq!(plan["plan"]["profile"], Value::Null);
    assert_eq!(plan["items"][4]["modelIndex"], 1);
    assert_eq!(plan["items"][2]["caseId"], "math-001");
    assert_eq!(db.standard_batch_history(&w).unwrap().len(), 1);
    assert!(db.personalized_batch_history(&w).unwrap().is_empty());
    assert!(db.personalized_report(&w, id).is_err());
    assert!(db.standard_report(&other, id).is_err());
    assert!(db.begin_standard_batch(&w, id, "wrong").is_err());
    let report = db.standard_report(&w, id).unwrap();
    assert_eq!(report["scope"], "standard");
    assert_eq!(report["selectedCases"], 2);
    assert_eq!(report["datasetCases"], 60);
    assert_eq!(
        report["reports"][0]["statistics"]["correctness"],
        Value::Null
    );
    assert_eq!(report["reports"][0]["statistics"]["planned"], 4);
    let mut invalid = settings.clone();
    invalid.models[0] = ModelChoice {
        kind: "cloud".into(),
        local: None,
    };
    assert!(db.plan_standard_batch(&w, invalid).is_err());
    let mut invalid = settings.clone();
    invalid.case_ids.push("missing".into());
    assert!(db.plan_standard_batch(&w, invalid).is_err());
    let mut invalid = settings.clone();
    invalid.release_models = vec!["unrelated-model".into()];
    assert!(db.plan_standard_batch(&w, invalid).is_err());
    let mut invalid = settings;
    invalid.dataset_version = 1;
    assert!(db.plan_standard_batch(&w, invalid).is_err());
    assert_eq!(
        serde_json::to_value(db.bootstrap().unwrap()).unwrap(),
        baseline
    );
    assert!(db.benchmark_profile(&w).unwrap().is_none());
}
#[tokio::test]
async fn standard_restart_recovers_saved_output_score_without_new_model_request() {
    let path = directory();
    let db = Database::open(&path).unwrap();
    let w = db.create_workspace("restart fixture").unwrap().id;
    let mut options = input(&db, &w);
    options.models.truncate(1);
    options.case_ids = vec!["math-001".into()];
    options.repetitions = 1;
    let batch = db.plan_standard_batch(&w, options.clone()).unwrap();
    let id = batch["id"].as_str().unwrap().to_string();
    let hash = batch["planHash"].as_str().unwrap().to_string();
    db.begin_standard_batch(&w, &id, &hash).unwrap();
    db.set_batch_item(&id, 0, "running", None, None).unwrap();
    let request = StartInput {
        launch_key: format!("{id}:0"),
        development_only: true,
        dataset_hash: options.dataset_hash,
        case_id: "math-001".into(),
        model: options.models[0].local.clone().unwrap(),
        dataset_version: 2,
        python_image: None,
        personalized_dataset: None,
    };
    let intent = db.benchmark_intent(&w, request).unwrap();
    let prepared = db
        .prepare_benchmark_record(
            &w,
            &new_id(),
            &intent,
            &json!({"request":intent.input.model,"status":"synthetic_metadata_only"}),
        )
        .unwrap();
    let expected = match intent.case.expected {
        crate::benchmark::dataset::Expected::Integer { answer } => answer.to_string(),
        _ => panic!(),
    };
    db.execution_event(
        &prepared.execution.run,
        ExecutionPurpose::Benchmark,
        "model_request_started",
        json!({"synthetic":true}),
    )
    .unwrap();
    db.execution_finish(
        &prepared.execution.run,
        ExecutionPurpose::Benchmark,
        &expected,
        "completed",
        None,
        json!({"finishReason":"stop"}),
    )
    .unwrap();
    let run_id = prepared.summary.id.clone();
    let wire = prepared.execution.request.clone();
    drop(db);
    let state = AppState::new(&path);
    let db = state.database.as_ref().unwrap();
    assert!(db.benchmark_score_pending(&w, &run_id).unwrap());
    let before = db.standard_report(&w, &id).unwrap();
    assert_eq!(before["status"], "interrupted");
    assert_eq!(before["remainingRequests"], 0);
    assert_eq!(before["canResume"], true);
    let plan = db.begin_standard_batch(&w, &id, &hash).unwrap();
    let lease = Arc::new(state.claim_task(&id, &w, TaskKind::Benchmark).unwrap());
    batch::execute(
        state.clone(),
        CloudState::new(),
        w.clone(),
        id.clone(),
        plan,
        lease,
    )
    .await
    .unwrap();
    let after = db.standard_report(&w, &id).unwrap();
    assert_eq!(after["status"], "completed");
    assert_eq!(after["items"][0]["status"], "finished");
    assert_eq!(after["reports"][0]["statistics"]["correctness"], 1.0);
    assert_eq!(
        after["reports"][0]["results"][0]["score"]["details"]["recoveredFromSavedOutput"],
        true
    );
    let detail = db.benchmark_detail(&w, &run_id).unwrap();
    assert_eq!(detail.evaluations.len(), 1);
    assert_eq!(detail.execution.snapshot.unwrap().request, wire);
    assert_eq!(
        detail
            .execution
            .events
            .iter()
            .filter(|e| e.kind == "model_request_started")
            .count(),
        1
    );
    assert!(db.begin_standard_batch(&w, &id, &hash).is_err());
    assert!(db.conversations(&w).unwrap().is_empty());
}

#[test]
fn report_filters_count_before_paging_and_validate_frozen_metadata() {
    use crate::benchmark::report_read::{ReportScope, ResultFilter};
    let db = Database::open(&directory()).unwrap();
    let w = db.create_workspace("filter fixture").unwrap().id;
    let mut settings = input(&db, &w);
    settings.case_ids.clear();
    settings.models.truncate(1);
    settings.repetitions = 5;
    let plan = db.plan_standard_batch(&w, settings).unwrap();
    let id = plan["id"].as_str().unwrap();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../datasets/cortex-core-60-v2/cases.json")).unwrap();
    for rule in [
        "exact_alias",
        "integer_equal",
        "schema_semantic",
        "evidence_match",
        "python_assertions",
        "native_tool_equal",
    ] {
        let expected = cases.iter().filter(|c| c["scoring"]["id"] == rule).count();
        let result = db
            .benchmark_filtered_result_page(
                &w,
                id,
                ReportScope::Standard,
                0,
                0,
                2,
                &ResultFilter {
                    evaluator: Some(rule.into()),
                    repetition: Some(3),
                    state: Some("Not Run".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result["total"], expected);
        assert_eq!(result["plannedTotal"], 300);
        assert_eq!(result["rows"].as_array().unwrap().len(), 2);
        for row in result["rows"].as_array().unwrap() {
            assert_eq!(row["evaluator"], rule);
            assert_eq!(row["repetition"], 3);
        }
        let last = db
            .benchmark_filtered_result_page(
                &w,
                id,
                ReportScope::Standard,
                0,
                (expected as u32 - 1) / 2,
                2,
                &ResultFilter {
                    evaluator: Some(rule.into()),
                    repetition: Some(3),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            last["rows"].as_array().unwrap().len(),
            if expected % 2 == 0 { 2 } else { 1 }
        );
    }
    for difficulty in ["easy", "medium", "hard"] {
        let expected = cases
            .iter()
            .filter(|c| c["difficulty"] == difficulty)
            .count()
            * 5;
        let result = db
            .benchmark_filtered_result_page(
                &w,
                id,
                ReportScope::Standard,
                0,
                0,
                20,
                &ResultFilter {
                    difficulty: Some(difficulty.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result["total"], expected);
    }
    for filter in [
        ResultFilter {
            state: Some("' OR 1=1".into()),
            ..Default::default()
        },
        ResultFilter {
            evaluator: Some("invented".into()),
            ..Default::default()
        },
        ResultFilter {
            difficulty: Some("extreme".into()),
            ..Default::default()
        },
        ResultFilter {
            repetition: Some(0),
            ..Default::default()
        },
        ResultFilter {
            repetition: Some(6),
            ..Default::default()
        },
    ] {
        assert!(
            db.benchmark_filtered_result_page(&w, id, ReportScope::Standard, 0, 0, 20, &filter)
                .is_err()
        );
    }
    assert!(serde_json::from_value::<ResultFilter>(json!({"unexpected":true})).is_err());
    assert!(
        db.benchmark_filtered_result_page(
            &w,
            id,
            ReportScope::Personalized,
            0,
            0,
            20,
            &ResultFilter::default()
        )
        .is_err()
    );
    assert!(db.conversations(&w).unwrap().is_empty());
}

fn report_transfer_fixture() -> (Database, String, String, String) {
    let db = Database::open(&directory()).unwrap();
    let w = db.create_workspace("report transfer fixture").unwrap().id;
    let mut options = input(&db, &w);
    options.models.truncate(1);
    options.case_ids = vec!["math-001".into()];
    options.repetitions = 2;
    let batch = db.plan_standard_batch(&w, options.clone()).unwrap();
    let id = batch["id"].as_str().unwrap().to_string();
    db.begin_standard_batch(&w, &id, batch["planHash"].as_str().unwrap())
        .unwrap();
    let intent = db
        .benchmark_intent(
            &w,
            StartInput {
                launch_key: format!("{id}:0"),
                development_only: true,
                dataset_hash: options.dataset_hash,
                case_id: "math-001".into(),
                model: options.models[0].local.clone().unwrap(),
                dataset_version: 2,
                python_image: None,
                personalized_dataset: None,
            },
        )
        .unwrap();
    let p = db
        .prepare_benchmark_record(
            &w,
            &new_id(),
            &intent,
            &json!({"request":intent.input.model,"api_key":"SECRET-SENTINEL"}),
        )
        .unwrap();
    db.execution_event(
        &p.execution.run,
        ExecutionPurpose::Benchmark,
        "model_request_started",
        json!({"controlled_test":true}),
    )
    .unwrap();
    db.execution_finish(
        &p.execution.run,
        ExecutionPurpose::Benchmark,
        "95",
        "completed",
        None,
        json!({"finishReason":"stop"}),
    )
    .unwrap();
    db.finish_benchmark_v2(&w, &p.summary.id, None).unwrap();
    db.set_batch_item(&id, 0, "finished", Some(&p.summary.id), None)
        .unwrap();
    db.finish_batch(&w, &id, "cancelled").unwrap();
    (db, w, id, p.summary.id)
}
#[test]
fn report_transfer_regrades_are_atomic_scoped_and_leave_original_results_unchanged() {
    use crate::benchmark::{report_read::ReportScope, report_transfer::validate_report};
    let (db, w, id, run) = report_transfer_fixture();
    let other = db.create_workspace("other export workspace").unwrap().id;
    let before = db.standard_report(&w, &id).unwrap();
    let original = db.benchmark_detail(&w, &run).unwrap().evaluations[0].clone();
    let plan = db
        .preview_benchmark_regrade(&w, &id, ReportScope::Standard)
        .unwrap();
    assert_eq!(plan["eligible"], 1);
    assert_eq!(plan["plannedItems"], 2);
    let session = plan["sessionId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    assert!(
        db.regrade_benchmark(&w, &id, ReportScope::Standard, session, "bad", None)
            .is_err()
    );
    let (cancel, _) = tokio::sync::watch::channel(true);
    assert!(
        db.regrade_benchmark(
            &w,
            &id,
            ReportScope::Standard,
            session,
            hash,
            Some(cancel.subscribe())
        )
        .is_err()
    );
    assert_eq!(db.benchmark_detail(&w, &run).unwrap().evaluations.len(), 1);
    for scope in [ReportScope::Personalized] {
        assert!(db.preview_benchmark_regrade(&w, &id, scope).is_err());
        assert!(db.benchmark_export(&w, &id, scope).is_err());
    }
    assert!(
        db.preview_benchmark_regrade(&other, &id, ReportScope::Standard)
            .is_err()
    );
    assert!(
        db.benchmark_export(&other, &id, ReportScope::Standard)
            .is_err()
    );
    let result = db
        .regrade_benchmark(&w, &id, ReportScope::Standard, session, hash, None)
        .unwrap();
    assert_eq!(result["statistics"]["passed"], 1);
    assert!(
        db.regrade_benchmark(&w, &id, ReportScope::Standard, session, hash, None)
            .is_err()
    );
    let after = db.standard_report(&w, &id).unwrap();
    assert_eq!(
        before["reports"][0]["statistics"],
        after["reports"][0]["statistics"]
    );
    assert_eq!(
        before["reports"][0]["performance"],
        after["reports"][0]["performance"]
    );
    let detail = db.benchmark_detail(&w, &run).unwrap();
    assert_eq!(detail.evaluations.len(), 2);
    assert_eq!(
        detail
            .evaluations
            .iter()
            .find(|v| v["id"] == original["id"])
            .unwrap(),
        &original
    );
    assert_eq!(
        crate::benchmark::report_read::ReportRead::from_detail(&detail)
            .evaluation
            .unwrap()["id"],
        original["id"]
    );
    let history = db
        .benchmark_regrades(&w, &id, ReportScope::Standard, None, 0)
        .unwrap();
    assert_eq!(history["total"], 1);
    assert!(
        db.benchmark_regrades(&w, &id, ReportScope::Standard, Some(session), 999)
            .is_err()
    );
    assert!(
        db.benchmark_regrades(&other, &id, ReportScope::Standard, Some(session), 0)
            .is_err()
    );
    let next = db
        .preview_benchmark_regrade(&w, &id, ReportScope::Standard)
        .unwrap();
    // Simulate a concurrent terminal-state update in this isolated fixture.
    db.connection()
        .unwrap()
        .execute(
            "UPDATE benchmark_batches SET status='completed' WHERE id=?1",
            [&id],
        )
        .unwrap();
    assert!(
        db.regrade_benchmark(
            &w,
            &id,
            ReportScope::Standard,
            next["sessionId"].as_str().unwrap(),
            next["planHash"].as_str().unwrap(),
            None
        )
        .is_err()
    );
    let exported = db.benchmark_export(&w, &id, ReportScope::Standard).unwrap();
    validate_report(&exported).unwrap();
    assert_eq!(
        exported["payload"]["items"][0]["attempts"][0]["evaluations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut tampered = exported.clone();
    tampered["payload"]["scope"] = json!("changed");
    assert!(validate_report(&tampered).is_err());
    let mut legacy = input(&db, &w);
    legacy.dataset_version = 1; // A mismatched frozen plan must be rejected by the existing planner.
    assert!(db.plan_standard_batch(&w, legacy).is_err());
}
#[test]
fn report_transfer_exports_all_attempts_csv_safely_and_never_overwrites_files() {
    use crate::benchmark::report_read::ReportScope;
    use crate::benchmark::report_transfer::{
        csv_report, strip_transport, validate_report, write_new,
    };
    let (db, w, id, run) = report_transfer_fixture();
    let summary = db.benchmark_detail(&w, &run).unwrap().summary;
    db.connection().unwrap().execute("INSERT INTO benchmark_attempts(id,workspace_id,case_run_id,attempt_no,execution_run_id,status,request_disposition) VALUES(?1,?2,?3,2,NULL,'failed','unknown')",rusqlite::params![new_id(),w,summary.case_run_id]).unwrap();
    let report = db.benchmark_export(&w, &id, ReportScope::Standard).unwrap();
    validate_report(&report).unwrap();
    assert!(!report.to_string().contains("SECRET-SENTINEL"));
    assert_eq!(
        report["payload"]["items"][0]["attempts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let raw = serde_json::to_vec(&report).unwrap();
    validate_report(&serde_json::from_slice(&raw).unwrap()).unwrap();
    let mut csv_input = report.clone();
    csv_input["payload"]["items"][0]["case"]["input_json"]["prompt"] =
        json!("  =HYPERLINK(\"x\"),中文\n第二行");
    csv_input["contentHash"] = json!(crate::benchmark::dataset::hash(
        &serde_json::to_vec(&csv_input["payload"]).unwrap()
    ));
    let csv = String::from_utf8(csv_report(&csv_input).unwrap()).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains("\"'  =HYPERLINK(\"\"x\"\"),中文\n第二行\""));
    assert!(csv.contains("Not Run"));
    let mut metadata = json!({"api_key":"SECRET-SENTINEL","headers":{"Authorization":"SECRET-SENTINEL","X-Request-ID":"public"},"messages":[{"content":"原文保持完整"}]});
    strip_transport(&mut metadata);
    assert!(!metadata.to_string().contains("SECRET-SENTINEL"));
    assert_eq!(metadata["headers"]["X-Request-ID"], "public");
    let root = directory();
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("报告.json");
    write_new(&path, &raw).unwrap();
    assert!(write_new(&path, b"changed").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), raw);
    let missing = root.join("missing").join("report.json");
    assert!(write_new(&missing, &raw).is_err());
    assert!(!missing.exists());
    let directory_target = root.join("existing-directory");
    std::fs::create_dir(&directory_target).unwrap();
    assert!(write_new(&directory_target, &raw).is_err());
    assert!(directory_target.is_dir());
    assert!(
        std::fs::read_dir(&root).unwrap().all(|p| !p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
}
