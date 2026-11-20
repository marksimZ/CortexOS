use super::*;

fn edited(edit: impl FnOnce(&mut Value)) -> AppResult<Bundle> {
    let mut cases: Value = serde_json::from_str(CASES_V2).unwrap();
    edit(&mut cases);
    for c in cases.as_array_mut().unwrap() {
        c.as_object_mut().unwrap().remove("content_hash");
        c["content_hash"] = json!(hash(encode(c).unwrap().as_bytes()));
    }
    let cases = encode(&cases).unwrap();
    let mut manifest: Value = serde_json::from_str(MANIFEST_V2).unwrap();
    manifest["files"]["cases.json"] = json!(hash(cases.as_bytes()));
    validate(&encode(&manifest).unwrap(), &cases, SCHEMA_V2, RULES_V2)
}

#[test]
fn benchmark_v2_versions_hashes_and_revision_identity() {
    let old = builtin_version(1).unwrap();
    let new = builtin_version(2).unwrap();
    assert_eq!(
        old.content_hash,
        "3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc"
    );
    assert_eq!(
        hash(CASES.as_bytes()),
        "784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806"
    );
    assert_eq!(new.cases.len(), 60);
    assert!(builtin_version(3).is_err());
    assert_ne!(old.content_hash, new.content_hash);
    for (before, after) in old.cases.iter().zip(&new.cases) {
        assert_eq!(before.case_id, after.case_id);
        assert_eq!(before.category, after.category);
        assert!(before.scoring.contract.is_none());
        assert_eq!(after.scoring.version, 2);
        assert_ne!(before.content_hash, after.content_hash);
        assert!(!encode(before).unwrap().contains("contract"));
    }
    assert!(new.cases[20].input.prompt.contains("等差数列"));
    assert!(new.cases[56].input.prompt.contains("limit明确设为3"));
    assert!(validate(MANIFEST_V2, &format!("{CASES_V2} "), SCHEMA_V2, RULES_V2).is_err());
    assert!(edited(|c| c[0]["dataset_version"] = json!(1)).is_err());
    assert!(edited(|c| c[0]["scoring"]["version"] = json!(1)).is_err());
    assert!(edited(|c| c[0]["unexpected"] = json!(true)).is_err());
    assert!(edited(|c| c[0]["scoring"]["contract"]["case_sensitive"] = json!(false)).is_err());
}

#[test]
fn benchmark_v2_python_domains_types_and_boundaries() {
    let b = builtin_version(2).unwrap();
    let mut total = 0;
    for case in &b.cases[30..40] {
        let Expected::Python { tests, .. } = &case.expected else {
            panic!()
        };
        let old = super::super::builtin().unwrap();
        let Expected::Python {
            tests: original, ..
        } = &old.cases[30 + (case.case_id[5..].parse::<usize>().unwrap() - 1)].expected
        else {
            panic!()
        };
        assert_eq!(
            encode(&tests[..original.len()]).unwrap(),
            encode(original).unwrap()
        );
        total += tests.len();
    }
    assert_eq!(total, 93);
    let Expected::Python { tests, .. } = &b.cases[36].expected else {
        panic!()
    };
    assert_eq!(
        tests
            .iter()
            .map(|t| t.args[0].as_i64().unwrap())
            .collect::<BTreeSet<_>>(),
        (0..=12).collect()
    );
    assert!(edited(|c| c[30]["expected"]["tests"][0]["output"] = json!(false)).is_err());
    assert!(edited(|c| c[34]["expected"]["tests"][0]["output"] = json!(1)).is_err());
    assert!(edited(|c| c[36]["expected"]["tests"][0]["args"] = json!([-1])).is_err());
    assert!(edited(|c| c[35]["expected"]["tests"][0]["args"] = json!([5, 10, 0])).is_err());
    assert!(edited(|c| c[38]["expected"]["tests"][1]["args"] = json!([[3, 1], [2, 3]])).is_err());
    assert!(edited(|c| c[32]["scoring"]["contract"]["text_units"] = json!("graphemes")).is_err());
    assert!(edited(|c| c[30]["scoring"]["contract"]["input_mutation"] = json!("allowed")).is_err());
}

