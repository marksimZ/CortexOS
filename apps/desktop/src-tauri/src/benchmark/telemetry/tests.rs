use super::*;
use crate::{
    benchmark::{
        preflight::{ModelRequest, Parameters},
        repository::StartInput,
    },
    models::{ExecutionPurpose, new_id},
};

#[test]
fn telemetry_keeps_missing_distinct_from_zero_and_pairs_speed_samples() {
    let empty = measure(&json!({}), "completed", None, resource_summary(&[]));
    let measured = measure(
        &json!({"promptEvalCount":0,"evalCount":10,"evalDurationNs":2_000_000_000u64,"loadDurationNs":0,"promptEvalDurationNs":50_000_000,"clientTiming":{"completeMs":10_000,"firstTextMs":400}}),
        "completed",
        None,
        resource_summary(&[]),
    );
    assert_eq!(measured["tokensPerSecond"], 5.);
    assert_eq!(measured["promptMs"], 50.);
    assert_eq!(measured["loadMs"], 0.);
    let missing_duration = measure(
        &json!({"evalCount":500,"clientTiming":{"totalMs":2000}}),
        "completed",
        None,
        resource_summary(&[]),
    );
    let cancelled = measure(
        &json!({"clientTiming":{"responseMs":9000,"firstTextMs":100}}),
        "cancelled",
        None,
        resource_summary(&[]),
    );
    let stats = summarize(&[empty, measured, missing_duration, cancelled], 5);
    assert_eq!(stats["tokensPerSecond"], 5.);
    assert_eq!(stats["speedCases"], 1);
    assert_eq!(stats["firstTextMs"]["n"], 1);
    assert_eq!(stats["firstTextMs"]["missing"], 4);
    assert_eq!(stats["allResponsesMs"]["n"], 3);
    assert_eq!(stats["responseMs"]["n"], 2);
    assert_eq!(stats["inputTokensObserved"], 0);
    assert_eq!(stats["inputTokenCases"], 1);
    let unknown = summarize(&[], 2);
    assert!(unknown["inputTokensObserved"].is_null());
    assert!(unknown["tokensPerSecond"].is_null());
    assert!(
        measure(
            &json!({"evalCount":10,"evalDurationNs":0}),
            "completed",
            None,
            Value::Null
        )["tokensPerSecond"]
            .is_null()
    );
    assert_eq!(distribution(vec![4., 1., 3., 2.], 4)["p50"], 2.5);
    assert_eq!(distribution(vec![4., 1., 3., 2.], 4)["p95"], 4.);
    let huge = measure(
        &json!({"evalCount":u64::MAX}),
        "completed",
        None,
        Value::Null,
    );
    assert!(summarize(&[huge.clone(), huge], 2)["outputTokensObserved"].is_null());
}

#[test]
fn telemetry_samples_are_scoped_append_only_and_survive_restart_without_rewriting_grades() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-telemetry")
        .join(new_id());
    let db = Database::open(&path).unwrap();
    let w = db.create_workspace("measurements").unwrap().id;
    let other = db.create_workspace("other").unwrap().id;
    let hash = db
        .benchmark_datasets(&w)
        .unwrap()
        .into_iter()
        .find(|d| d.version == 2)
        .unwrap()
        .content_hash;
    let intent = db
        .benchmark_intent(
            &w,
            StartInput {
                launch_key: new_id(),
                development_only: true,
                dataset_hash: hash,
                case_id: "math-001".into(),
                model: ModelRequest {
                    model: "fixture".into(),
                    expected_digest: Some("a".repeat(64)),
                    parameters: Parameters {
                        num_ctx: 4096,
                        num_predict: 128,
                        temperature: 0.,
                        timeout_seconds: 30,
                    },
                    requirements: vec!["completion".into()],
                },
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
    let sample = json!({"systemCpuPercent":12.,"systemMemory":{"usedBytes":1000,"totalBytes":2000},"gpuProbe":true,"gpuReason":"fixture_unavailable"});
    db.benchmark_resource_sample(&p.execution.run, 1, 0, sample.clone())
        .unwrap();
    assert!(
        db.benchmark_resource_sample(&p.execution.run, 1, 1, sample.clone())
            .is_err()
    );
    assert!(
        db.connection()
            .unwrap()
            .execute("UPDATE benchmark_resource_samples SET t_rel_ms=1", [])
            .is_err()
    );
    assert!(
        db.benchmark_resource_samples(&other, &p.summary.attempt_id)
            .unwrap()
            .is_empty()
    );
    let mut forged = p.execution.run.clone();
    forged.workspace_id = other;
    assert!(
        db.benchmark_resource_sample(&forged, 2, 1000, sample.clone())
            .is_err()
    );
    db.execution_finish(
        &p.execution.run,
        ExecutionPurpose::Benchmark,
        "partial",
        "cancelled",
        None,
        json!({"clientTiming":{"responseMs":42,"firstTextMs":12}}),
    )
    .unwrap();
    db.finish_benchmark_v2(&w, &p.summary.id, None).unwrap();
    let before = db.benchmark_detail(&w, &p.summary.id).unwrap();
    assert_eq!(before.resources.len(), 1);
    let resource = resource_summary(&before.resources);
    assert_eq!(resource["systemMemoryBytesPeak"], 1000.);
    assert_eq!(resource["gpuReasons"], json!(["fixture_unavailable"]));
    assert_eq!(before.hardware["schemaVersion"], 1);
    let samples = before.resources;
    let grades = before.evaluations;
    drop(db);
    let reopened = Database::open(&path)
        .unwrap()
        .benchmark_detail(&w, &p.summary.id)
        .unwrap();
    assert_eq!(reopened.resources, samples);
    assert_eq!(reopened.evaluations, grades);
    assert_eq!(reopened.execution.run.status, "cancelled");
}
