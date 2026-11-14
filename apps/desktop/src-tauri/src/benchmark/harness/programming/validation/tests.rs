use super::*;

#[test]
fn programming_controls_bind_evidence_reject_stale_approval_and_preserve_versions() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-validation-rust")
        .join(new_id());
    let db = Database::open(&root).unwrap();
    let workspace = db.create_workspace("对照边界检查".into()).unwrap().id;
    let other = db.create_workspace("隔离".into()).unwrap().id;
    let path = root.join("development.md");
    std::fs::write(&path, "Use Python pure functions.").unwrap();
    let doc = db.fit_import(&workspace, &path).unwrap();
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
    let code = "def adjust(n):\n    return n";
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
        rationale: "固定受控需求：负数归零，其他不变。".into(),
        prompt: "返回非负输入本身，否则0。".into(),
        function: "adjust".into(),
        parameters: vec![Parameter {
            name: "n".into(),
            kind: "integer".into(),
        }],
        return_type: "integer".into(),
        examples: (-3..=3)
            .map(|n| Example {
                label: format!("{n}"),
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
        reference_code: "def adjust(n):\n    return max(0, n)".into(),
        wrong_code: "def adjust(n):\n    return 0".into(),
    };
    let draft = db
        .save_programming_draft(
            &workspace,
            SaveInput {
                job_id: None,
                expected_revision: 0,
                origin: origin.clone(),
                spec: spec.clone(),
            },
        )
        .unwrap();
    let draft_id = draft["id"].as_str().unwrap();
    let row = &draft["revisions"][0];
    let image = format!("sha256:{}", "a".repeat(64));
    let input = || Start {
        draft_id: draft_id.into(),
        expected_revision: 1,
        draft_hash: row["contentHash"].as_str().unwrap().into(),
        image_id: image.clone(),
    };
    assert!(
        db.begin_programming_validation(&other, &new_id(), &input())
            .is_err()
    );
    let validation_id = new_id();
    let body = db
        .begin_programming_validation(&workspace, &validation_id, &input())
        .unwrap();
    assert!(db.generation_job(&workspace, &validation_id).is_err());
    assert!(db.harness_task_job(&workspace, &validation_id).is_err());
    assert!(db.validation_record(&other, &validation_id).is_err());
    let approve = |hash_value: String, version| Publish {
        draft_id: draft_id.into(),
        expected_revision: 1,
        validation_id: validation_id.clone(),
        validation_hash: hash_value,
        reviewer: "检查审核人".into(),
        approved: true,
        expected_version: version,
    };
    let pending = db.validation_record(&workspace, &validation_id).unwrap();
    assert_eq!(pending["publishable"], false);
    assert!(
        db.publish_programming_task(
            &workspace,
            approve(pending["contentHash"].as_str().unwrap().into(), 0)
        )
        .is_err()
    );
    // Synthetic scorer observations exercise storage gates; no Python is executed by this test.
    for name in ["reference", "unchanged", "wrong"] {
        let mut containers = vec![];
        let mut assertions = vec![];
        for (i, e) in spec.examples.iter().enumerate() {
            let container = format!("cortexos-bench-{}", new_id());
            db.append_validation(
                &workspace,
                &validation_id,
                &format!("container-{container}"),
                json!({"name":container,"owner":validation_id,"variant":name}),
            )
            .unwrap();
            containers.push(json!({"name":container,"removed":true}));
            let pass = name == "reference"
                || name == "unchanged" && e.role != "target"
                || name == "wrong" && i == 0;
            assertions
                .push(json!({"index":i+1,"passed":pass,"observation":{"outcome":"returned"}}));
        }
        let code = if name == "reference" {
            &spec.reference_code
        } else if name == "unchanged" {
            body["source"]["excerpt"].as_str().unwrap()
        } else {
            &spec.wrong_code
        };
        db.append_validation(&workspace,&validation_id,name,json!({"codeHash":hash(code.as_bytes()),"score":{"state":if name=="reference"{"Passed"}else{"Failed"},"details":{"engineVersion":evaluation::ENGINE_VERSION,"imageId":image,"sandboxPolicy":python::policy(),"assertions":assertions,"ownedContainers":containers}}})).unwrap();
    }
    db.finish_programming_validation(&workspace, &validation_id, "completed", None)
        .unwrap();
    let valid = db.validation_record(&workspace, &validation_id).unwrap();
    assert_eq!(valid["publishable"], true);
    assert!(
        db.publish_programming_task(&workspace, approve("forged".into(), 0))
            .is_err()
    );
    let mut unapproved = approve(valid["contentHash"].as_str().unwrap().into(), 0);
    unapproved.approved = false;
    assert!(db.publish_programming_task(&workspace, unapproved).is_err());
    let published = db
        .publish_programming_task(
            &workspace,
            approve(valid["contentHash"].as_str().unwrap().into(), 0),
        )
        .unwrap();
    let dataset_id = format!("harness-python-{draft_id}");
    assert_eq!(
        db.programming_taskset(&workspace, &dataset_id, 1).unwrap(),
        published
    );
    assert!(db.programming_taskset(&other, &dataset_id, 1).is_err());
    assert!(db.personalized_dataset(&workspace, &dataset_id, 1).is_err());
    assert!(
        db.benchmark_case(
            &workspace,
            &dataset_id,
            1,
            body["case"]["case_id"].as_str().unwrap()
        )
        .is_err()
    );
    assert!(
        db.publish_programming_task(
            &workspace,
            approve(valid["contentHash"].as_str().unwrap().into(), 0)
        )
        .is_err()
    );
    let mut changed = spec.clone();
    changed.prompt.push_str(" 不修改输入。");
    db.save_programming_draft(
        &workspace,
        SaveInput {
            job_id: Some(draft_id.into()),
            expected_revision: 1,
            origin: origin.clone(),
            spec: changed,
        },
    )
    .unwrap();
    assert!(
        db.publish_programming_task(
            &workspace,
            approve(valid["contentHash"].as_str().unwrap().into(), 1)
        )
        .is_err()
    );
    assert_eq!(
        db.programming_taskset(&workspace, &dataset_id, 1).unwrap(),
        published
    );
    let mut bad = valid["records"].as_object().unwrap().clone();
    bad["wrong"]["score"]["details"]["assertions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .for_each(|r| r["observation"] = json!({"outcome":"program_error"}));
    assert_eq!(
        comparisons(&body, &bad)["checks"]["negativeDetected"],
        false
    );
    bad["unchanged"]["score"]["details"]["assertions"][6]["passed"] = json!(false);
    assert_eq!(
        comparisons(&body, &bad)["checks"]["regressionPreserved"],
        false
    );
    let latest = db.programming_job(&workspace, draft_id).unwrap();
    let mut next = input();
    next.expected_revision = 2;
    next.draft_hash = latest["revisions"][0]["contentHash"]
        .as_str()
        .unwrap()
        .into();
    let interrupted_id = new_id();
    db.begin_programming_validation(&workspace, &interrupted_id, &next)
        .unwrap();
    drop(db);
    let db = Database::open(&root).unwrap();
    assert_eq!(
        db.validation_record(&workspace, &interrupted_id).unwrap()["status"],
        "interrupted"
    );
    assert_eq!(
        db.validation_record(&workspace, &validation_id).unwrap(),
        valid
    );
    assert_eq!(
        db.programming_taskset(&workspace, &dataset_id, 1).unwrap(),
        published
    );
    // Synthetic observations exercise persistence boundaries, not real CLI/Docker readiness.
    use crate::benchmark::harness::preflight::CheckInput;
    let mut check = CheckInput {
        assessment_version: 1,
        dataset_id: dataset_id.clone(),
        version: 1,
        dataset_hash: published["content_hash"].as_str().unwrap().into(),
    };
    assert!(db.preflight_task(&other, &check).is_err());
    check.dataset_hash = "forged".into();
    assert!(db.preflight_task(&workspace, &check).is_err());
    check.dataset_hash = published["content_hash"].as_str().unwrap().into();
    check.version = 2;
    assert!(db.preflight_task(&workspace, &check).is_err());
    check.version = 1;
    let first_check = db
        .save_harness_preflight(
            &workspace,
            &check,
            json!({"state":"detected","version":"0.0.0","testOnly":true}),
            json!({"state":"available","testOnly":true}),
            &now(),
        )
        .unwrap();
    assert_eq!(first_check["body"]["canRun"], false);
    assert_eq!(first_check["body"]["taskExecution"], "Not Run");
    assert_eq!(first_check["body"]["checks"][3]["state"], "blocked");
    let check_id = first_check["id"].as_str().unwrap();
    assert!(db.generation_job(&workspace, check_id).is_err());
    assert!(db.harness_task_job(&workspace, check_id).is_err());
    assert!(db.harness_preflight(&other, 1, Some(check_id)).is_err());
    assert!(db.harness_preflight(&workspace, 2, Some(check_id)).is_err());
    assert!(
        db.connection()
            .unwrap()
            .execute(
                "UPDATE benchmark_generation_jobs SET plan_json='{}' WHERE id=?1",
                [check_id]
            )
            .is_err()
    );
    db.save_harness_preflight(
        &workspace,
        &check,
        json!({"state":"not_found","testOnly":true}),
        json!({"state":"unavailable","testOnly":true}),
        &now(),
    )
    .unwrap();
    let history = db.harness_preflight(&workspace, 1, Some(check_id)).unwrap();
    assert_eq!(history["selected"], first_check);
    assert_eq!(history["history"].as_array().unwrap().len(), 2);
    assert_eq!(history["datasets"].as_array().unwrap().len(), 1);
    assert_eq!(
        db.programming_taskset(&workspace, &dataset_id, 1).unwrap(),
        published
    );
    // Only synthetic transport observations here; real Docker transport is a separate native check.
    let prepare = || {
        serde_json::from_value::<crate::benchmark::harness::workspace::Prepare>(json!({"assessmentVersion":1,"checkId":check_id,"checkHash":first_check["contentHash"]})).unwrap()
    };
    let copy_id = new_id();
    assert!(db.begin_workspace(&other, &copy_id, &prepare()).is_err());
    db.begin_workspace(&workspace, &copy_id, &prepare())
        .unwrap();
    assert!(
        db.begin_workspace(&workspace, &new_id(), &prepare())
            .is_err()
    );
    assert!(db.workspace_record(&other, &copy_id).is_err());
    assert!(db.generation_job(&workspace, &copy_id).is_err());
    assert!(db.harness_task_job(&workspace, &copy_id).is_err());
    let copy = db.workspace_record(&workspace, &copy_id).unwrap();
    assert_eq!(copy["plan"]["taskExecution"], "Not Run");
    db.workspace_finish(&workspace, &copy_id, "ready", None)
        .unwrap();
    let submit = |confirmed: bool, hash_value: Value, code: &str| {
        serde_json::from_value::<crate::benchmark::harness::workspace::Submit>(
            json!({"jobId":copy_id,"planHash":hash_value,"code":code,"confirmed":confirmed}),
        )
        .unwrap()
    };
    assert!(
        db.begin_workspace_submit(
            &workspace,
            submit(
                false,
                copy["planHash"].clone(),
                "def adjust(n):\n    return 0"
            )
        )
        .is_err()
    );
    assert!(
        db.begin_workspace_submit(
            &workspace,
            submit(true, json!("forged"), "def adjust(n):\n    return 0")
        )
        .is_err()
    );
    assert!(
        db.begin_workspace_submit(
            &workspace,
            submit(
                true,
                copy["planHash"].clone(),
                copy["plan"]["bundle"]["originalCode"].as_str().unwrap()
            )
        )
        .is_err()
    );
    let new_code = "def adjust(n):\n    return max(0, n)";
    db.connection().unwrap().execute_batch("CREATE TEMP TRIGGER reject_workspace_submission BEFORE INSERT ON benchmark_generated_cases WHEN json_extract(NEW.candidate_json,'$.kind')='submission' BEGIN SELECT RAISE(ABORT,'controlled write failure'); END;").unwrap();
    assert!(
        db.begin_workspace_submit(&workspace, submit(true, copy["planHash"].clone(), new_code))
            .is_err()
    );
    assert_eq!(
        db.workspace_record(&workspace, &copy_id).unwrap()["status"],
        "ready"
    );
    db.connection()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_workspace_submission;")
        .unwrap();
    db.begin_workspace_submit(&workspace, submit(true, copy["planHash"].clone(), new_code))
        .unwrap();
    assert!(
        db.begin_workspace_submit(&workspace, submit(true, copy["planHash"].clone(), new_code))
            .is_err()
    );
    db.workspace_event(&workspace,&copy_id,"container",json!({"name":copy["plan"]["containerName"],"owner":copy_id,"role":"workspace","testOnly":true})).unwrap();
    assert!(
        db.workspace_finish(&workspace, &copy_id, "completed", None)
            .is_err()
    );
    let scope = workspace.clone();
    let copy_name = copy["plan"]["containerName"].clone();
    drop(db);
    let db = Database::open(&root).unwrap();
    let recovered = db.workspace_record(&scope, &copy_id).unwrap();
    assert_eq!(recovered["status"], "interrupted");
    assert_eq!(recovered["pendingContainers"], json!([copy_name]));
    assert_eq!(recovered["events"][0]["data"]["code"], new_code);
    db.workspace_event(
        &scope,
        &copy_id,
        "cleanup",
        json!({"name":copy_name,"clean":true,"testOnly":true}),
    )
    .unwrap();
    db.workspace_finish(&scope, &copy_id, "closed", None)
        .unwrap();
    assert_eq!(
        db.programming_taskset(&scope, &dataset_id, 1).unwrap(),
        published
    );
    for table in ["messages", "knowledge_nodes", "execution_runs"] {
        let count: u32 = db
            .connection()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