#[test]
fn benchmark_v2_rag_reference_contracts_and_spans() {
    let b = builtin_version(2).unwrap();
    let spans = &b.cases[47].scoring.contract.as_ref().unwrap()["evidence"]["floor"];
    assert_eq!(spans.as_array().unwrap().len(), 4); // 2:2, 1:2, 2:3, 1:3.
    assert!(
        edited(|c| c[40]["scoring"]["contract"]["evidence"]["opening_time"][0]["start"] = json!(0))
            .is_err()
    );
    assert!(
        edited(
            |c| c[40]["scoring"]["contract"]["evidence"]["opening_time"][0]["source_id"] =
                json!("missing")
        )
        .is_err()
    );
    assert!(
        edited(
            |c| c[40]["scoring"]["contract"]["evidence"]["opening_time"] =
                json!([{"source_id":"notice","start":2,"end":2}])
        )
        .is_err()
    );
    assert!(
        edited(|c| c[43]["expected"]["answer"]["facts"] = json!({"saturday_price_yuan":42}))
            .is_err()
    );
    assert!(edited(|c| c[44]["expected"]["answer"]["reason"] = json!("任意说明")).is_err());
    assert!(
        edited(|c| c[45]["expected"]["answer"]["citations"][1]["source_id"] = json!("a")).is_err()
    );
    assert!(
        edited(|c| c[49]["scoring"]["contract"]["as_of"] = json!("2026-06-01T00:00:00+08:00"))
            .is_err()
    );
    assert!(
        edited(|c| c[40]["scoring"]["contract"]["fact_aliases"]["absent"] = json!(["value"]))
            .is_err()
    );
}

#[test]
fn benchmark_v2_json_tool_reference_shapes_and_date() {
    assert!(edited(|c| c[50]["expected"]["answer"]["age"] = json!("28")).is_err());
    assert!(
        edited(|c| c[50]["expected"]["answer"]
            .as_object_mut()
            .unwrap()
            .remove("name")
            .map(|_| ())
            .unwrap())
        .is_err()
    );
    assert!(edited(|c| c[50]["expected"]["answer"]["extra"] = json!(1)).is_err());
    assert!(edited(|c| c[51]["expected"]["answer"]["done"] = json!(1)).is_err());
    assert!(edited(|c| c[53]["expected"]["answer"]["date"] = json!("2026-02-30")).is_err());
    assert!(edited(|c| c[54]["expected"]["answer"]["note"] = json!("")).is_err());
    assert!(edited(|c| c[55]["expected"]["name"] = json!("wrong_tool")).is_err());
    assert!(edited(|c| c[56]["expected"]["arguments"]["limit"] = json!("3")).is_err());
    assert!(
        edited(|c| {
            c[56]["expected"]["arguments"]
                .as_object_mut()
                .unwrap()
                .remove("query");
        })
        .is_err()
    );
    assert!(edited(|c| c[55]["expected"]["arguments"]["extra"] = json!(true)).is_err());
    assert!(edited(|c| c[55]["scoring"]["contract"]["execute"] = json!(true)).is_err());
    assert!(edited(|c| c[55]["scoring"]["contract"]["native_only"] = json!(false)).is_err());
}

#[test]
fn benchmark_v2_same_version_conflict_is_rejected_atomically() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-core60-v2")
        .join(crate::models::new_id());
    let db = Database::open(&path).unwrap();
    let ws = db
        .create_workspace("same version conflict fixture")
        .unwrap()
        .id;
    let c = db.connection().unwrap();
    c.execute("INSERT INTO benchmark_datasets VALUES('cortex-core-60',2,'conflicting fixture',2,?1,'{}','{}','fixture')",params!["f".repeat(64)]).unwrap();
    drop(c);
    assert!(
        db.benchmark_datasets(&ws)
            .err()
            .unwrap()
            .contains("不能覆盖历史题目")
    );
    let c = db.connection().unwrap();
    // The v1 insertion earlier in the same transaction also rolls back.
    assert_eq!(
        c.query_row("SELECT count(*) FROM benchmark_datasets", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM benchmark_cases", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT content_hash FROM benchmark_datasets", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "f".repeat(64)
    );
}

