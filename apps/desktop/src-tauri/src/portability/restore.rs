use super::*;

const REQUEST: &str = "request.json";
const OLD_UI: &str = "previous-ui.json";
const ACTIVE_KEY: &str = "h2.restore";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: String,
    manifest: Manifest,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub(super) phase: &'static str,
    message: Option<String>,
    pub(super) id: Option<String>,
    scope: Option<String>,
    pub(super) ui: Option<Ui>,
}

pub(super) fn pending(path: &Path) -> PathBuf {
    root(path).join("pending")
}

fn read_request(dir: &Path) -> AppResult<Request> {
    if !fs::symlink_metadata(dir).is_ok_and(|v| v.is_dir() && !crate::storage::is_link(&v)) {
        return Err(INVALID.into());
    }
    let req: Request = parse_json(
        File::open(dir.join(REQUEST)).map_err(io_error)?,
        MAX_MANIFEST + 65536,
    )?;
    uuid::Uuid::parse_str(&req.id).map_err(invalid)?;
    validate_manifest(&req.manifest)?;
    Ok(req)
}

fn marker(c: &Connection) -> AppResult<Option<String>> {
    c.query_row(
        "SELECT json_extract(value_json,'$') FROM app_settings WHERE key=?1",
        [ACTIVE_KEY],
        |r| r.get(0),
    )
    .optional()
    .map_err(error)
}

fn safety_path(db_path: &Path, req: &Request) -> PathBuf {
    root(db_path)
        .join("safety")
        .join(format!("恢复前-{}.cortexbackup", req.id))
}

fn staged_space(
    dir: &Path,
    path: &Path,
    manifest: &Manifest,
    free: impl FnOnce(&Path) -> Option<u64>,
) -> AppResult<()> {
    let old_db = fs::metadata(path).map_or(0, |meta| meta.len());
    let wal =
        fs::metadata(path.with_file_name("cortexos.sqlite3-wal")).map_or(0, |meta| meta.len());
    let new_db = manifest
        .files
        .iter()
        .find(|f| f.name == DB)
        .ok_or(INVALID)?
        .bytes;
    let old_bytes = old_db.saturating_add(wal);
    // A checked candidate, optional migration backup, original DB snapshot and safety archive.
    let need = new_db
        .saturating_mul(3)
        .saturating_add(old_bytes.saturating_mul(4))
        .saturating_add(2 * MAX_UI + 4 * 1024 * 1024);
    available_space(free(dir), need)
}

pub(super) fn stage(db: &Database, backup: OpenedBackup, mut previous: Ui) -> AppResult<()> {
    let pending = pending(&db.path);
    if pending.exists() {
        return Err("已有等待重启的恢复，请先处理。".into());
    }
    staged_space(
        db.path.parent().ok_or(IO_ERROR)?,
        &db.path,
        &backup.manifest,
        crate::storage::disk_free_bytes,
    )?;
    if let Some(workspace) = backup.manifest.workspace_id.as_deref() {
        if db
            .connection()?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
                [workspace],
                |r| r.get::<_, bool>(0),
            )
            .map_err(error)?
        {
            return Err("此工作区已存在。可以恢复到另一个实例，当前工作区未替换。".into());
        }
    }
    let work = Scratch::create(&root(&db.path))?;
    let (manifest, _) = unpack(&backup.path, &work.0)?;
    if manifest != backup.manifest {
        return Err("备份已变化，请重新打开并查看清单。".into());
    }
    previous.retain(|key, _| ui_key_allowed(key, None));
    check_ui_size(&previous)?;
    write_json(&work.0.join(OLD_UI), &previous)?;
    write_json(
        &work.0.join(REQUEST),
        &Request {
            id: new_id(),
            manifest,
            path: backup.path.to_string_lossy().into_owned(),
        },
    )?;
    fs::rename(&work.0, &pending).map_err(io_error)
}

fn check_staged(dir: &Path, manifest: &Manifest) -> AppResult<()> {
    for item in &manifest.files {
        if !matches!(item.name.as_str(), DB | UI) {
            return Err(INVALID.into());
        }
        let file = File::open(dir.join(&item.name)).map_err(io_error)?;
        let (bytes, hash) = hash_stream(file.take(item.bytes.saturating_add(1)), io::sink())?;
        if bytes != item.bytes || hash != item.sha256 {
            return Err("暂存文件校验失败，当前数据未替换。".into());
        }
    }
    validate_extracted(manifest, dir).map(|_| ())
}

