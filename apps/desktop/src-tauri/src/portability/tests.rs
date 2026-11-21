use super::*;
use crate::knowledge::SaveKnowledge;
use rusqlite::params;

fn run_dir() -> Scratch {
    Scratch::create(&std::env::temp_dir().join("cortexos-h2-tests")).unwrap()
}

fn open(root: &Path, name: &str) -> Database {
    Database::open(&root.join(name)).unwrap()
}

fn file(ws: &str, text: &str) -> SaveKnowledge {
    SaveKnowledge {
        workspace_id: ws.into(),
        node_id: None,
        expected_revision: None,
        title: text.into(),
        kind: "knowledge".into(),
        content: text.into(),
        change_reason: "初始版本".into(),
        source_message_id: None,
        source_excerpt: None,
    }
}

fn set_history(db: &Database, workspace: &str) {
    db.benchmark_datasets(workspace).unwrap();
    let c = db.connection().unwrap();
    let question: String = c.query_row(
        "SELECT case_id FROM benchmark_cases WHERE dataset_id='cortex-core-60' AND dataset_version=1 ORDER BY case_id LIMIT 1",
        [], |row| row.get(0),
    ).unwrap();
    let hash = "a".repeat(64);
    c.execute(
        "INSERT INTO benchmark_runs(id,workspace_id,dataset_id,dataset_version,launch_key,status,config_snapshot_json,config_hash,hardware_snapshot_json,app_version,source_revision,evaluator_version,created_at) VALUES('historic-run',?1,'cortex-core-60',1,'old-launch','completed',json_object('repetitions',1),?2,'{}','0.2.0','test','evaluator-v1','old-date')",
        params![workspace, hash],
    ).unwrap();
    c.execute(
        "INSERT INTO benchmark_run_models VALUES('historic-run',0,?1,'{}',?2)",
        [workspace, &hash],
    )
    .unwrap();
    c.execute("INSERT INTO benchmark_case_runs VALUES('historic-case',?1,'historic-run',0,'cortex-core-60',1,?2,1,'single','completed')", [workspace, &question]).unwrap();
    c.execute("INSERT INTO benchmark_attempts VALUES('historic-attempt',?1,'historic-case',1,NULL,'completed','finished','old-date','old-date',NULL)", [workspace]).unwrap();
    c.execute(
        "INSERT INTO benchmark_evaluations VALUES('old-score',?1,'historic-attempt','exact','evaluator-v1',?2,?2,'{}',json_object('answer',true),'completed','old-date')",
        [workspace, &hash],
    ).unwrap();
}

fn ticket(path: &Path, manifest: &Manifest) -> OpenedBackup {
    OpenedBackup {
        path: path.into(),
        manifest: manifest.clone(),
    }
}

