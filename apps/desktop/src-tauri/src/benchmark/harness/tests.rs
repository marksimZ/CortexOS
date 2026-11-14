use super::*;
fn weights() -> Value {
    json!({"compatibility":3,"automation":3,"governance":3,"context":3})
}
fn dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-d1-rust")
        .join(new_id())
}
#[test]
fn harness_candidates_and_document_bounds_are_conservative() {
    let content = "禁止使用云端模型。\n仅在未来考虑MCP。\nWindows 环境需要人工审批。\n";
    let docs = vec![Document {
        source_id: "s".into(),
        path: "AGENTS.md".into(),
        content_hash: hash(content.as_bytes()),
        content: content.into(),
    }];
    let mut rs = extract::extract(&docs);
    assert!(rs.iter().any(|r| r.value == "禁止使用云端模型。"));
    assert!(
        rs.iter()
            .filter(|r| r.value == "仅在未来考虑MCP。")
            .all(|r| r.criterion.is_none())
    );
    assert!(
        rs.iter()
            .all(|r| !r.hard && matches!(r.status.as_str(), "candidate" | "unknown"))
    );
    assert_eq!(
        rs.iter()
            .map(|r| &r.category)
            .collect::<BTreeSet<_>>()
            .len(),
        12
    );
    let r = rs
        .iter_mut()
        .find(|r| r.criterion.as_deref() == Some("human_approval"))
        .unwrap();
    r.hard = true;
    r.status = "confirmed".into();
    let catalog = fit::catalog();
    let report = fit::assess(&rs, &weights(), &catalog);
    let oh = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["harnessId"] == "openhands")
        .unwrap();
    assert_eq!(oh["eligibility"], "excluded");
    let codex = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["harnessId"] == "codex")
        .unwrap();
    assert_eq!(codex["eligibility"], "needs_verification");
    assert_eq!(codex["verifiedCount"], 0);
    let mut repeated = rs.clone();
    repeated.extend(rs.clone());
    assert_eq!(
        fit::assess(&repeated, &weights(), &catalog)["results"][0]["weights"],
        report["results"][0]["weights"]
    );
    for r in &mut rs {
        r.status = "rejected".into();
        r.hard = false;
    }
    assert!(fit::assess(&rs, &weights(), &catalog)["results"][0]["lowerPercent"].is_null());
}
#[test]
fn harness_sources_versions_and_scope_preserve_existing_data() {
    let dir = dir();
    let db = Database::open(&dir).unwrap();
    let w = db.create_workspace("Harness专项".into()).unwrap();
    let other = db.create_workspace("另一工作区".into()).unwrap();
    let file = dir.join("AGENTS.MD");
    std::fs::write(&file, "Windows\n人工审批\n本地模型 Ollama\n").unwrap();
    let s = db.fit_import(&w.id, &file).unwrap();
    assert!(db.fit_documents(&other.id, &[s.id.clone()]).is_err());
    let mut input = Input {
        snapshot_ids: vec![s.id],
        requirements: vec![],
        weights: weights(),
        expected_version: 0,
        note: "首次分析".into(),
    };
    let body = db.fit_body(&w.id, &input).unwrap();
    let mut rs: Vec<Requirement> = serde_json::from_value(body["requirements"].clone()).unwrap();
    rs[0].evidence.as_mut().unwrap().excerpt = "伪造原文".into();
    input.requirements = rs;
    assert!(db.fit_save(&w.id, input).is_err());
    let input = || Input {
        snapshot_ids: body["snapshotIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().into())
            .collect(),
        requirements: vec![],
        weights: weights(),
        expected_version: 0,
        note: "首次分析".into(),
    };
    let saved = db.fit_save(&w.id, input()).unwrap();
    assert_eq!(saved.version, 1);
    assert!(db.fit_save(&w.id, input()).is_err());
    assert!(db.fit_read(&other.id, Some(1)).is_err());
    assert!(
        db.connection()
            .unwrap()
            .execute("UPDATE harness_fit_assessments SET version=2", [])
            .is_err()
    );
    assert!(
        db.connection()
            .unwrap()
            .execute("DELETE FROM harness_fit_assessments", [])
            .is_err()
    );
    let mut next = input();
    next.expected_version = 1;
    next.requirements = serde_json::from_value(saved.body["requirements"].clone()).unwrap();
    let r = &mut next.requirements[0];
    r.value = "用户补充：只使用本地模型".into();
    r.status = "edited".into();
    r.criterion = Some("local_model".into());
    r.hard = true;
    let v2 = db.fit_save(&w.id, next).unwrap();
    assert_eq!(v2.body["previousContentHash"], saved.content_hash);
    assert_eq!(
        db.fit_read(&w.id, Some(1)).unwrap()["selected"]["body"],
        saved.body
    );
    assert_eq!(db.conversations(&w.id).unwrap().len(), 0);
    drop(db);
    assert_eq!(
        Database::open(&dir).unwrap().fit_read(&w.id, None).unwrap()["selected"]["version"],
        2
    );
}