fn prepare_candidate(
    db_path: &Path,
    req: &Request,
    dir: &Path,
    free: impl FnOnce(&Path) -> Option<u64>,
) -> AppResult<()> {
    staged_space(
        db_path.parent().ok_or(IO_ERROR)?,
        db_path,
        &req.manifest,
        free,
    )?;
    check_staged(dir, &req.manifest)?;
    let work = Scratch::create(&root(db_path))?;
    let original = work.0.join("original.sqlite3");
    let received = work.0.join("received.sqlite3");
    let old = db_path
        .exists()
        .then(|| open_snapshot(db_path, false))
        .transpose()?;
    if let Some(old) = &old {
        snapshot(old, &original)?;
        let safety = safety_path(db_path, req);
        if !safety.exists() {
            safe_dir(safety.parent().ok_or(IO_ERROR)?)?;
            let ui = parse_json(File::open(dir.join(OLD_UI)).map_err(io_error)?, MAX_UI)?;
            archive(&original, &ui, None, &safety)?;
        }
    }
    // Keep the staged payload untouched so a crash before the switch can be retried.
    fs::copy(dir.join(DB), &received).map_err(io_error)?;
    let mut imported = open_snapshot(&received, false)?;
    crate::db::migrate_restored(&mut imported, &work.0)?;
    check_integrity(&imported)?;
    drop(imported);
    let candidate = if let Some(workspace) = req.manifest.workspace_id.as_deref() {
        if old.is_some() {
            let mut candidate = open_snapshot(&original, false)?;
            crate::db::migrate_restored(&mut candidate, &work.0)?;
            scope::copy_workspace(&mut candidate, &received, workspace, false)?;
            original
        } else {
            received
        }
    } else {
        received
    };
    let c = open_snapshot(&candidate, false)?;
    c.execute(
        "INSERT INTO app_settings VALUES(?1,json_quote(?2)) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",
        [ACTIVE_KEY, &req.id],
    ).map_err(error)?;
    check_integrity(&c)?;
    drop(c);
    File::options()
        .write(true)
        .open(&candidate)
        .and_then(|file| file.sync_all())
        .map_err(io_error)?;
    if let Some(old) = old {
        let busy = old
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                row.get::<_, u32>(0)
            })
            .map_err(error)?;
        if busy != 0 {
            return Err("当前数据库仍在使用，恢复未替换任何数据。".into());
        }
        drop(old);
        if db_path.with_file_name("cortexos.sqlite3-wal").exists()
            || db_path.with_file_name("cortexos.sqlite3-shm").exists()
        {
            return Err("数据库日志尚未关闭，恢复未替换任何数据。".into());
        }
    }
    move_complete(&candidate, db_path, true)
}

pub(crate) fn apply_pending(db_path: &Path) -> AppResult<()> {
    apply_with_capacity(db_path, crate::storage::disk_free_bytes)
}

pub(super) fn apply_with_capacity(
    db_path: &Path,
    free: impl FnOnce(&Path) -> Option<u64>,
) -> AppResult<()> {
    let dir = pending(db_path);
    if !dir.exists() {
        return Ok(());
    }
    let req = match read_request(&dir) {
        Ok(req) => req,
        Err(e) => {
            let _ = fs::rename(&dir, root(db_path).join(format!("failed-{}", new_id())));
            return Err(e);
        }
    };
    if db_path.exists() {
        let c = open_snapshot(db_path, false)?;
        if marker(&c)? == Some(req.id.clone()) {
            return Ok(());
        }
    }
    if let Err(reason) = prepare_candidate(db_path, &req, &dir, free) {
        // Keep a failed request for diagnosis while unblocking normal app startup.
        let _ = fs::rename(&dir, root(db_path).join(format!("failed-{}", req.id)));
        let safety = safety_path(db_path, &req);
        let _ = publish_history(
            db_path,
            &History {
                id: req.id,
                kind: "restore_failed".into(),
                at: now(),
                path: req.path,
                safety_path: safety
                    .exists()
                    .then(|| safety.to_string_lossy().into_owned()),
                workspaces: req.manifest.workspaces,
                detail: Some(reason.clone()),
            },
        );
        return Err(reason);
    }
    Ok(())
}

