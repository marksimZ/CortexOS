use super::*;
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
    migrate(&mut c, &dir, MIGRATIONS).unwrap();
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
