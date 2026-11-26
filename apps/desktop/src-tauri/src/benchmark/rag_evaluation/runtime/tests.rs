use super::*;
use crate::{
    cloud::runtime_connections::{CloudTarget, ModelLimits},
    knowledge::SaveKnowledge,
    model_runtime::http_fixture,
};

#[tokio::test]
#[ignore = "Requires explicit authorization for two real local requests"]
async fn f7_authorized_local_smoke() {
    assert_eq!(
        std::env::var("CORTEXOS_F7_APPROVED_CPU_SMOKE").as_deref(),
        Ok("1")
    );
    let owner = Arc::new(Database::rag_evaluation_database().unwrap());
    let workspace = owner.create_workspace("模型组合实机验证").unwrap().id;
    let model = "qwen3:4b-instruct";
    let digest = "0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0";
    let provider = Ollama::with_endpoint("http://127.0.0.1:11434").unwrap();
    assert!(
        provider.get("/api/ps").await.unwrap()["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    owner
        .save_settings(&ModelSettings {
            model: model.into(),
            num_ctx: 4096,
            temperature: 0.,
            num_predict: 256,
            timeout_seconds: 120,
            ..Default::default()
        })
        .unwrap();
    let preferences: crate::local_runtime::Preferences =
        serde_json::from_value(json!({"device":"cpu","keepAliveSeconds":0})).unwrap();
    preferences.save(&owner.connection().unwrap()).unwrap();
    let suite = Suite::load().unwrap();
    let input: Input = serde_json::from_value(
        json!({"arms":[{"profileId":null,"policy":"fts5"},{"profileId":null,"policy":"off"}],
        "caseId":"RAG-01","maxCostMicros":null}),
    )
    .unwrap();
    let plan = plan::prepare(&owner, &workspace, &suite, &input, Some(&provider))
        .await
        .unwrap();
    for arm in &plan.arms {
        assert_eq!(arm.local_generation.as_deref(), Some(digest));
    }
    let id = new_id();
    let (_tx, cancel) = watch::channel(false);
    let report = run(
        owner,
        &workspace,
        &id,
        &suite,
        &plan,
        Some(&provider),
        cancel,
    )
    .await
    .unwrap();
    if let Some(path) = std::env::var_os("CORTEXOS_F7_REPORT_FILE") {
        crate::benchmark::report_transfer::write_new(
            std::path::Path::new(&path),
            &serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
    for (i, row) in report["results"].as_array().unwrap().iter().enumerate() {
        let case = &row["cases"][0];
        assert_eq!(case["status"], "completed", "{}", case["error"]);
        assert_eq!(
            case["execution"]["snapshot"]["request"]["options"]["num_gpu"],
            0
        );
        assert_eq!(
            case["execution"]["snapshot"]["request"]["options"]["num_ctx"],
            4096
        );
        assert_eq!(
            case["execution"]["snapshot"]["request"]["options"]["num_predict"],
            256
        );
        assert_eq!(case["execution"]["snapshot"]["request"]["keep_alive"], 0);
        assert_eq!(
            case["metrics"]["contextRecall"],
            if i == 0 { 1. } else { 0. }
        );
        println!(
            "{} {}",
            i,
            json!({"metrics":case["metrics"],"usage":row["usage"],"status":case["status"]})
        );
    }
}

#[tokio::test]
async fn rag_runtime_runs_real_pipeline_with_loopback_replies_and_isolated_reports() {
    let owner = Arc::new(Database::rag_evaluation_database().unwrap());
    let workspace = owner.create_workspace("对照工作区").unwrap().id;
    let other = owner.create_workspace("独立工作区").unwrap().id;
    let private = owner
        .save_knowledge(SaveKnowledge {
            workspace_id: workspace.clone(),
            node_id: None,
            expected_revision: None,
            title: "PRIVATE_DATA".into(),
            kind: "knowledge".into(),
            content: "PRIVATE_KNOWLEDGE_NEVER_SENT".into(),
            change_reason: "test".into(),
            source_message_id: None,
            source_excerpt: None,
        })
        .unwrap();
    owner.approve_knowledge(&workspace, &private, 1).unwrap();
    let suite = Suite::load().unwrap();
    let recorded_model = owner.settings().unwrap().model;
    let fixed_model = recorded_model.clone();
    let (url, server) = http_fixture::serve(7, move |i, path, body| {
        assert!(!body.to_string().contains("PRIVATE_KNOWLEDGE_NEVER_SENT"));
        match path {
            "/api/tags" => (
                200,
                json!({"models":[{"name":fixed_model,"digest":"c".repeat(64)}]}),
            ),
            "/api/show" => (
                200,
                json!({"details":{"format":"gguf"},"capabilities":["completion"]}),
            ),
            "/api/chat" => {
                let reached = body.to_string().contains("日志保留 90 天");
                assert_eq!(reached, i == 3);
                (
                    200,
                    json!({"done":true,"message":{"role":"assistant","content":if reached {"日志保留 90 天。[资料1:1-1]"} else {"资料不足，无法作答。"}},
                    "prompt_eval_count":60,"eval_count":12,"total_duration":1000000}),
                )
            }
            _ => panic!("{path}"),
        }
    });
    let provider = Ollama::with_endpoint(url.as_str()).unwrap();
    let input: Input = serde_json::from_value(
        json!({"arms":[{"profileId":null,"policy":"fts5"},{"profileId":null,"policy":"off"}],
        "caseId":"RAG-01","maxCostMicros":null}),
    )
    .unwrap();
    let plan = plan::prepare(&owner, &workspace, &suite, &input, Some(&provider))
        .await
        .unwrap();
    assert_eq!(plan.audit["arms"][0]["settings"]["model"], recorded_model);
    assert_eq!(
        plan.audit["arms"][0]["localGenerationDigest"],
        "c".repeat(64)
    );
    let id = new_id();
    let (_tx, rx) = watch::channel(false);
    let report = run(
        owner.clone(),
        &workspace,
        &id,
        &suite,
        &plan,
        Some(&provider),
        rx,
    )
    .await
    .unwrap();
    assert_eq!(report["status"], "completed", "{report}");
    let a = &report["results"][0];
    let b = &report["results"][1];
    assert_eq!(a["cases"][0]["metrics"]["answerQuality"], 1.);
    assert_eq!(a["cases"][0]["metrics"]["citationPrecision"], 1.);
    assert_eq!(b["cases"][0]["metrics"]["answerQuality"], 0.);
    assert_eq!(b["cases"][0]["metrics"]["contextRecall"], 0.);
    assert_eq!(a["usage"]["localInputTokens"], 60);
    assert_eq!(a["usage"]["localOutputTokens"], 12);
    assert!(
        a["cases"][0]["execution"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "model_request_started")
    );
    assert!(read(&owner, &other, Some(&id)).is_err());
    assert!(persist(&owner, &workspace, &report, "running").is_err());
    let c = owner.connection().unwrap();
    let counts: (i64, i64, i64) = c
        .query_row(
            "SELECT (SELECT COUNT(*) FROM knowledge_nodes),
        (SELECT COUNT(*) FROM execution_runs),(SELECT COUNT(*) FROM conversations)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 0, 0));
    server.join().unwrap();
}

#[tokio::test]
async fn rag_runtime_cloud_budget_is_shared_across_arms() {
    let owner = Arc::new(Database::rag_evaluation_database().unwrap());
    let workspace = owner.create_workspace("预算测试").unwrap().id;
    let suite = Suite::load().unwrap();
    let case = suite.cases.iter().find(|c| c.id == "RAG-05").unwrap();
    let (url, server) = http_fixture::serve(1, |_, path, body| {
        assert_eq!(path, "/chat/completions");
        assert!(body.to_string().contains("日志保留 30 天"));
        assert!(!body.to_string().contains("日志保留 90 天"));
        (
            200,
            json!({"choices":[{"index":0,"message":{"role":"assistant","content":"日志保留 30 天。[资料1:1-1]"},"finish_reason":"stop"}],
           "usage":{"prompt_tokens":60,"completion_tokens":12}}),
        )
    });
    let mut target = CloudTarget::loopback_test(url, "test");
    target.connection.limits = vec![ModelLimits {
        model: "test".into(),
        context_tokens: Some(4096),
        max_input_tokens: None,
        max_output_tokens: Some(512),
        input_micro_usd_per_million: None,
        output_micro_usd_per_million: None,
        micro_usd_per_request: Some(10),
    }];
    let profile = serde_json::from_value(
        json!({"id":"profile","updatedAt":now(),"name":"测试","mode":"cloud",
        "generation":{"kind":"cloud","connectionId":target.connection.id,"model":"test"},
        "embedding":{"kind":"off"},"reranking":{"kind":"off"}}),
    )
    .unwrap();
    let targets = CloudTargets {
        generation: Some(target),
        embedding: None,
        reranking: None,
    };
    let date = case.fact_date.as_deref().unwrap();
    let corpus = execute::Corpus::open(&suite, date, &targets, Default::default()).unwrap();
    let arm = Arm {
        profile,
        policy: Policy::VersionAware,
        settings: ModelSettings {
            model: "test".into(),
            ..ModelSettings::default()
        },
        local_generation: None,
        local_embedding: None,
        audit: Value::Null,
    };
    let id = new_id();
    let (_tx, rx) = watch::channel(false);
    for (side, expected) in ["completed", "execution_error"].into_iter().enumerate() {
        let run_id = new_id();
        let unit = format!("{side}/{}", case.id);
        let meter = Meter::fixed_rag_experiment(
            owner.clone(),
            &workspace,
            corpus.database.clone(),
            WORKSPACE,
            &id,
            &unit,
            &run_id,
            Some(10),
        );
        let row = execute::case(
            &corpus,
            &arm,
            &targets,
            None,
            case,
            date,
            &run_id,
            &meter,
            rx.clone(),
        )
        .await
        .unwrap();
        assert_eq!(row["status"], expected, "{row}");
        if side == 0 {
            assert_eq!(row["metrics"]["citationPrecision"], 1.);
            assert_eq!(row["usedReferences"][0]["version"], 1);
        } else {
            assert!(row["error"].as_str().unwrap().contains("费用"));
            assert!(
                row["execution"]["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["kind"] != "model_request_started")
            );
        }
    }
    let c = owner.connection().unwrap();
    let calls = cloud_calls(&c, &workspace, &id).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["settledMicros"], 10);
    assert_eq!(
        calls[0]["dataTypes"],
        json!(["question", "granted_knowledge"])
    );
    server.join().unwrap();
}

#[tokio::test]
async fn rag_runtime_cloud_semantic_and_rerank_share_fixed_corpus() {
    let owner = Arc::new(Database::rag_evaluation_database().unwrap());
    let workspace = owner.create_workspace("云端回环对照").unwrap().id;
    let suite = Suite::load().unwrap();
    let case = &suite.cases[0];
    let (url, server) = http_fixture::serve(5, |i, path, body| {
        let raw = body.to_string();
        assert!(!raw.contains("旧无线 PIN") && !raw.contains("日志保留 30 天"));
        match (i, path) {
            (0..=2, "/embeddings") => {
                assert_eq!(body["model"], "fixed-embedding");
                let input = body["input"].as_array().unwrap();
                let data=input.iter().enumerate().map(|(index,entry)|json!({"index":index,
                    "embedding":if entry.as_str().unwrap().contains("日志保留") {[1,0]} else {[0,1]}})).collect::<Vec<_>>();
                (200, json!({"data":data,"usage":{"prompt_tokens":20}}))
            }
            (3, "/rerank") => {
                assert_eq!(body["model"], "fixed-rerank");
                assert_eq!(body["query"], "日志保留期限是多少天？");
                let scores=body["documents"].as_array().unwrap().iter().enumerate().map(|(index,entry)|json!({"index":index,
                    "relevance_score":if entry.as_str().unwrap().contains("日志保留 90 天") {0.9} else {0.1}})).collect::<Vec<_>>();
                (
                    200,
                    json!({"results":scores,"meta":{"billed_units":{"input_tokens":20}}}),
                )
            }
            (4, "/chat/completions") => {
                assert_eq!(body["model"], "fixed-generation");
                assert!(raw.contains("日志保留 90 天"));
                (
                    200,
                    json!({"choices":[{"index":0,"message":{"role":"assistant","content":"日志保留 90 天。[资料1]"},"finish_reason":"stop"}],
                    "usage":{"prompt_tokens":60,"completion_tokens":12}}),
                )
            }
            _ => panic!("{i}: {path}"),
        }
    });
    let make = |model: &str| {
        let mut target = CloudTarget::loopback_test(url.clone(), model);
        target.connection.limits = vec![ModelLimits {
            model: model.into(),
            context_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: Some(512),
            input_micro_usd_per_million: None,
            output_micro_usd_per_million: None,
            micro_usd_per_request: Some(10),
        }];
        target
    };
    let (generation, embedding, reranking) = (
        make("fixed-generation"),
        make("fixed-embedding"),
        make("fixed-rerank"),
    );
    let profile=serde_json::from_value(json!({"id":"fixture","updatedAt":now(),"name":"回环方案","mode":"cloud",
        "generation":{"kind":"cloud","model":generation.model,"connectionId":generation.connection.id},
        "embedding":{"kind":"cloud","model":embedding.model,"connectionId":embedding.connection.id},
        "reranking":{"kind":"cloud","model":reranking.model,"connectionId":reranking.connection.id}})).unwrap();
    let targets = CloudTargets {
        generation: Some(generation),
        embedding: Some(embedding),
        reranking: Some(reranking),
    };
    let arm = Arm {
        profile,
        policy: Policy::HybridRerank,
        settings: ModelSettings {
            model: "fixed-generation".into(),
            ..ModelSettings::default()
        },
        local_generation: None,
        local_embedding: None,
        audit: Value::Null,
    };
    let corpus =
        execute::Corpus::open(&suite, &suite.snapshot_date, &targets, Default::default()).unwrap();
    let id = new_id();
    let run_id = new_id();
    let (_tx, rx) = watch::channel(false);
    let meter = |unit: &str| {
        Meter::fixed_rag_experiment(
            owner.clone(),
            &workspace,
            corpus.database.clone(),
            WORKSPACE,
            &id,
            unit,
            &run_id,
            Some(50),
        )
    };
    execute::index(&corpus, &arm, &targets, None, &meter("0/index"), rx.clone())
        .await
        .unwrap();
    let result = execute::case(
        &corpus,
        &arm,
        &targets,
        None,
        case,
        &suite.snapshot_date,
        &run_id,
        &meter("0/RAG-01"),
        rx,
    )
    .await
    .unwrap();
    assert_eq!(result["status"], "completed", "{result}");
    assert_eq!(result["usedMethod"], "hybrid_rerank");
    assert_eq!(result["metrics"]["citationPrecision"], 1.);
    let c = owner.connection().unwrap();
    let totals = report_usage(&c, &workspace, &id, 0).unwrap();
    assert_eq!(totals["cloudEmbeddingCalls"], 3);
    assert_eq!(totals["cloudRerankingCalls"], 1);
    assert_eq!(totals["settledMicros"], 50);
    server.join().unwrap();
}

#[test]
fn rag_runtime_score_keeps_bad_citations_independent_of_correct_facts() {
    let suite = Suite::load().unwrap();
    let case = &suite.cases[0];
    let manifest = json!({"citationProtocol":"knowledge_lines_v1","retrieval":{"candidates":[]},"knowledgeRefs":[
        {"citationId":"资料1","nodeId":"retention","version":2,"content":"日志保留 90 天。\n其他文字"},
        {"citationId":"资料2","nodeId":"retention","version":1,"content":"日志保留 30 天。"}
    ]});
    for (answer, precision) in [
        ("**日志保留 90 天。** [资料1:1-1]", Some(1.)),
        ("日志保留期限是90天[资料1]。", Some(1.)),
        ("日志保留期为90天。[资料1:1-1]", Some(1.)),
        ("日志保留天数：90天[资料1:2-2]。", Some(0.)),
        ("日志保留 90 天。[资料1:2-2]", Some(0.)),
        ("日志保留 90 天。[资料2]", Some(0.)),
        ("日志保留 90 天。[资料3]", Some(0.)),
        (
            "日志保留期限是90天。\n日志保留期限是90天[资料1]是误传。",
            Some(0.),
        ),
        ("日志保留 90 天。", None),
    ] {
        let row = score::grade(case, &manifest, Some(answer)).unwrap();
        assert_eq!(row["metrics"]["answerQuality"], 1.);
        assert_eq!(
            row["metrics"]["citationPrecision"],
            json!(precision),
            "{answer}"
        );
        assert_eq!(
            row["metrics"]["citationRecall"],
            json!(precision.unwrap_or(0.))
        );
    }
    for answer in [
        "日志保留期限不是90天[资料1]。",
        "日志保留期限可能是90天[资料1]。",
        "日志保留期限是190天[资料1]。",
        "日志保留期限是90天或30天[资料1]。",
        "日志保留期限是90天[资料1]是误传。",
    ] {
        let row = score::grade(case, &manifest, Some(answer)).unwrap();
        assert_eq!(row["metrics"]["answerQuality"], 0., "{answer}");
        assert_eq!(row["metrics"]["citationRecall"], 0., "{answer}");
    }
    let history = suite.cases.iter().find(|case| case.id == "RAG-05").unwrap();
    let historical = score::grade(history, &manifest, Some("日志保留期限是30天[资料2]。")).unwrap();
    assert_eq!(historical["metrics"]["answerQuality"], 1.);
    assert_eq!(historical["metrics"]["citationRecall"], 1.);
    let wrong_date = score::grade(history, &manifest, Some("日志保留期限是90天[资料1]。")).unwrap();
    assert_eq!(wrong_date["metrics"]["answerQuality"], 0.);
    assert!(score::grade(case, &manifest, None).unwrap()["metrics"]["answerQuality"].is_null());
}

#[test]
#[ignore = "Offline verification of the saved F7 acceptance artifact"]
fn f7_offline_rescore() {
    let source = std::env::var_os("CORTEXOS_F7_OFFLINE_SOURCE").unwrap();
    let destination = std::env::var_os("CORTEXOS_F7_OFFLINE_REPORT").unwrap();
    let bytes = std::fs::read(source).unwrap();
    let saved: Value = serde_json::from_slice(&bytes).unwrap();
    let suite = Suite::load().unwrap();
    assert_eq!(saved["dataset"], json!(suite));
    assert_eq!(saved["plan"]["dataset"], suite.metadata());
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["plan"]["scorer"], "cortex-rag-response-rubric-1");
    assert_eq!(saved["plan"]["caseIds"], json!(["RAG-01"]));
    let case = suite.cases.iter().find(|case| case.id == "RAG-01").unwrap();
    let rows = saved["results"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let results = rows
        .iter()
        .zip([("fts5", 1.), ("off", 0.)])
        .map(|(row, (strategy, expected))| {
            assert_eq!(row["arm"]["strategy"], strategy);
            let prior = &row["cases"][0];
            assert_eq!(prior["status"], "completed");
            assert_eq!(prior["id"], case.id);
            let grade = score::grade(
                case,
                &prior["execution"]["contextManifest"],
                Some(prior["answer"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(grade["metrics"]["answerQuality"], expected);
            assert_eq!(grade["metrics"]["citationRecall"], expected);
            json!({"strategy":strategy,"caseId":case.id,"answer":prior["answer"],
                "previousMetrics":prior["metrics"],"metrics":grade["metrics"],"responseScoring":grade["responseScoring"]})
        })
        .collect::<Vec<_>>();
    let audit = json!({"mode":"offline_rescore","sourceExperimentId":saved["id"],
        "sourceReportHash":hash(&bytes),"previousScorer":saved["plan"]["scorer"],
        "scorer":score::RUNTIME_VERSION,"modelCalls":0,"results":results});
    crate::benchmark::report_transfer::write_new(
        std::path::Path::new(&destination),
        &serde_json::to_vec_pretty(&audit).unwrap(),
    )
    .unwrap();
}