pub(super) fn status(db: &Database) -> AppResult<Status> {
    let dir = pending(&db.path);
    if !dir.exists() {
        return Ok(Status {
            phase: if db.restore_issue.is_some() {
                "failed"
            } else {
                "idle"
            },
            message: db.restore_issue.clone(),
            id: None,
            scope: None,
            ui: None,
        });
    }
    let req = read_request(&dir)?;
    let applied = marker(&*db.connection()?)? == Some(req.id.clone());
    let ui = if applied {
        let file = req
            .manifest
            .files
            .iter()
            .find(|item| item.name == UI)
            .ok_or(INVALID)?;
        let bytes = fs::read(dir.join(UI)).map_err(io_error)?;
        if bytes.len() as u64 != file.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != file.sha256
        {
            return Err("数据已恢复，但暂存草稿的校验未通过，恢复前的备份仍可用。".into());
        }
        Some(parse_json(&bytes[..], MAX_UI)?)
    } else {
        None
    };
    Ok(Status {
        phase: if applied { "apply_ui" } else { "pending" },
        message: None,
        id: Some(req.id),
        scope: req.manifest.workspace_id,
        ui,
    })
}

pub(super) fn acknowledge(db: &Database, id: &str) -> AppResult<()> {
    let dir = pending(&db.path);
    let req = read_request(&dir)?;
    if req.id != id || marker(&*db.connection()?)?.as_deref() != Some(id) {
        return Err("恢复记录已变化，请重新打开应用。".into());
    }
    let safety = safety_path(&db.path, &req);
    let record = root(&db.path).join("history").join(format!("{id}.json"));
    if !record.exists() {
        publish_history(
            &db.path,
            &History {
                id: req.id,
                kind: "restore".into(),
                at: now(),
                path: req.path,
                safety_path: safety
                    .exists()
                    .then(|| safety.to_string_lossy().into_owned()),
                workspaces: req.manifest.workspaces,
                detail: None,
            },
        )?;
    }
    retire(&db.path, &dir)
}

fn retire(db_path: &Path, dir: &Path) -> AppResult<()> {
    let scratch = Scratch::create(&root(db_path))?;
    fs::rename(dir, scratch.0.join("completed")).map_err(io_error)
}

#[tauri::command]
pub(crate) async fn restore_stage(
    state: State<'_, AppState>,
    backup_id: String,
    confirmed: bool,
    previous_ui: Ui,
) -> AppResult<()> {
    if !confirmed {
        return Err("请先预览并确认恢复范围。".into());
    }
    let _lease = state.claim_task(&new_id(), "application-storage", TaskKind::LocalMaintenance)?;
    let ticket = state.opened_backup.clone();
    blocking(state.database.clone()?, move |db| {
        let backup = ticket
            .lock()
            .map_err(|_| IO_ERROR)?
            .take()
            .filter(|ticket| ticket.manifest.backup_id == backup_id)
            .ok_or("请重新打开备份并确认范围。")?;
        stage(db, backup, previous_ui)
    })
    .await
}

#[tauri::command]
pub(crate) async fn restore_cancel(state: State<'_, AppState>) -> AppResult<()> {
    let _lease = state.claim_task(&new_id(), "application-storage", TaskKind::LocalMaintenance)?;
    blocking(state.database.clone()?, |db| {
        let dir = pending(&db.path);
        let req = read_request(&dir)?;
        if marker(&*db.connection()?)? == Some(req.id) {
            return Err("数据库已恢复，请重新启动完成草稿与界面偏好恢复。".into());
        }
        retire(&db.path, &dir)
    })
    .await
}

#[tauri::command]
pub(crate) async fn restore_on_start(state: State<'_, AppState>) -> AppResult<Status> {
    blocking(state.database.clone()?, status).await
}

#[tauri::command]
pub(crate) async fn restore_ack(state: State<'_, AppState>, id: String) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| acknowledge(db, &id)).await
}