#[test]
fn benchmark_v2_upgrade_rolls_back_without_changing_v1_history() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-core60-v2")
        .join(crate::models::new_id());
    let db = Database::open(&path).unwrap();
    let ws = db
        .create_workspace("v2 isolated compatibility fixture")
        .unwrap()
        .id;
    db.create_conversation(&ws).unwrap();
    let chat_before = encode(&db.bootstrap().unwrap()).unwrap();
    let old = super::super::builtin().unwrap();
    {
        let c = db.connection().unwrap();
        c.execute(
            "INSERT INTO benchmark_datasets VALUES('cortex-core-60',1,?1,1,?2,?3,?4,?5)",
            params![
                old.manifest["name"].as_str().unwrap(),
                old.content_hash,
                encode(&old.manifest).unwrap(),
                encode(&old.manifest["provenance"]).unwrap(),
                crate::models::now()
            ],
        )
        .unwrap();
        for case in &old.cases {
            c.execute("INSERT INTO benchmark_cases VALUES('cortex-core-60',1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![case.case_id,case.category,encode(&case.input).unwrap(),encode(&case.expected).unwrap(),encode(&case.scoring).unwrap(),case.difficulty,case.output_format,encode(&case.requirements).unwrap(),encode(&case.fixture).unwrap(),case.content_hash]).unwrap();
        }
        let h = "0".repeat(64);
        c.execute("INSERT INTO benchmark_runs(id,workspace_id,dataset_id,dataset_version,launch_key,status,config_snapshot_json,config_hash,hardware_snapshot_json,app_version,source_revision,evaluator_version,created_at) VALUES('old-run',?1,'cortex-core-60',1,'old-launch','completed','{\"repetitions\":1}',?2,'{}','fixture','fixture','slice3-exact-v1','fixture')",params![ws,h]).unwrap();
        c.execute(
            "INSERT INTO benchmark_run_models VALUES('old-run',0,?1,'{}',?2)",
            params![ws, h],
        )
        .unwrap();
        c.execute("INSERT INTO benchmark_case_runs VALUES('old-case',?1,'old-run',0,'cortex-core-60',1,'math-001',1,'single','completed')",params![ws]).unwrap();
        c.execute("INSERT INTO benchmark_attempts(id,workspace_id,case_run_id,attempt_no,status,request_disposition) VALUES('old-attempt',?1,'old-case',1,'completed','finished')",params![ws]).unwrap();
        c.execute("INSERT INTO benchmark_evaluations VALUES('old-evaluation',?1,'old-attempt','integer_equal','1',?2,?2,'{\"correctness\":1}','{\"fixture\":true}','completed','fixture')",params![ws,h]).unwrap();
        c.execute_batch("CREATE TRIGGER fail_v2 BEFORE INSERT ON benchmark_cases WHEN NEW.dataset_version=2 AND NEW.case_id='math-002' BEGIN SELECT RAISE(ABORT,'v2 install failure'); END;").unwrap();
    }
    assert!(db.benchmark_datasets(&ws).is_err());
    {
        let c = db.connection().unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM benchmark_datasets", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM benchmark_cases", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            60
        );
        c.execute_batch("DROP TRIGGER fail_v2").unwrap();
    }
    let installed = db.benchmark_datasets(&ws).unwrap();
    assert_eq!(
        installed.iter().map(|b| b.version).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        db.benchmark_case(&ws, "cortex-core-60", 1, "rag-010")
            .unwrap()
            .content_hash,
        old.cases[49].content_hash
    );
    assert_eq!(
        db.benchmark_case(&ws, "cortex-core-60", 2, "rag-010")
            .unwrap()
            .dataset_version,
        2
    );
    assert_eq!(encode(&db.bootstrap().unwrap()).unwrap(), chat_before);
    assert!(db.benchmark_datasets("missing").is_err());
    drop(db);
    let db = Database::open(&path).unwrap();
    assert_eq!(
        db.benchmark_datasets(&ws).unwrap()[1].content_hash,
        installed[1].content_hash
    );
    let c = db.connection().unwrap();
    assert_eq!(
        c.query_row(
            "SELECT dataset_version FROM benchmark_runs WHERE id='old-run'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        c.query_row(
            "SELECT metrics_json FROM benchmark_evaluations WHERE id='old-evaluation'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "{\"correctness\":1}"
    );
    assert_eq!(
        c.query_row(
            "SELECT evaluator_version FROM benchmark_evaluations WHERE id='old-evaluation'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "1"
    );
    assert!(
        c.execute(
            "UPDATE benchmark_cases SET expected_json='{}' WHERE dataset_version=1",
            []
        )
        .is_err()
    );
    assert!(
        c.execute(
            "UPDATE benchmark_cases SET expected_json='{}' WHERE dataset_version=2",
            []
        )
        .is_err()
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM knowledge_nodes", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(c);
    let input = crate::benchmark::repository::StartInput {
        local_keep_alive_seconds: None,
        personalized_dataset: None,
        dataset_version: 1,
        python_image: None,
        launch_key: "v2-must-not-run-in-stage-a".into(),
        development_only: true,
        dataset_hash: installed[1].content_hash.clone(),
        case_id: "math-001".into(),
        model: crate::benchmark::preflight::ModelRequest {
            model: "synthetic-not-contacted".into(),
            expected_digest: Some("a".repeat(64)),
            parameters: crate::benchmark::preflight::Parameters {
                num_ctx: 4096,
                num_predict: 64,
                temperature: 0.0,
                timeout_seconds: 30,
            },
            requirements: vec!["completion".into()],
        },
    };
    assert!(db.benchmark_intent(&ws, input).is_err());
}