#[test]
fn full_snapshot_from_wal_and_restore_into_independent_instance() {
    let run = run_dir();
    let from = open(&run.0, "from");
    let target = open(&run.0, "target");
    let workspace = from.create_workspace("原工作区").unwrap();
    let original = target.create_workspace("目标原数据").unwrap();
    let ws = &workspace.id;
    let node = from.save_knowledge(file(ws, "walsnapshotneedle")).unwrap();
    from.approve_knowledge(ws, &node, 1).unwrap();
    set_history(&from, ws);
    assert!(
        fs::metadata(from.path.with_file_name("cortexos.sqlite3-wal"))
            .unwrap()
            .len()
            > 32
    );
    let source_path = run.0.join("full.cortexbackup");
    let draft = format!("cortexos.draft.v1.chat.{ws}%3Anew");
    let ui = Ui::from([
        (draft.clone(), "\"草稿\"".into()),
        ("cortexos.appearance".into(), "dark".into()),
        ("apiKey".into(), "test-key-excluded-H2".into()),
    ]);
    let manifest = save(&from, None, ui, &source_path).unwrap();
    assert_eq!(manifest.workspaces.len(), 1);
    assert!(save(&from, None, Ui::new(), &source_path).is_err());
    let target_dir = target.path.parent().unwrap().to_owned();
    let target_before = target.path.clone();
    restore::stage(
        &target,
        ticket(&source_path, &manifest),
        Ui::from([("cortexos.appearance".into(), "light".into())]),
    )
    .unwrap();
    assert_eq!(restore::status(&target).unwrap().phase, "pending");
    assert_eq!(target.conversations(&original.id).unwrap().len(), 0);
    drop(target);
    let restored = Database::open(&target_dir).unwrap();
    assert_eq!(restored.restore_issue, None);
    let state = restore::status(&restored).unwrap();
    assert_eq!(state.phase, "apply_ui");
    let saved_ui = state.ui.unwrap();
    assert_eq!(saved_ui[&draft], "\"草稿\"");
    assert!(!saved_ui.contains_key("apiKey"));
    assert!(restored.conversations(&original.id).is_err());
    {
        let c = restored.connection().unwrap();
        let evidence: (i64, String) = c.query_row(
            "SELECT r.dataset_version,e.evaluator_version FROM benchmark_runs r JOIN benchmark_evaluations e ON r.workspace_id=e.workspace_id WHERE e.id='old-score'",
            [], |r| Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(evidence, (1, "evaluator-v1".into()));
        let indexed: u32 = c
            .query_row(
                "SELECT count(*) FROM knowledge_fts WHERE knowledge_fts MATCH 'walsnapshotneedle'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(indexed > 0);
    }
    // Interrupted after the database switch and before WebView acknowledgement.
    let id = state.id.unwrap();
    drop(restored);
    let restored = Database::open(&target_dir).unwrap();
    assert_eq!(
        restore::status(&restored).unwrap().id.as_deref(),
        Some(id.as_str())
    );
    let proof = root(&target_before)
        .join("safety")
        .join(format!("恢复前-{id}.cortexbackup"));
    let copy = Scratch::create(&run.0).unwrap();
    let (safety, safety_ui) = unpack(&proof, &copy.0).unwrap();
    assert_eq!(safety.workspaces[0].id, original.id);
    assert_eq!(safety_ui["cortexos.appearance"], "light");
    restore::acknowledge(&restored, &id).unwrap();
    assert!(!restore::pending(&restored.path).exists());
    assert!(
        root(&restored.path)
            .join("history")
            .join(format!("{id}.json"))
            .is_file()
    );
    assert!(from.knowledge_detail(ws, &node).is_ok());
}

#[test]
fn workspace_only_keeps_related_records_and_merges_without_adding_other_workspaces() {
    let run = run_dir();
    let from = open(&run.0, "from");
    let a = from.create_workspace("备份工作区").unwrap().id;
    let b = from.create_workspace("outside-secret-2913").unwrap().id;
    let a_node = from.save_knowledge(file(&a, "keeperneedle")).unwrap();
    from.approve_knowledge(&a, &a_node, 1).unwrap();
    let b_node = from.save_knowledge(file(&b, "outsideneedle")).unwrap();
    from.approve_knowledge(&b, &b_node, 1).unwrap();
    set_history(&from, &a);
    {
        let c = from.connection().unwrap();
        c.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES('ordered-job',?1,'completed',json_object('purpose','harness_workspace'),'fixture','t')", [&a]).unwrap();
        for id in ["z-first", "a-second"] {
            c.execute(
                "INSERT INTO benchmark_generated_cases VALUES('ordered-job',?1,1,'{}','{}','t')",
                [id],
            )
            .unwrap();
        }
    }
    let dest = open(&run.0, "dest");
    let keep = dest.create_workspace("已有工作区").unwrap().id;
    dest.benchmark_datasets(&keep).unwrap();
    let uri = run.0.join("workspace.cortexbackup");
    let inside = format!("cortexos.draft.v1.knowledge.%5B%22{a}%22%2Cnull%5D");
    let outside = format!("cortexos.conversation.{b}");
    let manifest = save(
        &from,
        Some(a.clone()),
        Ui::from([
            (inside.clone(), "{}".into()),
            (outside.clone(), "hidden".into()),
        ]),
        &uri,
    )
    .unwrap();
    let stage_dir = Scratch::create(&run.0).unwrap();
    let (_, ui) = unpack(&uri, &stage_dir.0).unwrap();
    assert!(ui.contains_key(&inside));
    assert!(!ui.contains_key(&outside));
    let raw = fs::read(stage_dir.0.join(DB)).unwrap();
    assert!(
        !raw.windows(b"outsideneedle".len())
            .any(|s| s == b"outsideneedle")
    );
    let dir = dest.path.parent().unwrap().to_owned();
    assert!(restore::stage(&from, ticket(&uri, &manifest), Ui::new()).is_err());
    restore::stage(&dest, ticket(&uri, &manifest), Ui::new()).unwrap();
    drop(dest);
    let dest = Database::open(&dir).unwrap();
    assert_eq!(dest.restore_issue, None);
    assert!(dest.knowledge_detail(&a, &a_node).is_ok());
    assert!(dest.conversations(&keep).is_ok());
    assert!(dest.conversations(&b).is_err());
    assert_eq!(dest.benchmark_datasets(&a).unwrap().len(), 2);
    {
        let c = dest.connection().unwrap();
        let ids = c.prepare("SELECT case_id FROM benchmark_generated_cases WHERE job_id='ordered-job' ORDER BY rowid").unwrap()
            .query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert_eq!(ids, ["z-first", "a-second"]);
    }
    let state = restore::status(&dest).unwrap();
    restore::acknowledge(&dest, &state.id.unwrap()).unwrap();
}

#[test]
fn rejects_corruption_forged_schema_and_insufficient_space_without_replacing_original() {
    let run = run_dir();
    let original = open(&run.0, "original");
    let target = open(&run.0, "target");
    original.create_workspace("新数据").unwrap();
    let keep = target.create_workspace("必须保留").unwrap().id;
    let uri = run.0.join("file.cortexbackup");
    let manifest = save(&original, None, Ui::new(), &uri).unwrap();
    let short = run.0.join("short.cortexbackup");
    let mut bytes = fs::read(&uri).unwrap();
    bytes.truncate(bytes.len() / 2);
    fs::write(&short, bytes).unwrap();
    assert!(unpack(&short, &Scratch::create(&run.0).unwrap().0).is_err());
    let bad = Scratch::create(&run.0).unwrap();
    unpack(&uri, &bad.0).unwrap();
    open_snapshot(&bad.0.join(DB), false)
        .unwrap()
        .execute_batch("CREATE VIEW simulated_unexpected_object AS SELECT 1;")
        .unwrap();
    let forged = run.0.join("unexpected-schema.cortexbackup");
    archive(&bad.0.join(DB), &Ui::new(), None, &forged).unwrap();
    assert!(
        unpack(&forged, &Scratch::create(&run.0).unwrap().0)
            .unwrap_err()
            .contains("数据库结构")
    );
    restore::stage(&target, ticket(&uri, &manifest), Ui::new()).unwrap();
    let p = target.path.clone();
    drop(target);
    let no_space = restore::apply_with_capacity(&p, |_| Some(1)).unwrap_err();
    assert_eq!(no_space, SPACE_ERROR);
    let target = Database::open(p.parent().unwrap()).unwrap();
    assert!(target.conversations(&keep).is_ok());
    restore::stage(&target, ticket(&uri, &manifest), Ui::new()).unwrap();
    fs::write(restore::pending(&p).join(UI), b"{} changed").unwrap();
    drop(target);
    let target = Database::open(p.parent().unwrap()).unwrap();
    assert!(
        target
            .restore_issue
            .as_deref()
            .unwrap()
            .contains("校验失败")
    );
    assert!(target.conversations(&keep).is_ok());
}

#[test]
fn restores_legacy_version_with_migration_and_keeps_knowledge_exports_scoped() {
    let run = run_dir();
    let old_path = run.0.join("legacy.sqlite3");
    let mut old = Connection::open(&old_path).unwrap();
    crate::db::initialize_fixture_schema(&mut old, &run.0, 7).unwrap();
    old.execute(
        "INSERT INTO workspaces VALUES('legacy','旧数据库','old','old')",
        [],
    )
    .unwrap();
    drop(old);
    let file_path = run.0.join("legacy.cortexbackup");
    let manifest = archive(&old_path, &Ui::new(), None, &file_path).unwrap();
    assert_eq!(manifest.schema_version, 7);
    let dest = open(&run.0, "dest");
    let dir = dest.path.parent().unwrap().to_owned();
    restore::stage(&dest, ticket(&file_path, &manifest), Ui::new()).unwrap();
    drop(dest);
    let dest = Database::open(&dir).unwrap();
    assert_eq!(dest.restore_issue, None);
    assert!(backup_schema_version(&*dest.connection().unwrap()).unwrap() > 7);
    assert!(dest.conversations("legacy").is_ok());
    assert_eq!(
        backup_schema_version(&open_snapshot(&old_path, true).unwrap()).unwrap(),
        7
    );
    let node = dest
        .save_knowledge(file("legacy", "portable text"))
        .unwrap();
    {
        let mut c = dest.connection().unwrap();
        let tx = c.transaction().unwrap();
        tx.execute("INSERT INTO source_documents VALUES('source','legacy','C:/h2-input.txt','原文.txt',1,1,1,'t','t')", []).unwrap();
        tx.execute(
            "INSERT INTO source_versions VALUES('source',1,'原始资料',?1,?1,'t')",
            ["a".repeat(64)],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO knowledge_source_links VALUES(?1,1,'source',1,1,1,'原始资料')",
            [&node],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let other = dest.create_workspace("另一个").unwrap().id;
    dest.save_knowledge(file(&other, "other text")).unwrap();
    let doc = knowledge_export::report(&*dest.connection().unwrap(), "legacy").unwrap();
    assert_eq!(doc["versions"].as_array().unwrap().len(), 1);
    assert_eq!(doc["versions"][0]["node_id"], node);
    assert_eq!(doc["sourceLinks"][0]["source_version"], 1);
    assert_eq!(doc["sourceLinks"][0]["start_line"], 1);
    assert_eq!(doc["sourceVersions"][0]["content"], "原始资料");
    let markdown = String::from_utf8(knowledge_export::markdown(&doc)).unwrap();
    assert!(markdown.contains("portable text"));
    assert!(markdown.contains("原文.txt · 版本 1 · 第 1–1 行"));
}
