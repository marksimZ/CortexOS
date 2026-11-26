use super::*;
use rusqlite::types::ValueRef;
use std::collections::BTreeMap;

fn directory(label: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/benchmark-slice1")
        .join(format!("{label}-{}", new_id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn seed_execution(
    c: &Connection,
    workspace: &str,
    id: &str,
    purpose: Option<&str>,
    status: &str,
) -> RunSummary {
    let conversation = format!("{id}-conversation");
    let answer = format!("{id}-answer");
    let settings = ModelSettings {
        model: "isolated-fixture-model".into(),
        ..ModelSettings::default()
    };
    let tx = c.unchecked_transaction().unwrap();
    if let Some(purpose) = purpose {
        tx.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose) VALUES(?1,?2,'Fixture','t','t',?3)", params![conversation,workspace,purpose]).unwrap();
    } else {
        tx.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at) VALUES(?1,?2,'Fixture','t','t')", params![conversation,workspace]).unwrap();
    }
    tx.execute("INSERT INTO execution_runs(id,workspace_id,conversation_id,assistant_message_id,model_id,model_config_json,context_manifest_json,status,started_at,finished_at) VALUES(?1,?2,?3,?4,?5,?6,'{}',?7,'t',CASE WHEN ?7='running' THEN NULL ELSE 't' END)",params![id,workspace,conversation,answer,settings.model,serde_json::to_string(&settings).unwrap(),status]).unwrap();
    tx.execute(
        "INSERT INTO messages(id,workspace_id,conversation_id,seq,role,content,status,model_run_id,created_at,updated_at) VALUES(?1,?2,?3,1,'user','fixture question','completed',?4,'t','t')",
        params![format!("{id}-question"), workspace, conversation, id],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO messages(id,workspace_id,conversation_id,seq,role,content,status,model_run_id,created_at,updated_at) VALUES(?1,?2,?3,2,'assistant','saved fixture output',?4,?5,'t','t')",
        params![
            answer,
            workspace,
            conversation,
            if status == "running" {
                "streaming"
            } else {
                status
            },
            id
        ],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO context_snapshots VALUES(?1,'{}',?2,'{}','t')",
        params![id, hash("{}")],
    )
    .unwrap();
    event(&tx, id, "fixture_seed", json!({"synthetic":true})).unwrap();
    tx.commit().unwrap();
    RunSummary {
        id: id.into(),
        workspace_id: workspace.into(),
        conversation_id: conversation,
        assistant_message_id: answer,
        model_id: settings.model,
        status: status.into(),
        started_at: "t".into(),
        finished_at: if status == "running" {
            None
        } else {
            Some("t".into())
        },
        error_message: None,
    }
}

fn legacy_seven(path: &Path) -> Connection {
    let mut c = Connection::open(path.join("cortexos.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    migrate(&mut c, path, &MIGRATIONS[..7]).unwrap();
    c.execute_batch("INSERT INTO workspaces VALUES('w','Production fixture','t','t'); INSERT INTO workspaces VALUES('other','Other fixture','t','t');").unwrap();
    seed_execution(&c, "w", "old-chat", None, "completed");
    c.execute_batch(r#"
      BEGIN;
      INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at) VALUES('n1','w',2,1,2,'t','t'),('n2','w',1,1,1,'t','t');
      INSERT INTO knowledge_versions VALUES('n1',1,'Old approved','knowledge','approved evidence','user','fixture','t'),('n1',2,'New draft','knowledge','new draft evidence','user','fixture','t'),('n2',1,'Decision','decision','fixture decision','user','fixture','t');
      INSERT INTO knowledge_approval_events VALUES('approval1','n1',1,'user','t'),('approval2','n2',1,'user','t');
      INSERT INTO source_documents VALUES('s','w','fixture.md','fixture.md',2,1,2,'t','t');
      INSERT INTO source_versions VALUES('s',1,'approved evidence','fixture-hash-1','fixture-raw-1','t'),('s',2,'new draft evidence','fixture-hash-2','fixture-raw-2','t');
      INSERT INTO source_events VALUES('se1','s','imported','t'),('se2','s','reimported','t');
      INSERT INTO knowledge_source_links VALUES('n1',1,'s',1,1,1,'approved evidence'),('n1',2,'s',2,1,1,'new draft evidence');
      INSERT INTO knowledge_edges VALUES('edge','w',2);
      INSERT INTO knowledge_edge_versions VALUES('edge',1,'w','n1','n2','supports',1,'fixture','t'),('edge',2,'w','n1','n2','supports',0,'fixture','t');
      INSERT INTO graph_layouts VALUES('w','knowledge',1,'[12,10,28]','[0,0,0]','t');
      INSERT INTO knowledge_positions VALUES('w','n1',1,2,3),('w','n2',4,5,6);
      INSERT INTO execution_layouts VALUES('old-chat','w',1,'[12,10,28]','[0,0,0]','t');
      INSERT INTO conversation_retrieval_settings VALUES('old-chat-conversation','w',1);
      INSERT INTO context_knowledge_items VALUES('old-chat',1,'n1',1);
      INSERT INTO retrieval_results VALUES('old-chat',1,'n1',1,1.5,1,'fixture',10);
      INSERT INTO app_settings VALUES('model','{"model":"isolated-fixture-model","numCtx":4096,"temperature":0.3,"numPredict":512,"timeoutSeconds":180}');
      COMMIT;
    "#).unwrap();
    crate::retrieval::rebuild(&c).unwrap();
    c
}

type Columns = BTreeMap<String, Vec<String>>;
fn columns(c: &Connection) -> Columns {
    let names: Vec<String> = c.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name!='schema_migrations' ORDER BY name").unwrap()
        .query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    names
        .into_iter()
        .map(|name| {
            let fields = c
                .prepare(&format!(
                    "PRAGMA table_info(\"{}\")",
                    name.replace('"', "\"\"")
                ))
                .unwrap()
                .query_map([], |r| r.get(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<String>>>()
                .unwrap();
            (name, fields)
        })
        .collect()
}
fn snapshot(c: &Connection, schema: &Columns) -> Value {
    let mut result = serde_json::Map::new();
    for (table, fields) in schema {
        let projection = fields
            .iter()
            .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",");
        let order = (1..=fields.len())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = c
            .prepare(&format!(
                "SELECT {projection} FROM \"{}\" ORDER BY {order}",
                table.replace('"', "\"\"")
            ))
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                (0..fields.len())
                    .map(|i| {
                        Ok(match row.get_ref(i)? {
                            ValueRef::Null => Value::Null,
                            ValueRef::Integer(v) => json!(v),
                            ValueRef::Real(v) => json!(v),
                            ValueRef::Text(v) => json!(String::from_utf8_lossy(v)),
                            ValueRef::Blob(v) => json!(format!("blob:{v:?}")),
                        })
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        result.insert(
            table.clone(),
            json!({"rows":rows.len(),"hash":hash(&serde_json::to_string(&rows).unwrap())}),
        );
    }
    Value::Object(result)
}
fn backups(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("before-migration-8-")
        })
        .collect()
}
fn version(c: &Connection) -> i64 {
    c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
        r.get(0)
    })
    .unwrap()
}
fn integrity(c: &Connection) {
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        !c.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_some()
    );
}

fn durable_snapshot(mut value: Value) -> Value {
    // Retrieval rebuild can change FTS5's internal segment encodings. Compare the public
    // index and its MATCH results alongside byte-for-byte durable rows.
    for name in [
        "knowledge_fts_data",
        "knowledge_fts_docsize",
        "knowledge_fts_idx",
        "knowledge_fts_config",
        "knowledge_fts_content",
    ] {
        value.as_object_mut().unwrap().remove(name);
    }
    value
}

fn assert_migrated_snapshot(c: &Connection, schema: &Columns, before: &Value) {
    assert_eq!(
        durable_snapshot(snapshot(c, schema)),
        durable_snapshot(before.clone())
    );
    let indexed: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM knowledge_fts WHERE knowledge_fts MATCH 'approved'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(indexed > 0);
}

fn seed_plan(c: &Connection, id: &str, workspace: &str) -> String {
    c.execute(
        "INSERT OR IGNORE INTO benchmark_datasets VALUES('fixture',1,'Fixture',1,?1,'{}','{}','t')",
        params![hash("fixture")],
    )
    .unwrap();
    c.execute("INSERT OR IGNORE INTO benchmark_cases VALUES('fixture',1,'case','math_logic','{}','{}','{}','easy','json','[]',NULL,?1)",params![hash("case")]).unwrap();
    c.execute("INSERT INTO benchmark_runs(id,workspace_id,dataset_id,dataset_version,launch_key,status,config_snapshot_json,config_hash,hardware_snapshot_json,app_version,source_revision,evaluator_version,created_at) VALUES(?1,?2,'fixture',1,?1,'pending','{\"repetitions\":1}',?3,'{}','fixture','fixture','fixture','t')",params![id,workspace,hash("config")]).unwrap();
    c.execute(
        "INSERT INTO benchmark_run_models VALUES(?1,0,?2,'{}',?3)",
        params![id, workspace, hash("model")],
    )
    .unwrap();
    let case = format!("{id}-case");
    c.execute("INSERT INTO benchmark_case_runs VALUES(?1,?2,?3,0,'fixture',1,'case',1,'single','pending')",params![case,workspace,id]).unwrap();
    case
}

#[test]
fn fresh_and_seven_upgrade_preserve_data_and_backup() {
    let path = directory("upgrade");
    let old = legacy_seven(&path);
    let schema = columns(&old);
    let before = snapshot(&old, &schema);
    let old_checksums: Vec<String> = old
        .prepare("SELECT checksum FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    drop(old);
    let db = Database::open(&path).unwrap();
    {
        let c = db.connection().unwrap();
        assert_eq!(version(&c), MIGRATIONS.len() as i64);
        assert_migrated_snapshot(&c, &schema, &before);
        assert_eq!(
            c.query_row(
                "SELECT purpose FROM conversations WHERE id='old-chat-conversation'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "chat"
        );
        let after: Vec<String> = c
            .prepare("SELECT checksum FROM schema_migrations WHERE version<=7 ORDER BY version")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(after, old_checksums);
        for table in [
            "benchmark_runs",
            "benchmark_cases",
            "benchmark_attempts",
            "benchmark_events",
        ] {
            assert!(
                c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get::<_, bool>(0)
                )
                .unwrap(),
                "missing {table}"
            );
        }
        integrity(&c);
    }
    let backup = backups(&path);
    assert_eq!(backup.len(), 1);
    let copy = Connection::open(&backup[0]).unwrap();
    assert_eq!(version(&copy), 7);
    assert_eq!(snapshot(&copy, &schema), before);
    integrity(&copy);
    drop(copy);
    db.create_conversation("w").unwrap();
    assert_eq!(
        db.messages("w", "old-chat-conversation").unwrap()[1].content,
        "saved fixture output"
    );
    assert_eq!(
        db.execution_graph("w", "old-chat").unwrap().output.content,
        "saved fixture output"
    );
    drop(db);
    let again = Database::open(&path).unwrap();
    assert_eq!(
        version(&again.connection().unwrap()),
        MIGRATIONS.len() as i64
    );
    assert_eq!(backups(&path).len(), 1);
    std::fs::write(path.join("upgrade-evidence.json"),serde_json::to_vec_pretty(&json!({"version":MIGRATIONS.len(),"backupVersion":7,"oldTables":before,"allOldRowsAndChecksumsEqual":true})).unwrap()).unwrap();
    let fresh = Database::open(&directory("fresh")).unwrap();
    assert_eq!(
        version(&fresh.connection().unwrap()),
        MIGRATIONS.len() as i64
    );
}

#[test]
fn migration_failure_rolls_back_and_can_retry() {
    let path = directory("rollback");
    let mut c = legacy_seven(&path);
    let schema = columns(&c);
    let before = snapshot(&c, &schema);
    let failed = format!(
        "{}\nINSERT INTO deliberately_missing_table VALUES(1);",
        MIGRATIONS[7]
    );
    let mut bad = MIGRATIONS[..7].to_vec();
    bad.push(&failed);
    assert!(migrate(&mut c, &path, &bad).is_err());
    assert!(c.is_autocommit());
    assert_eq!(version(&c), 7);
    assert_eq!(columns(&c), schema);
    assert_eq!(snapshot(&c, &schema), before);
    let backup = Connection::open(&backups(&path)[0]).unwrap();
    assert_eq!(snapshot(&backup, &schema), before);
    integrity(&backup);
    drop(backup);
    let restored_path = directory("restored-backup");
    std::fs::copy(&backups(&path)[0], restored_path.join("cortexos.sqlite3")).unwrap();
    let restored = Database::open(&restored_path).unwrap();
    assert_eq!(
        version(&restored.connection().unwrap()),
        MIGRATIONS.len() as i64
    );
    assert_migrated_snapshot(&restored.connection().unwrap(), &schema, &before);
    migrate(&mut c, &path, MIGRATIONS).unwrap();
    assert_eq!(version(&c), MIGRATIONS.len() as i64);
    assert_migrated_snapshot(&c, &schema, &before);
    integrity(&c);
}

#[test]
fn backup_failure_stops_before_any_schema_change() {
    let path = directory("backup-failure");
    let mut c = legacy_seven(&path);
    let schema = columns(&c);
    let before = snapshot(&c, &schema);
    // Exercise the real VACUUM INTO error path with an unavailable destination directory.
    let failure = migrate(&mut c, &path.join("missing-backup-directory"), MIGRATIONS).unwrap_err();
    assert!(failure.contains("备份失败"));
    assert_eq!(version(&c), 7);
    assert_eq!(columns(&c), schema);
    assert_eq!(snapshot(&c, &schema), before);
    assert!(backups(&path).is_empty());
    integrity(&c);
    migrate(&mut c, &path, MIGRATIONS).unwrap();
    assert_eq!(version(&c), MIGRATIONS.len() as i64);
}

#[test]
fn checksum_mismatch_and_old_app_reject_before_changes() {
    let path = directory("checksum");
    let mut c = legacy_seven(&path);
    c.execute(
        "UPDATE schema_migrations SET checksum='tampered' WHERE version=1",
        [],
    )
    .unwrap();
    assert!(migrate(&mut c, &path, MIGRATIONS).is_err());
    assert_eq!(version(&c), 7);
    assert!(backups(&path).is_empty());
    c.execute(
        "UPDATE schema_migrations SET checksum=?1 WHERE version=1",
        params![hash(MIGRATIONS[0])],
    )
    .unwrap();
    migrate(&mut c, &path, MIGRATIONS).unwrap();
    assert!(migrate(&mut c, &path, &MIGRATIONS[..7]).is_err());
    assert_eq!(version(&c), MIGRATIONS.len() as i64);
}

#[test]
fn chat_access_cannot_read_write_or_promote_benchmark_records() {
    let path = directory("purpose");
    let old = legacy_seven(&path);
    drop(old);
    let db = Database::open(&path).unwrap();
    let bench = {
        let c = db.connection().unwrap();
        seed_execution(&c, "w", "bench", Some("benchmark"), "completed")
    };
    let (schema, before) = {
        let c = db.connection().unwrap();
        let schema = columns(&c);
        let before = snapshot(&c, &schema);
        (schema, before)
    };
    let preview = db
        .prepare_context_run(
            &new_id(),
            "w",
            "",
            "approved evidence",
            &ModelSettings::default(),
            &[],
            true,
        )
        .unwrap();
    assert_eq!(
        preview.manifest["knowledgeRefs"].as_array().unwrap().len(),
        1
    );
    assert!(
        db.prepare_context_run(
            &new_id(),
            "missing-workspace",
            "",
            "approved evidence",
            &ModelSettings::default(),
            &[],
            true
        )
        .is_err()
    );
    assert_eq!(db.conversations("w").unwrap().len(), 1);
    assert_eq!(db.list_runs("w", 0).unwrap().len(), 1);
    assert!(db.messages("w", &bench.conversation_id).is_err());
    assert!(db.messages("other", &bench.conversation_id).is_err());
    assert!(
        db.rename_conversation("w", &bench.conversation_id, "bad")
            .is_err()
    );
    assert!(db.run_detail("w", &bench.id).is_err());
    assert!(db.run_detail("other", &bench.id).is_err());
    assert!(db.execution_graph("w", &bench.id).is_err());
    assert!(
        db.save_execution_view(crate::execution::SaveExecutionView {
            workspace_id: "w".into(),
            run_id: bench.id.clone(),
            expected_revision: 0,
            camera: [12., 10., 28.],
            target: [0., 0., 0.]
        })
        .is_err()
    );
    for preview in [false, true] {
        assert!(
            db.prepare_context_run(
                &new_id(),
                "w",
                &bench.conversation_id,
                "should not run",
                &ModelSettings::default(),
                &[],
                preview
            )
            .is_err()
        );
    }
    assert!(
        db.set_conversation_retrieval("w", &bench.conversation_id, true)
            .is_err()
    );
    assert!(db.record_event(&bench.id, "forbidden", json!({})).is_err());
    assert!(db.checkpoint(&bench.id, "bad").is_err());
    assert!(
        db.finish(&bench, "bad", "completed", None, json!({}))
            .is_err()
    );
    let mut wrong_identity = db.run_detail("w", "old-chat").unwrap().run;
    wrong_identity.assistant_message_id = bench.assistant_message_id.clone();
    assert!(
        db.finish(&wrong_identity, "bad", "completed", None, json!({}))
            .is_err()
    );
    assert!(
        db.save_knowledge(crate::knowledge::SaveKnowledge {
            workspace_id: "w".into(),
            node_id: None,
            expected_revision: None,
            title: "Bad promotion".into(),
            kind: "knowledge".into(),
            content: "saved fixture output".into(),
            change_reason: "fixture".into(),
            source_message_id: Some(bench.assistant_message_id.clone()),
            source_excerpt: Some("saved fixture output".into())
        })
        .is_err()
    );
    let c = db.connection().unwrap();
    {
        let tx = c.unchecked_transaction().unwrap();
        let failure = tx.execute("INSERT INTO knowledge_nodes(id,workspace_id,latest_version,revision,created_at,updated_at,source_conversation_id,source_message_id,source_excerpt,source_hash) VALUES('bad-origin','w',1,1,'t','t',?1,?2,'saved fixture output','fixture')",params![bench.conversation_id,bench.assistant_message_id]).unwrap_err();
        assert!(failure.to_string().contains("Benchmark output"));
    }
    assert!(crate::retrieval::enabled(&c, "w", &bench.conversation_id).is_err());
    assert!(
        c.execute(
            "UPDATE conversations SET purpose='chat' WHERE id=?1",
            params![bench.conversation_id]
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO context_knowledge_items VALUES('bench',1,'n1',1)",
            []
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO retrieval_results VALUES('bench',1,'n1',1,1,1,'bad',10)",
            []
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO conversation_retrieval_settings VALUES(?1,'w',1)",
            params![bench.conversation_id]
        )
        .is_err()
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM knowledge_nodes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT content FROM messages WHERE id=?1",
            params![bench.assistant_message_id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "saved fixture output"
    );
    assert_eq!(snapshot(&c, &schema), before);
    integrity(&c);
}

#[test]
fn benchmark_constraints_enforce_scope_plan_and_immutable_facts() {
    let path = directory("constraints");
    let old = legacy_seven(&path);
    drop(old);
    let db = Database::open(&path).unwrap();
    let c = db.connection().unwrap();
    let case = seed_plan(&c, "b", "w");
    let other_case = seed_plan(&c, "other-b", "other");
    let bench = seed_execution(&c, "w", "bench", Some("benchmark"), "completed");
    c.execute("INSERT INTO benchmark_attempts VALUES('a','w',?1,1,?2,'completed','finished','t','t',NULL)",params![case,bench.id]).unwrap();
    assert!(c.execute("INSERT INTO benchmark_attempts VALUES('wrong-workspace','other',?1,1,'bench','completed','finished','t','t',NULL)",params![other_case]).is_err());
    assert!(c.execute("INSERT INTO benchmark_attempts VALUES('chat-origin','w',?1,2,'old-chat','completed','finished','t','t',NULL)",params![case]).is_err());
    assert!(c.execute("INSERT INTO benchmark_attempts VALUES('wrong-case','other',?1,1,NULL,'pending','not_sent',NULL,NULL,NULL)",params![case]).is_err());
    assert!(c.execute("INSERT INTO benchmark_case_runs VALUES('too-many','w','b',0,'fixture',1,'case',2,'single','pending')",[]).is_err());
    assert!(c.execute("INSERT INTO benchmark_case_runs VALUES('wrong-version','w','b',0,'fixture',2,'case',1,'single','pending')",[]).is_err());
    c.execute(
        "UPDATE benchmark_runs SET status='running' WHERE id='b'",
        [],
    )
    .unwrap();
    assert!(
        c.execute(
            "UPDATE benchmark_runs SET status='running' WHERE id='other-b'",
            []
        )
        .is_err()
    );
    c.execute(
        "INSERT INTO benchmark_events VALUES('b',1,'w','fixture','t','a','{}')",
        [],
    )
    .unwrap();
    assert!(
        c.execute(
            "INSERT INTO benchmark_events VALUES('other-b',1,'other','fixture','t','a','{}')",
            []
        )
        .is_err()
    );
    c.execute("INSERT INTO benchmark_evaluations VALUES('ev','w','a','fixture','1',?1,?1,'{}','{}','completed','t')",params![hash("fixture")]).unwrap();
    c.execute("INSERT INTO benchmark_resource_samples VALUES('a',1,'w',0,'fixture','whole_gpu','0','{\"memoryBytes\":null}','unavailable',1000)",[]).unwrap();
    for sql in [
        "UPDATE benchmark_datasets SET name='bad'",
        "DELETE FROM benchmark_cases",
        "UPDATE benchmark_runs SET config_snapshot_json='{}'",
        "UPDATE benchmark_run_models SET profile_snapshot_json='{}'",
        "UPDATE benchmark_case_runs SET model_slot=1",
        "UPDATE benchmark_attempts SET status='running'",
        "DELETE FROM benchmark_evaluations",
        "DELETE FROM benchmark_events",
        "DELETE FROM benchmark_resource_samples",
    ] {
        assert!(c.execute(sql, []).is_err(), "unexpectedly allowed: {sql}");
    }
    integrity(&c);
}

#[test]
fn restart_recovers_benchmark_without_exposing_it_as_chat() {
    let path = directory("restart");
    let old = legacy_seven(&path);
    drop(old);
    {
        let db = Database::open(&path).unwrap();
        let c = db.connection().unwrap();
        let case = seed_plan(&c, "b", "w");
        let bench = seed_execution(&c, "w", "bench", Some("benchmark"), "running");
        c.execute(
            "UPDATE benchmark_runs SET status='running' WHERE id='b'",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE benchmark_case_runs SET status='running' WHERE id=?1",
            params![case],
        )
        .unwrap();
        c.execute("INSERT INTO benchmark_attempts VALUES('a','w',?1,1,?2,'running','unknown','t',NULL,NULL)",params![case,bench.id]).unwrap();
        drop(c);
        assert!(db.bootstrap().unwrap().active_run.is_none());
    }
    let db = Database::open(&path).unwrap();
    assert!(db.bootstrap().unwrap().active_run.is_none());
    assert_eq!(db.list_runs("w", 0).unwrap().len(), 1);
    {
        let c = db.connection().unwrap();
        for table in [
            "execution_runs",
            "benchmark_runs",
            "benchmark_attempts",
            "benchmark_case_runs",
        ] {
            let status: String = c
                .query_row(
                    &format!("SELECT status FROM {table} WHERE id=?1"),
                    params![if table == "execution_runs" {
                        "bench"
                    } else if table == "benchmark_runs" {
                        "b"
                    } else if table == "benchmark_attempts" {
                        "a"
                    } else {
                        "b-case"
                    }],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(status, "interrupted");
        }
        assert_eq!(
            c.query_row(
                "SELECT content FROM messages WHERE id='bench-answer'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "saved fixture output"
        );
        assert_eq!(c.query_row("SELECT COUNT(*) FROM execution_events WHERE run_id='bench' AND kind='run_interrupted'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM benchmark_events WHERE run_id='b'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        integrity(&c);
    }
    drop(db);
    let again = Database::open(&path).unwrap();
    assert_eq!(
        again
            .connection()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM benchmark_events WHERE run_id='b'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn completed_output_is_reconciled_without_reexecution() {
    let path = directory("completed-recovery");
    let old = legacy_seven(&path);
    drop(old);
    {
        let db = Database::open(&path).unwrap();
        let c = db.connection().unwrap();
        let case = seed_plan(&c, "b", "w");
        let bench = seed_execution(&c, "w", "bench", Some("benchmark"), "completed");
        c.execute(
            "UPDATE benchmark_runs SET status='running' WHERE id='b'",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE benchmark_case_runs SET status='running' WHERE id=?1",
            params![case],
        )
        .unwrap();
        c.execute("INSERT INTO benchmark_attempts VALUES('a','w',?1,1,?2,'running','unknown','t',NULL,NULL)",params![case,bench.id]).unwrap();
    }
    let db = Database::open(&path).unwrap();
    let c = db.connection().unwrap();
    assert_eq!(
        c.query_row(
            "SELECT status FROM benchmark_attempts WHERE id='a'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "completed"
    );
    assert_eq!(
        c.query_row(
            "SELECT request_disposition FROM benchmark_attempts WHERE id='a'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "finished"
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM execution_runs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM benchmark_evaluations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM execution_events WHERE run_id='bench'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    integrity(&c);
}
