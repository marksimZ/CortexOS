use super::*;
use crate::benchmark::personalized::cases::Location;

#[test]
fn harness_tasks_scope_revision_and_publication_are_auditable() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-d2-rust")
        .join(new_id());
    let db = Database::open(&root).unwrap();
    let w = db.create_workspace("D2任务检查".into()).unwrap().id;
    let other = db.create_workspace("隔离".into()).unwrap().id;
    let file = root.join("AGENTS.md");
    std::fs::write(
        &file,
        "Windows requires human approval.\nNever run project scripts on the host.\n",
    )
    .unwrap();
    let document = db.fit_import(&w, &file).unwrap();
    let mut analysis = Input {
        snapshot_ids: vec![document.id],
        requirements: vec![],
        weights: json!({"compatibility":3,"automation":3,"governance":3,"context":3}),
        expected_version: 0,
        note: String::new(),
    };
    analysis.requirements =
        serde_json::from_value(db.fit_body(&w, &analysis).unwrap()["requirements"].clone())
            .unwrap();
    let confirmed = analysis
        .requirements
        .iter_mut()
        .find(|r| r.evidence.is_some())
        .unwrap();
    confirmed.status = "confirmed".into();
    let requirement = confirmed.id.clone();
    db.fit_save(&w, analysis).unwrap();
    let content = "pub fn example() {}\n";
    let snapshot = db
        .save_repository_snapshot(
            &w,
            RepositorySnapshot {
                id: new_id(),
                name: "fixture".into(),
                content_hash: hash(b"fixed fixture"),
                profile: json!({"execution":"none"}),
                files: ["src/a.rs", "src/b.rs"]
                    .iter()
                    .map(|p| RepositoryFile {
                        path: (*p).into(),
                        language: "Rust".into(),
                        content: content.into(),
                        content_hash: hash(content.as_bytes()),
                        symbols: vec![],
                    })
                    .collect(),
                created_at: now(),
            },
        )
        .unwrap();
    let input = || CreateInput {
        scenarios: vec![],
        assessment_version: 1,
        repository_id: Some(snapshot.id.clone()),
        requirement_ids: vec![requirement.clone()],
        locations: vec![Location {
            path: "src/a.rs".into(),
            start: 1,
            end: 1,
        }],
    };
    assert!(db.create_harness_tasks(&other, input()).is_err());
    let job = db.create_harness_tasks(&w, input()).unwrap();
    let mut duplicate = input();
    duplicate.requirement_ids.clear();
    duplicate.locations.push(Location {
        path: "src/b.rs".into(),
        start: 1,
        end: 1,
    });
    assert!(db.create_harness_tasks(&w, duplicate).is_err());
    let id = job["id"].as_str().unwrap();
    assert!(db.generation_job(&w, id).is_err());
    assert!(db.generation_history(&w).unwrap().is_empty());
    assert!(db.harness_task_job(&other, id).is_err());
    assert!(
        job["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["validation"]["valid"] == true
                && t["validation"]["taskExecution"] == "Not Run")
    );
    assert_eq!(
        job["candidates"][1]["task"]["case"]["expected"]["answers"],
        json!(["src/a.rs", "src/b.rs"])
    );
    let first = job["candidates"][0]["id"].as_str().unwrap();
    let second = job["candidates"][1]["id"].as_str().unwrap();
    let revise = |job: &Value, index: usize| Edit {
        prompt: format!(
            "{}\n仅输出上述要求的原文或路径。",
            job["candidates"][index]["task"]["case"]["input"]["prompt"]
                .as_str()
                .unwrap()
        ),
        rationale: "已人工核对固定原文。".into(),
    };
    let revised = db
        .revise_harness_task(&w, id, first, 1, revise(&job, 0))
        .unwrap();
    assert_eq!(revised["candidates"][0]["id"], first);
    assert_eq!(revised["candidates"][1]["id"], second);
    assert!(
        db.revise_harness_task(&w, id, first, 1, revise(&job, 0))
            .is_err()
    );
    let revised = db
        .revise_harness_task(&w, id, second, 1, revise(&revised, 1))
        .unwrap();
    assert_eq!(
        revised["candidates"][0]["task"]["kind"],
        "requirement_quote"
    );
    assert_eq!(revised["candidates"][1]["task"]["kind"], "locate_excerpt");
    let approvals = |a: u32, b: u32| {
        vec![
            Approval {
                id: first.into(),
                revision: a,
                approved: true,
            },
            Approval {
                id: second.into(),
                revision: b,
                approved: true,
            },
        ]
    };
    assert!(
        db.publish_harness_tasks(&w, id, approvals(1, 2), "reviewer", 0)
            .is_err()
    );
    let published = db
        .publish_harness_tasks(&w, id, approvals(2, 2), "reviewer", 0)
        .unwrap();
    let ds = format!("harness-{id}");
    assert_eq!(db.harness_taskset(&w, &ds, 1).unwrap(), published);
    assert!(db.harness_taskset(&other, &ds, 1).is_err());
    assert!(db.personalized_dataset(&w, &ds, 1).is_err());
    assert!(db.personalized_datasets(&w).unwrap().is_empty());
    assert!(db.benchmark_case(&w, &ds, 1, first).is_err());
    assert_eq!(db.benchmark_datasets(&w).unwrap().len(), 2);
    assert!(
        db.publish_harness_tasks(&w, id, approvals(2, 2), "reviewer", 0)
            .is_err()
    );
    db.revise_harness_task(&w, id, first, 2, revise(&revised, 0))
        .unwrap();
    let v2 = db
        .publish_harness_tasks(&w, id, approvals(3, 2), "reviewer", 1)
        .unwrap();
    assert_eq!(v2["tasks"][0]["task"]["case"]["dataset_version"], 2);
    let versioned: Task = serde_json::from_value(v2["tasks"][0]["task"].clone()).unwrap();
    dataset::validate_runtime_case(&versioned.case).unwrap();
    assert_ne!(published["content_hash"], v2["content_hash"]);
    assert_eq!(db.harness_taskset(&w, &ds, 1).unwrap(), published);
    assert!(
        db.connection()
            .unwrap()
            .execute(
                "UPDATE benchmark_generated_cases SET revision=99 WHERE job_id=?1",
                [id]
            )
            .is_err()
    );
    assert!(
        db.connection()
            .unwrap()
            .execute(
                "UPDATE benchmark_datasets SET version=99 WHERE id=?1",
                [&ds]
            )
            .is_err()
    );
    assert!(db.conversations(&w).unwrap().is_empty());
    drop(db);
    assert_eq!(
        Database::open(&root)
            .unwrap()
            .harness_taskset(&w, &ds, 2)
            .unwrap(),
        v2
    );
}

