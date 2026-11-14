use super::*;

#[test]
fn harness_programming_drafts_preserve_scope_history_and_execution_gate() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-programming-rust")
        .join(new_id());
    let db = Database::open(&root).unwrap();
    let workspace = db.create_workspace("编程草案检查".into()).unwrap().id;
    let other = db.create_workspace("隔离".into()).unwrap().id;
    let source = root.join("development.md");
    std::fs::write(&source, "Use Python pure functions.\n").unwrap();
    let doc = db.fit_import(&workspace, &source).unwrap();
    db.fit_save(
        &workspace,
        Input {
            snapshot_ids: vec![doc.id],
            requirements: vec![],
            weights: json!({"compatibility":3,"automation":3,"governance":3,"context":3}),
            expected_version: 0,
            note: String::new(),
        },
    )
    .unwrap();
    let code = "def adjust(n):\n    return n\n";
    let project = db
        .save_repository_snapshot(
            &workspace,
            RepositorySnapshot {
                id: new_id(),
                name: "controlled fixture".into(),
                content_hash: hash(code.as_bytes()),
                profile: json!({"execution":"none"}),
                files: vec![RepositoryFile {
                    path: "src/value.py".into(),
                    language: "Python".into(),
                    content: code.into(),
                    content_hash: hash(code.as_bytes()),
                    symbols: vec![],
                }],
                created_at: now(),
            },
        )
        .unwrap();
    let origin = Origin {
        assessment_version: 1,
        repository_id: project.id,
        path: "src/value.py".into(),
        start: 1,
        end: 2,
    };
    let spec = Spec {
        project: None,
        basis: "confirmed_requirement".into(),
        rationale: "受控人工需求：只将负数归零，其他行为保持。".into(),
        prompt: "返回非负输入本身，负数返回0。".into(),
        function: "adjust".into(),
        parameters: vec![Parameter {
            name: "n".into(),
            kind: "integer".into(),
        }],
        return_type: "integer".into(),
        examples: (-3..=3)
            .map(|n| Example {
                label: format!("输入{n}"),
                role: if n < 0 {
                    "target"
                } else if n == 0 {
                    "boundary"
                } else {
                    "regression"
                }
                .into(),
                args: vec![json!(n)],
                output: json!(n.max(0)),
            })
            .collect(),
        reference_code: "def adjust(n):\n    return max(n, 0)".into(),
        wrong_code: "def adjust(n):\n    return 0".into(),
    };
    let input = |s: Spec| SaveInput {
        job_id: None,
        expected_revision: 0,
        origin: origin.clone(),
        spec: s,
    };
    assert!(
        db.save_programming_draft(&other, input(spec.clone()))
            .is_err()
    );
    let mut bad = spec.clone();
    bad.examples[0].output = json!(true);
    assert!(db.save_programming_draft(&workspace, input(bad)).is_err());
    let mut bad = spec.clone();
    bad.examples[1].args = bad.examples[0].args.clone();
    assert!(db.save_programming_draft(&workspace, input(bad)).is_err());
    let mut bad = spec.clone();
    bad.examples
        .iter_mut()
        .for_each(|e| e.role = "target".into());
    assert!(db.save_programming_draft(&workspace, input(bad)).is_err());
    let mut bad = spec.clone();
    bad.function = "return".into();
    assert!(db.save_programming_draft(&workspace, input(bad)).is_err());
    let mut escaped = input(spec.clone());
    escaped.origin.path = "../value.py".into();
    assert!(db.save_programming_draft(&workspace, escaped).is_err());
    let job = db
        .save_programming_draft(&workspace, input(spec.clone()))
        .unwrap();
    let id = job["id"].as_str().unwrap();
    let first = job["revisions"][0].clone();
    assert_eq!(
        first["body"]["source"]["excerpt"],
        "def adjust(n):\n    return n"
    );
    assert_eq!(first["body"]["validation"]["staticValid"], true);
    assert_eq!(first["body"]["validation"]["publishable"], false);
    assert!(
        first["body"]["validation"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["actual"] == "Not Run")
    );
    assert!(db.programming_drafts(&other, Some(id)).is_err());
    assert!(db.generation_job(&workspace, id).is_err());
    assert!(db.harness_task_job(&workspace, id).is_err());
    assert!(
        db.harness_tasks(&workspace, None).unwrap()["history"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(db.generation_history(&workspace).unwrap().is_empty());
    assert!(
        db.publish_harness_tasks(
            &workspace,
            id,
            vec![tasks::Approval {
                id: id.into(),
                revision: 1,
                approved: true
            }],
            "reviewer",
            0
        )
        .is_err()
    );
    let mut changed = spec.clone();
    changed.rationale.push_str(" 已复核期望类型。");
    let edit = || SaveInput {
        job_id: Some(id.into()),
        expected_revision: 1,
        origin: origin.clone(),
        spec: changed.clone(),
    };
    let revised = db.save_programming_draft(&workspace, edit()).unwrap();
    assert_eq!(revised["revisions"][0]["revision"], 2);
    assert_eq!(revised["revisions"][1], first);
    assert_ne!(revised["revisions"][0]["contentHash"], first["contentHash"]);
    assert!(db.save_programming_draft(&workspace, edit()).is_err());
    let mut moved = edit();
    moved.expected_revision = 2;
    moved.origin.start = 2;
    assert!(db.save_programming_draft(&workspace, moved).is_err());
    drop(db);
    let db = Database::open(&root).unwrap();
    assert_eq!(
        db.programming_drafts(&workspace, Some(id)).unwrap()["selected"],
        revised
    );
    let c = db.connection().unwrap();
    for table in [
        "messages",
        "execution_runs",
        "knowledge_nodes",
        "benchmark_datasets",
        "benchmark_dataset_reviews",
    ] {
        let count: u32 = c
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "draft unexpectedly wrote to {table}");
    }
    assert!(c.execute("UPDATE benchmark_generated_cases SET candidate_json='{}' WHERE job_id=?1 AND revision=2",[id]).is_err());
    c.execute("INSERT INTO benchmark_generated_cases VALUES(?1,?1,3,'{}','{\"contentHash\":\"incorrect\",\"publishable\":false}',?2)",params![id,now()]).unwrap();
    drop(c);
    assert!(db.programming_drafts(&workspace, Some(id)).is_err());
}
