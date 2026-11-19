use super::*;

fn case(report: &Value, side: usize, id: &str) -> Value {
    report["results"][side]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap()
        .clone()
}

#[test]
fn rag_evaluation_runs_real_strategies_and_retains_isolated_history() {
    let db = Database::rag_evaluation_database().unwrap();
    let workspace = db.create_workspace("评测 A").unwrap().id;
    let other = db.create_workspace("评测 B").unwrap().id;
    assert!(run_and_save(&db, &workspace, Policy::Fts5, Policy::Fts5).is_err());
    let first = run_and_save(&db, &workspace, Policy::Fts5, Policy::Graph).unwrap();
    assert_eq!(first["dataset"]["caseCount"], 9);
    assert_eq!(first["generation"], "not_run");
    assert_eq!(first["llmJudge"], "not_run");
    let base = case(&first, 0, "RAG-09");
    let graph = case(&first, 1, "RAG-09");
    assert_eq!(base["metrics"]["contextRecall"], 1.0 / 3.0);
    assert_eq!(graph["metrics"]["contextRecall"], 1.0, "{graph}");
    assert_eq!(graph["metrics"]["citationRecall"], 1.0);
    assert_eq!(graph["metrics"]["ndcg"], 1.0);
    assert_eq!(
        graph["graph"]["paths"][1]["steps"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(case(&first, 0, "RAG-06")["usedReferences"], json!([]));
    assert_eq!(case(&first, 1, "RAG-07")["detectedConflict"], true);
    assert_eq!(
        case(&first, 1, "RAG-08")["metrics"]["recallAt5"],
        Value::Null
    );
    let second = run_and_save(&db, &workspace, Policy::HybridRerank, Policy::VersionAware).unwrap();
    assert_eq!(first["dataset"], second["dataset"]);
    assert_ne!(first["id"], second["id"]);
    assert_eq!(case(&second, 0, "RAG-02")["metrics"]["recallAt5"], 1.0);
    assert_eq!(
        case(&second, 0, "RAG-02")["candidates"][0]["semanticRank"],
        1
    );
    assert_eq!(
        case(&second, 0, "RAG-02")["candidates"][0]["lexicalRank"],
        Value::Null
    );
    let dated = case(&second, 1, "RAG-05");
    assert_eq!(dated["usedReferences"][0]["version"], 1);
    assert_eq!(
        dated["usedReferences"][0]["sourceEvidence"][0]["version"],
        1
    );
    assert_eq!(dated["metrics"]["faithfulness"], 1.0);
    assert_eq!(
        case(&first, 0, "RAG-05")["metrics"]["citationPrecision"],
        0.0
    );
    let baseline = compare(&Suite::load().unwrap(), Policy::Off, Policy::Hybrid).unwrap();
    assert_eq!(case(&baseline, 0, "RAG-01")["metrics"]["mrr"], 0.0);
    assert_eq!(
        case(&baseline, 0, "RAG-01")["metrics"]["retrievalLatencyMs"],
        Value::Null
    );
    assert_eq!(case(&baseline, 1, "RAG-02")["usedMethod"], "hybrid");

    let mut suite = Suite::load().unwrap();
    let unanswerable = suite
        .cases
        .iter_mut()
        .find(|item| item.id == "RAG-08")
        .unwrap();
    unanswerable.replay.abstained = false;
    unanswerable
        .replay
        .claims
        .push(("存在一份实际上没有的资料。".into(), 1));
    let wrong = score::evaluate(unanswerable, &json!({"citationProtocol":"knowledge_lines_v1","knowledgeRefs":[],"retrieval":{"candidates":[]}})).unwrap();
    assert_eq!(wrong["metrics"]["abstentionAccuracy"], 0);
    assert_eq!(wrong["metrics"]["citationPrecision"], 0.0);
    assert_eq!(wrong["metrics"]["faithfulness"], 0.0);
    assert!(wrong["replay"]["checks"][0]["locationError"].is_string());

    let again = run_and_save(&db, &workspace, Policy::Fts5, Policy::Graph).unwrap();
    let stable = |row: &Value| {
        let mut row = row.clone();
        row["metrics"]
            .as_object_mut()
            .unwrap()
            .remove("retrievalLatencyMs");
        row
    };
    for side in 0..2 {
        let rows = |report: &Value| {
            report["results"][side]["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(stable)
                .collect::<Vec<_>>()
        };
        assert_eq!(rows(&first), rows(&again));
    }
    let id = first["id"].as_str().unwrap();
    let history = snapshot(&db, &workspace, Some(id)).unwrap();
    assert_eq!(history["report"], first);
    assert_eq!(history["history"].as_array().unwrap().len(), 3);
    assert_eq!(snapshot(&db, &other, None).unwrap()["history"], json!([]));
    assert!(snapshot(&db, &other, Some(id)).is_err());
    let c = db.connection().unwrap();
    assert!(
        c.execute(
            "UPDATE rag_evaluations SET body_json='{}' WHERE id=?1",
            [id]
        )
        .is_err()
    );
    assert!(
        c.execute("DELETE FROM rag_evaluations WHERE id=?1", [id])
            .is_err()
    );
    let rows: i64 = c
        .query_row(
            "SELECT (SELECT count(*) FROM knowledge_nodes) + (SELECT count(*) FROM conversations) +
        (SELECT count(*) FROM execution_runs) + (SELECT count(*) FROM benchmark_runs)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}