#[test]
fn harness_tasks_reject_unconfirmed_sources_and_invalid_ranges() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-d2-rust")
        .join(new_id());
    let db = Database::open(&root).unwrap();
    let w = db.create_workspace("invalid".into()).unwrap().id;
    let file = root.join("development.md");
    std::fs::write(&file, "Windows\n人工审批\n").unwrap();
    let s = db.fit_import(&w, &file).unwrap();
    let analysis = db
        .fit_save(
            &w,
            Input {
                snapshot_ids: vec![s.id.clone()],
                requirements: vec![],
                weights: json!({"compatibility":1,"automation":1,"governance":1,"context":1}),
                expected_version: 0,
                note: String::new(),
            },
        )
        .unwrap();
    let r = analysis.body["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| !r["evidence"].is_null())
        .unwrap();
    assert!(
        db.create_harness_tasks(
            &w,
            CreateInput {
                scenarios: vec![],
                assessment_version: 1,
                repository_id: None,
                requirement_ids: vec![r["id"].as_str().unwrap().into()],
                locations: vec![]
            }
        )
        .is_err()
    );
    for (path, start, end) in [
        ("../development.md", 1, 1),
        ("development.md", 0, 1),
        ("development.md", 1, 99),
        ("development.md", 1, 1),
    ] {
        assert!(
            db.create_harness_tasks(
                &w,
                CreateInput {
                    scenarios: vec![],
                    assessment_version: 1,
                    repository_id: Some(s.id.clone()),
                    requirement_ids: vec![],
                    locations: vec![Location {
                        path: path.into(),
                        start,
                        end
                    }]
                }
            )
            .is_err()
        );
    }
    assert!(
        db.harness_tasks(&w, None).unwrap()["history"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
