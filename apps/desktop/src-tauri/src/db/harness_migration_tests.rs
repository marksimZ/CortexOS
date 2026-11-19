use super::*;

#[test]
fn rag_runtime_migration_preserves_v25_and_recovers_pending_experiment() {
    let path = std::env::temp_dir().join(format!("cortex-rag-26-{}",new_id()));
    std::fs::create_dir_all(&path).unwrap();
    let database_path = path.join("cortexos.sqlite3");
    let mut c = Connection::open(&database_path).unwrap();
    migrate(&mut c, &path, &MIGRATIONS[..25]).unwrap();
    c.execute("INSERT INTO workspaces VALUES('w','原工作区','t','t')",[]).unwrap();
    c.execute("INSERT INTO rag_evaluations VALUES('w','fixed-replay','t','{}','test')",[]).unwrap();
    let broken = format!("{} INVALID SQL;",MIGRATIONS[25]);
    let mut migrations = MIGRATIONS[..25].to_vec();
    migrations.push(&broken);
    assert!(migrate(&mut c,&path,&migrations).is_err());
    assert!(!c.prepare("SELECT 1 FROM sqlite_schema WHERE name='rag_runtime_experiments'").unwrap().exists([]).unwrap());
    let backup = std::fs::read_dir(&path).unwrap().flatten()
        .find(|entry|entry.file_name().to_string_lossy().starts_with("before-migration-26-")).unwrap();
    let backup = Connection::open_with_flags(backup.path(),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert!(backup.prepare("SELECT 1 FROM rag_evaluations WHERE id='fixed-replay'").unwrap().exists([]).unwrap());
    assert_eq!(backup.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get::<_,i64>(0)).unwrap(),25);
    drop(backup);
    migrate(&mut c,&path,MIGRATIONS).unwrap();
    c.execute("INSERT INTO rag_runtime_experiments VALUES('w','new','t',NULL,'running','{}','test')",[]).unwrap();
    drop(c);
    let db = Database::open(&path).unwrap();
    {
        let c = db.connection().unwrap();
        assert!(c.prepare("SELECT 1 FROM rag_evaluations WHERE id='fixed-replay'").unwrap().exists([]).unwrap());
        assert!(c.prepare("SELECT 1 FROM rag_runtime_experiments WHERE id='new' AND status='interrupted' AND finished_at IS NOT NULL").unwrap().exists([]).unwrap());
        assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    drop(db);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn runtime_cloud_migration_rolls_back_and_keeps_phase_e_data() {
    let dir = std::env::temp_dir().join(format!("cortex-f2-migration-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..17]).unwrap();
    c.execute("INSERT INTO workspaces VALUES('w','原工作区','t','t')", []).unwrap();

    let invalid = format!("{} INVALID SQL;", MIGRATIONS[17]);
    let mut failed = MIGRATIONS[..17].to_vec();
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert!(!c.prepare("SELECT 1 FROM sqlite_schema WHERE name='model_cloud_connections'").unwrap().exists([]).unwrap());
    let backup = std::fs::read_dir(&dir).unwrap().flatten().find(|entry|
        entry.file_name().to_string_lossy().starts_with("before-migration-18-")).unwrap();
    let backup = Connection::open_with_flags(backup.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(backup.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get::<_, i64>(0)).unwrap(), 17);
    assert!(backup.prepare("SELECT 1 FROM workspaces WHERE id='w'").unwrap().exists([]).unwrap());
    drop(backup);

    migrate(&mut c, &dir, &MIGRATIONS[..18]).unwrap();
    assert_eq!(c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get::<_, i64>(0)).unwrap(), 18);
    assert!(c.prepare("SELECT 1 FROM workspaces WHERE id='w'").unwrap().exists([]).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn harness_migration_backs_up_and_recovers_failed_upgrade() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/harness-d1-migration")
        .join(new_id());
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..10]).unwrap();
    c.execute("INSERT INTO workspaces VALUES('w','existing','t','t')", [])
        .unwrap();
    c.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose) VALUES('c','w','keep chat','t','t','chat')",[]).unwrap();
    let mut bad = MIGRATIONS[..10].to_vec();
    bad.push("CREATE TABLE failed_harness_upgrade(x); INVALID SQL;");
    assert!(migrate(&mut c, &dir, &bad).is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        10
    );
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='failed_harness_upgrade'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let backups: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("before-migration-11-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = Connection::open(backups[0].path()).unwrap();
    assert_eq!(
        backup
            .query_row("SELECT title FROM conversations WHERE id='c'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        "keep chat"
    );
    migrate(&mut c, &dir, &MIGRATIONS[..11]).unwrap();
    assert_eq!(
        c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        c.query_row("SELECT purpose FROM conversations WHERE id='c'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "chat"
    );
}

#[test]
fn rag_migration_preserves_v11_and_rolls_back() {
    let dir = std::env::temp_dir().join(format!("cortex-rag-migration-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..11]).unwrap();
    c.execute("INSERT INTO workspaces VALUES('w','existing','t','t')", [])
        .unwrap();
    c.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose) VALUES('c','w','existing chat','t','t','chat')", []).unwrap();
    let bad_sql = format!("{} INVALID SQL;", MIGRATIONS[11]);
    let mut bad = MIGRATIONS[..11].to_vec();
    bad.push(&bad_sql);
    assert!(migrate(&mut c, &dir, &bad).is_err());
    let version = |c: &Connection| {
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
    };
    let chat = |c: &Connection| {
        c.query_row("SELECT title FROM conversations WHERE id='c'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap()
    };
    let index_tables = |c: &Connection| {
        c.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name LIKE 'rag_%'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    assert_eq!(version(&c), 11);
    assert_eq!(index_tables(&c), 0);
    let backup_path = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .find(|item| {
            item.file_name()
                .to_string_lossy()
                .starts_with("before-migration-12-")
        })
        .unwrap()
        .path();
    let backup = Connection::open(backup_path).unwrap();
    assert_eq!(version(&backup), 11);
    assert_eq!(chat(&backup), "existing chat");
    migrate(&mut c, &dir, &MIGRATIONS[..12]).unwrap();
    assert_eq!(version(&c), 12);
    assert_eq!(index_tables(&c), 3);
    assert_eq!(chat(&c), "existing chat");
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    drop(backup);
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rag_evaluation_migration_preserves_v16_backup_and_rolls_back() {
    let dir = std::env::temp_dir().join(format!("cortex-rag-v16-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..16]).unwrap();
    c.execute("INSERT INTO workspaces VALUES('saved','原工作区','t','t')", []).unwrap();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[16]);
    let mut failed = MIGRATIONS[..16].to_vec();
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert!(!c.prepare("SELECT 1 FROM sqlite_schema WHERE name='rag_evaluations'").unwrap().exists([]).unwrap());
    let backup = std::fs::read_dir(&dir).unwrap().flatten().find(|entry|
        entry.file_name().to_string_lossy().starts_with("before-migration-17-")).unwrap();
    let backup = Connection::open_with_flags(backup.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(backup.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_,i64>(0)).unwrap(), 16);
    assert!(backup.prepare("SELECT 1 FROM workspaces WHERE id='saved'").unwrap().exists([]).unwrap());
    migrate(&mut c, &dir, &MIGRATIONS[..17]).unwrap();
    assert_eq!(c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_,i64>(0)).unwrap(), 17);
    assert!(c.prepare("SELECT 1 FROM workspaces WHERE id='saved'").unwrap().exists([]).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    drop(backup);
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rag_migration_v13_and_v14_preserve_vectors_and_backfill_fts() {
    let dir = std::env::temp_dir().join(format!("cortex-rag-v12-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..12]).unwrap();
    c.execute_batch("BEGIN;
        INSERT INTO workspaces VALUES('w','legacy','t','t');
        INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at)
        VALUES('n','w',1,1,2,'t','t');
        INSERT INTO knowledge_versions VALUES('n',1,'保留','knowledge','SQLite 备份','user','fixture','t');
        INSERT INTO rag_index_profiles VALUES('w','idx','ollama','fixture','digest',1,'l2','approved-knowledge-v1','t');
        INSERT INTO rag_vectors VALUES('w','idx','n',1,'legacy-hash',X'0000803f');
        INSERT INTO rag_workspace_settings VALUES('w','idx',1,0);
        COMMIT;").unwrap();
    let mut failed = MIGRATIONS[..12].to_vec();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[12]);
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert_eq!(
        c.query_row("SELECT hex(vector) FROM rag_vectors", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "0000803F"
    );
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        12
    );

    migrate(&mut c, &dir, &MIGRATIONS[..13]).unwrap();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[13]);
    let mut failed = MIGRATIONS[..13].to_vec();
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        13
    );
    assert!(
        !c.prepare("SELECT 1 FROM sqlite_schema WHERE name='knowledge_snapshots'")
            .unwrap()
            .exists([])
            .unwrap()
    );
    let backup = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("before-migration-14-")
        })
        .unwrap();
    let backup = Connection::open(backup.path()).unwrap();
    assert_eq!(
        backup
            .query_row(
                "SELECT title FROM knowledge_versions WHERE node_id='n'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "保留"
    );
    drop(backup);
    migrate(&mut c, &dir, &MIGRATIONS[..14]).unwrap();
    assert_eq!(
        c.query_row("SELECT hex(vector),chunk_id FROM rag_vectors", [], |r| Ok(
            (r.get::<_, String>(0)?, r.get::<_, String>(1)?)
        ))
        .unwrap(),
        ("0000803F".into(), String::new())
    );
    assert_eq!(
        c.query_row("SELECT index_id FROM rag_workspace_settings", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "idx"
    );
    assert_eq!(
        crate::retrieval::search(&c, "w", "SQLite").unwrap()["candidates"][0]["nodeId"],
        "n"
    );
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        14
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with("before-migration-13-")
    }));
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn knowledge_scope_v15_and_v16_backfill_frozen_relations_and_roll_back() {
    let dir = std::env::temp_dir().join(format!("cortex-knowledge-v14-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Connection::open(dir.join("cortexos.sqlite3")).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&mut c, &dir, &MIGRATIONS[..14]).unwrap();
    c.execute_batch("BEGIN;
        INSERT INTO workspaces VALUES('w','legacy','t','t');
        INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at)
        VALUES('a','w',1,1,2,'t','t'),('b','w',1,1,2,'t','t');
        INSERT INTO knowledge_versions VALUES
        ('a',1,'甲','knowledge','原文甲','user','fixture','t'),('b',1,'乙','knowledge','原文乙','user','fixture','t');
        INSERT INTO knowledge_edges VALUES('edge','w',2),('support','w',2);
        INSERT INTO knowledge_edge_versions VALUES
        ('edge',1,'w','a','b','contradicts',1,'登记','2026-01-01T00:00:00.000Z'),
        ('edge',2,'w','a','b','contradicts',0,'停用','2026-01-03T00:00:00.000Z'),
        ('support',1,'w','b','a','supports',1,'登记','2026-01-01T00:00:00.000Z'),
        ('support',2,'w','b','a','supports',0,'停用','2026-01-03T00:00:00.000Z');
        INSERT INTO knowledge_snapshots VALUES('w','fixed','旧快照','2026-01-02T00:00:00.000Z','2026-01-02',2);
        INSERT INTO knowledge_snapshot_items VALUES('w','fixed','a',1,1),('w','fixed','b',1,2);
        COMMIT;").unwrap();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[14]);
    let mut failed = MIGRATIONS[..14].to_vec();
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert_eq!(
        c.query_row("SELECT count(*) FROM knowledge_snapshot_items", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        2
    );
    assert!(
        !c.prepare("SELECT 1 FROM sqlite_schema WHERE name='knowledge_snapshot_conflicts'")
            .unwrap()
            .exists([])
            .unwrap()
    );
    migrate(&mut c, &dir, &MIGRATIONS[..15]).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT edge_version FROM knowledge_snapshot_conflicts WHERE snapshot_id='fixed'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        15
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with("before-migration-15-")
    }));
    c.execute_batch("BEGIN;
        INSERT INTO knowledge_snapshots VALUES('w','branch','旧实验','2026-01-04T00:00:00.000Z','2026-01-02',2);
        INSERT INTO knowledge_snapshot_items VALUES('w','branch','a',1,1),('w','branch','b',1,2);
        INSERT INTO knowledge_experimental_branches VALUES('w','branch','fixed','[]');
        INSERT INTO knowledge_snapshot_conflicts VALUES('w','branch','edge',1);
        COMMIT;").unwrap();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[15]);
    let mut failed = MIGRATIONS[..15].to_vec();
    failed.push(&invalid);
    assert!(migrate(&mut c, &dir, &failed).is_err());
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM knowledge_snapshot_conflicts",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    migrate(&mut c, &dir, &MIGRATIONS[..16]).unwrap();
    for snapshot in ["fixed", "branch"] {
        let mut stmt=c.prepare("SELECT edge_id,edge_version FROM knowledge_snapshot_relations WHERE snapshot_id=?1 ORDER BY edge_id").unwrap();
        let relations = stmt
            .query_map([snapshot], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(relations, [("edge".into(), 1), ("support".into(), 1)]);
    }
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        16
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}
