use super::*;
use crate::benchmark::profile::{Profile, SaveProfile};
fn directory() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/phase-c2")
        .join(new_id());
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn save(version: u32, action: &str, profile: Profile) -> SaveProfile {
    SaveProfile {
        expected_version: version,
        action: action.into(),
        profile,
    }
}
#[test]
fn profile_versions_workspace_reset_validation_and_reopen() {
    let path = directory();
    let db = Database::open(&path).unwrap();
    let a = db.create_workspace("A").unwrap();
    let b = db.create_workspace("B").unwrap();
    let mut profile = Profile::default();
    profile.domains = vec!["software".into(), "science".into()];
    profile.tools = vec!["Rust".into(), "Excel".into()];
    let first = db
        .save_benchmark_profile(&a.id, save(0, "save", profile.clone()))
        .unwrap();
    assert_eq!(first.version, 1);
    assert_eq!(first.profile, profile);
    assert!(db.benchmark_profile(&b.id).unwrap().is_none());
    assert!(
        db.save_benchmark_profile(&a.id, save(0, "save", profile.clone()))
            .is_err()
    );
    assert!(
        db.save_benchmark_profile("missing", save(0, "save", profile.clone()))
            .is_err()
    );
    let mut bad = profile.clone();
    bad.weights.insert("quality".into(), 6);
    assert!(
        db.save_benchmark_profile(&a.id, save(1, "save", bad))
            .is_err()
    );
    let mut bad = profile.clone();
    bad.weights.values_mut().for_each(|v| *v = 0);
    assert!(
        db.save_benchmark_profile(&a.id, save(1, "save", bad))
            .is_err()
    );
    let mut bad = profile.clone();
    bad.domains.push("invented".into());
    assert!(
        db.save_benchmark_profile(&a.id, save(1, "save", bad))
            .is_err()
    );
    let mut bad = profile.clone();
    bad.tools = vec!["x".repeat(61)];
    assert!(
        db.save_benchmark_profile(&a.id, save(1, "save", bad))
            .is_err()
    );
    assert!(
        db.connection()
            .unwrap()
            .execute("UPDATE benchmark_profile_versions SET action='reset'", [])
            .is_err()
    );
    assert!(
        db.connection()
            .unwrap()
            .execute("DELETE FROM benchmark_profile_versions", [])
            .is_err()
    );
    let reset = db
        .save_benchmark_profile(&a.id, save(1, "reset", profile.clone()))
        .unwrap();
    assert_eq!(reset.profile, Profile::default());
    let skipped = db
        .save_benchmark_profile(&a.id, save(2, "skip", profile))
        .unwrap();
    assert_eq!(skipped.version, 3);
    drop(db);
    let db = Database::open(&path).unwrap();
    assert_eq!(db.benchmark_profile(&a.id).unwrap().unwrap().version, 3);
    let c = db.connection().unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM benchmark_profile_versions", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
    let old:String=c.query_row("SELECT content_hash FROM benchmark_profile_versions WHERE workspace_id=?1 AND version=1",[a.id],|r|r.get(0)).unwrap();
    assert_eq!(old, first.content_hash);
}
#[test]
fn profile_migration_backup_failure_retry_preserves_v8() {
    let path = directory();
    let mut c = Connection::open(path.join("cortexos.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&mut c, &path, &MIGRATIONS[..8]).unwrap();
    c.execute(
        "INSERT INTO workspaces VALUES('old','Existing','t','t')",
        [],
    )
    .unwrap();
    c.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose) VALUES('chat','old','Existing chat','t','t','chat')",[]).unwrap();
    let old: Vec<String> = c
        .prepare("SELECT checksum FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut broken = MIGRATIONS[..8].to_vec();
    broken.push("CREATE TABLE temporary_partial(id INTEGER); INVALID SQL;");
    assert!(migrate(&mut c, &path, &broken).is_err());
    assert_eq!(
        c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        8
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='temporary_partial'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    migrate(&mut c, &path, &MIGRATIONS[..9]).unwrap();
    assert_eq!(
        c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        9
    );
    assert_eq!(
        c.query_row("SELECT title FROM conversations WHERE id='chat'", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Existing chat"
    );
    let after: Vec<String> = c
        .prepare("SELECT checksum FROM schema_migrations WHERE version<=8 ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(after, old);
    let backups: Vec<_> = std::fs::read_dir(&path)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("before-migration-9-")
        })
        .collect();
    assert_eq!(backups.len(), 2);
    for backup in backups {
        let copy = Connection::open(backup).unwrap();
        assert_eq!(
            copy.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            8
        );
        assert_eq!(
            copy.query_row("SELECT count(*) FROM conversations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

#[test]
fn personalized_migration_v10_backs_up_rolls_back_and_keeps_prior_checksums() {
    let path = directory();
    let mut c = Connection::open(path.join("cortexos.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&mut c, &path, &MIGRATIONS[..9]).unwrap();
    c.execute(
        "INSERT INTO workspaces VALUES('old','Existing','t','t')",
        [],
    )
    .unwrap();
    let old: Vec<String> = c
        .prepare("SELECT checksum FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut broken = MIGRATIONS[..9].to_vec();
    let invalid = format!("{} INVALID SQL;", MIGRATIONS[9]);
    broken.push(&invalid);
    assert!(migrate(&mut c, &path, &broken).is_err());
    assert_eq!(
        c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        9
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='benchmark_generation_jobs'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    migrate(&mut c, &path, &MIGRATIONS[..10]).unwrap();
    assert_eq!(
        c.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        10
    );
    let after: Vec<String> = c
        .prepare("SELECT checksum FROM schema_migrations WHERE version<=9 ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(after, old);
    let backups: Vec<_> = std::fs::read_dir(&path)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("before-migration-10-")
        })
        .collect();
    assert_eq!(
        backups.len(),
        2,
        "Both failed and successful migration attempts must preserve a readable v9 backup"
    );
    for p in backups {
        let backup = Connection::open(p).unwrap();
        assert_eq!(
            backup
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            9
        );
        assert_eq!(
            backup
                .query_row("SELECT name FROM workspaces WHERE id='old'", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            "Existing"
        );
    }
}
