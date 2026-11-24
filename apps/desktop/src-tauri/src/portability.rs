use crate::{
    commands::{AppState, blocking},
    db::{Database, backup_schema_version, check_backup_schema, error, json_error},
    models::{AppResult, new_id, now},
    tasks::TaskKind,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub(crate) mod knowledge_export;
pub(crate) mod restore;
mod scope;
#[cfg(test)]
mod tests;

pub(crate) use restore::apply_pending;

const DB: &str = "cortexos.sqlite3";
const UI: &str = "ui.json";
const MANIFEST: &str = "manifest.json";
const MAX_UI: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST: u64 = 512 * 1024;
const INVALID: &str = "备份损坏、内容不完整，或不是受支持的 CortexOS 备份。";
const IO_ERROR: &str = "读写备份失败，请检查路径、可用空间和权限。";
const SPACE_ERROR: &str = "磁盘可用空间不足，未替换当前数据。";
const DEPENDENCIES: &[&str] = &[
    "云端密钥由 Windows 保存；换电脑后需重新绑定。",
    "模型权重和 Docker 镜像需在目标电脑单独准备。",
    "工作区的文档文本和版本来自应用保存的副本，原始文件由用户保管。",
    "未提交的 Docker 工作副本请先在原环境保存。",
];

type Ui = BTreeMap<String, String>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Workspace {
    id: String,
    name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileEntry {
    name: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Manifest {
    format_version: u32,
    backup_id: String,
    app_version: String,
    schema_version: usize,
    created_at: String,
    workspace_id: Option<String>,
    workspaces: Vec<Workspace>,
    files: Vec<FileEntry>,
    dependencies: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preview {
    workspaces: Vec<Workspace>,
    estimated_database_bytes: u64,
    available_bytes: Option<u64>,
    includes: Vec<&'static str>,
    dependencies: &'static [&'static str],
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct History {
    id: String,
    kind: String,
    at: String,
    path: String,
    safety_path: Option<String>,
    workspaces: Vec<Workspace>,
    detail: Option<String>,
}

pub(crate) struct OpenedBackup {
    path: PathBuf,
    manifest: Manifest,
}

struct Scratch(PathBuf);

impl Scratch {
    fn create(root: &Path) -> AppResult<Self> {
        safe_dir(root)?;
        let path = root.join(format!("work-{}", new_id()));
        fs::create_dir(&path).map_err(io_error)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn safe_dir(dir: &Path) -> AppResult<()> {
    // Refuse linked directories for application-owned journals and scratch data.
    if dir.exists()
        && !fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir() && !crate::storage::is_link(&m))
    {
        return Err("备份工作目录不是普通目录。".into());
    }
    fs::create_dir_all(dir).map_err(io_error)
}

fn root(path: &Path) -> PathBuf {
    path.with_file_name("data-portability")
}

fn io_error(_: io::Error) -> String {
    IO_ERROR.into()
}

fn invalid(_: impl std::fmt::Display) -> String {
    INVALID.into()
}

fn space_check(dir: &Path, needed: u64) -> AppResult<()> {
    available_space(crate::storage::disk_free_bytes(dir), needed)
}

fn available_space(available: Option<u64>, needed: u64) -> AppResult<()> {
    if available.is_some_and(|free| free < needed) {
        Err(SPACE_ERROR.into())
    } else {
        Ok(())
    }
}

fn new_file(path: &Path) -> AppResult<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error)
}

fn move_complete(from: &Path, to: &Path, replace: bool) -> AppResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let wide = |path: &Path| {
            path.as_os_str()
                .encode_wide()
                .chain([0])
                .collect::<Vec<_>>()
        };
        let flags = MOVEFILE_WRITE_THROUGH
            | if replace {
                MOVEFILE_REPLACE_EXISTING
            } else {
                0
            };
        // The staging file and destination share a volume. No copy fallback is enabled.
        if unsafe { MoveFileExW(wide(from).as_ptr(), wide(to).as_ptr(), flags) } == 0 {
            return Err("无法安全替换或保存文件，请检查占用、权限和目标文件是否已存在。".into());
        }
    }
    #[cfg(not(windows))]
    if replace {
        fs::rename(from, to).map_err(io_error)?;
    } else {
        fs::hard_link(from, to).map_err(io_error)?;
        let _ = fs::remove_file(from);
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let mut file = new_file(path)?;
    serde_json::to_writer(&mut file, value).map_err(json_error)?;
    file.sync_all().map_err(io_error)
}

fn parse_json<T: DeserializeOwned>(reader: impl Read, limit: u64) -> AppResult<T> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 > limit {
        return Err(INVALID.into());
    }
    serde_json::from_slice(&bytes).map_err(invalid)
}

fn workspaces(c: &Connection, id: Option<&str>) -> AppResult<Vec<Workspace>> {
    let rows = c
        .prepare("SELECT id,name FROM workspaces WHERE ?1 IS NULL OR id=?1 ORDER BY id")
        .map_err(error)?
        .query_map([id], |r| {
            Ok(Workspace {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    if id.is_some() && rows.len() != 1 {
        return Err("工作区不存在。".into());
    }
    Ok(rows)
}

fn decode_key(key: &str) -> Option<String> {
    let mut src = key.bytes();
    let mut out = Vec::with_capacity(key.len());
    while let Some(b) = src.next() {
        out.push(if b == b'%' {
            let high = (src.next()? as char).to_digit(16)?;
            let low = (src.next()? as char).to_digit(16)?;
            ((high << 4) | low) as u8
        } else {
            b
        });
    }
    String::from_utf8(out).ok()
}

fn ui_key_allowed(key: &str, id: Option<&str>) -> bool {
    if matches!(
        key,
        "cortexos.appearance"
            | "cortexos.workspace"
            | "cortexos.evidence-open"
            | "cortexos.graph-motion"
    ) {
        return id.is_none();
    }
    if let Some(owner) = key.strip_prefix("cortexos.conversation.") {
        return !owner.is_empty() && id.is_none_or(|id| owner == id);
    }
    let Some((namespace, encoded)) = key
        .strip_prefix("cortexos.draft.v1.")
        .and_then(|key| key.split_once('.'))
    else {
        return false;
    };
    let Some(key) = decode_key(encoded) else {
        return false;
    };
    let matches_owner = |owner: &str| !owner.is_empty() && id.is_none_or(|id| id == owner);
    match namespace {
        "knowledge" | "relation" => serde_json::from_str::<Vec<serde_json::Value>>(&key)
            .ok()
            .and_then(|parts| parts.first()?.as_str().map(matches_owner))
            .unwrap_or(false),
        "assistant" | "harness-analysis" | "standard-benchmark" => matches_owner(&key),
        "chat" | "dataset-review" | "benchmark-profile" | "programming" | "task-review"
        | "workspace-handoff" => key
            .split_once(':')
            .is_some_and(|(owner, _)| matches_owner(owner)),
        _ => false,
    }
}

fn check_ui_size(ui: &Ui) -> AppResult<()> {
    if serde_json::to_vec(ui).map_err(json_error)?.len() as u64 > MAX_UI {
        return Err("草稿与界面偏好超出备份允许的大小，请先保存体积较大的草稿。".into());
    }
    Ok(())
}

fn open_snapshot(path: &Path, read_only: bool) -> AppResult<Connection> {
    let mode = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let c = Connection::open_with_flags(path, mode | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        .map_err(invalid)?;
    c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON")
        .map_err(invalid)?;
    Ok(c)
}

fn check_integrity(c: &Connection) -> AppResult<()> {
    let quick = c
        .query_row("PRAGMA integrity_check(1)", [], |r| r.get::<_, String>(0))
        .map_err(invalid)?;
    let foreign = c
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()
        .map_err(invalid)?;
    if quick != "ok" || foreign.is_some() {
        return Err("数据库完整性或关联校验未通过。".into());
    }
    Ok(())
}

fn snapshot(c: &Connection, path: &Path) -> AppResult<()> {
    c.execute("VACUUM INTO ?1", [path.to_string_lossy()])
        .map_err(|_| "数据库快照失败，请检查可用空间与权限。".to_string())?;
    File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(io_error)
}

fn hash_stream(mut input: impl Read, mut output: impl Write) -> AppResult<(u64, String)> {
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buf = [0; 64 * 1024];
    loop {
        let n = input.read(&mut buf).map_err(invalid)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        output.write_all(&buf[..n]).map_err(io_error)?;
        bytes += n as u64;
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}

fn archive(
    snapshot_path: &Path,
    ui: &Ui,
    id: Option<String>,
    target: &Path,
) -> AppResult<Manifest> {
    let parent = target
        .parent()
        .filter(|dir| dir.is_dir())
        .ok_or("备份保存目录不存在。")?;
    if target.exists() {
        return Err("文件已存在，请另选文件名。".into());
    }
    let c = open_snapshot(snapshot_path, true)?;
    let mut manifest = Manifest {
        format_version: 1,
        backup_id: new_id(),
        app_version: crate::app_version().into(),
        schema_version: backup_schema_version(&c)?,
        created_at: now(),
        workspaces: workspaces(&c, id.as_deref())?,
        workspace_id: id,
        files: Vec::new(),
        dependencies: DEPENDENCIES.iter().map(|v| v.to_string()).collect(),
    };
    drop(c);
    check_ui_size(ui)?;
    let ui = serde_json::to_vec(ui).map_err(json_error)?;
    let db_size = fs::metadata(snapshot_path).map_err(io_error)?.len();
    space_check(
        parent,
        db_size
            .saturating_add(ui.len() as u64)
            .saturating_add(1024 * 1024),
    )?;
    let temp = parent.join(format!(".cortexos-backup-{}.tmp", manifest.backup_id));
    let file = new_file(&temp)?;
    let result = (|| {
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        for (name, input) in [(DB, None), (UI, Some(ui.as_slice()))] {
            zip.start_file(name, options).map_err(invalid)?;
            let (bytes, sha256) = match input {
                Some(bytes) => hash_stream(bytes, &mut zip)?,
                None => hash_stream(File::open(snapshot_path).map_err(io_error)?, &mut zip)?,
            };
            manifest.files.push(FileEntry {
                name: name.into(),
                bytes,
                sha256,
            });
        }
        let metadata = serde_json::to_vec_pretty(&manifest).map_err(json_error)?;
        if metadata.len() as u64 > MAX_MANIFEST {
            return Err(INVALID.into());
        }
        zip.start_file(MANIFEST, options).map_err(invalid)?;
        zip.write_all(&metadata).map_err(io_error)?;
        zip.finish()
            .map_err(invalid)?
            .sync_all()
            .map_err(io_error)?;
        move_complete(&temp, target, false)?;
        Ok(manifest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn validate_manifest(manifest: &Manifest) -> AppResult<()> {
    if manifest.format_version != 1 {
        return Err("备份格式版本不受支持，请使用兼容的 CortexOS。".into());
    }
    if uuid::Uuid::parse_str(&manifest.backup_id).is_err()
        || manifest.files.len() != 2
        || (manifest.workspace_id.is_some() && manifest.workspaces.len() != 1)
        || manifest.files.iter().any(|entry| {
            entry.sha256.len() != 64 || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn unpack(path: &Path, destination: &Path) -> AppResult<(Manifest, Ui)> {
    let mut zip = ZipArchive::new(File::open(path).map_err(io_error)?).map_err(invalid)?;
    if zip.len() != 3 || zip.file_names().count() != 3 {
        return Err(INVALID.into());
    }
    let manifest: Manifest = parse_json(zip.by_name(MANIFEST).map_err(invalid)?, MAX_MANIFEST)?;
    validate_manifest(&manifest)?;
    for (name, limit) in [(DB, u64::MAX - 1), (UI, MAX_UI)] {
        let file = manifest
            .files
            .iter()
            .find(|f| f.name == name)
            .ok_or(INVALID)?;
        let zip_file = zip.by_name(name).map_err(invalid)?;
        if zip_file.is_dir() || zip_file.size() != file.bytes || file.bytes > limit {
            return Err(INVALID.into());
        }
        space_check(destination, file.bytes.saturating_add(1024 * 1024))?;
        let mut out = new_file(&destination.join(name))?;
        let (bytes, hash) = hash_stream(zip_file.take(file.bytes + 1), &mut out)?;
        if bytes != file.bytes || hash != file.sha256 {
            return Err("备份文件的 SHA-256 校验未通过。".into());
        }
        out.sync_all().map_err(io_error)?;
    }
    let ui = validate_extracted(&manifest, destination)?;
    Ok((manifest, ui))
}

fn validate_extracted(manifest: &Manifest, dir: &Path) -> AppResult<Ui> {
    let c = open_snapshot(&dir.join(DB), true)?;
    check_backup_schema(&c, manifest.schema_version)?;
    check_integrity(&c)?;
    if workspaces(&c, None)? != manifest.workspaces {
        return Err("备份内的工作区与清单不一致。".into());
    }
    if let Some(id) = manifest.workspace_id.as_deref() {
        workspaces(&c, Some(id))?;
        scope::verify(&c, id)?;
    }
    let ui: Ui = parse_json(File::open(dir.join(UI)).map_err(io_error)?, MAX_UI)?;
    if ui
        .keys()
        .any(|key| !ui_key_allowed(key, manifest.workspace_id.as_deref()))
    {
        return Err("备份包含未支持的界面数据。".into());
    }
    Ok(ui)
}

fn publish_history(db_path: &Path, entry: &History) -> AppResult<()> {
    let root = root(db_path);
    let history = root.join("history");
    safe_dir(&history)?;
    let work = Scratch::create(&root)?;
    let path = work.0.join("entry.json");
    write_json(&path, entry)?;
    move_complete(&path, &history.join(format!("{}.json", entry.id)), false)
}

fn save(db: &Database, workspace: Option<String>, mut ui: Ui, path: &Path) -> AppResult<Manifest> {
    let work = Scratch::create(&root(&db.path))?;
    let scratch = work.0.join(DB);
    {
        let c = db.connection()?;
        snapshot(&c, &scratch)?;
    }
    let c = open_snapshot(&scratch, true)?;
    check_backup_schema(&c, backup_schema_version(&c)?)?;
    ui.retain(|key, _| ui_key_allowed(key, workspace.as_deref()));
    let payload = if let Some(id) = workspace.as_deref() {
        workspaces(&c, Some(id))?;
        let path = work.0.join("workspace.sqlite3");
        new_file(&path)?;
        let mut target = open_snapshot(&path, false)?;
        crate::db::migrate_restored(&mut target, &work.0)?;
        scope::copy_workspace(&mut target, &scratch, id, true)?;
        scope::verify(&target, id)?;
        path
    } else {
        check_integrity(&c)?;
        scratch
    };
    drop(c);
    let manifest = archive(&payload, &ui, workspace, path)?;
    if publish_history(
        &db.path,
        &History {
            id: manifest.backup_id.clone(),
            kind: "backup".into(),
            at: manifest.created_at.clone(),
            path: path.to_string_lossy().into_owned(),
            safety_path: None,
            workspaces: manifest.workspaces.clone(),
            detail: None,
        },
    )
    .is_err()
    {
        return Err("备份文件已保存，应用未能写入备份历史。可直接从保存位置打开文件校验。".into());
    }
    Ok(manifest)
}

#[tauri::command]
pub(crate) async fn backup_preview(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> AppResult<Preview> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        let ids = workspaces(&c, workspace_id.as_deref())?;
        let page_count: i64 = c
            .pragma_query_value(None, "page_count", |r| r.get(0))
            .map_err(error)?;
        let page_size: i64 = c
            .pragma_query_value(None, "page_size", |r| r.get(0))
            .map_err(error)?;
        Ok(Preview {
            workspaces: ids,
            estimated_database_bytes: u64::try_from(page_size.saturating_mul(page_count))
                .map_err(|_| INVALID)?,
            available_bytes: db.path.parent().and_then(crate::storage::disk_free_bytes),
            includes: vec![
                "会话与知识版本",
                "来源副本、关系和索引",
                "评测与项目记录",
                "模型配置与草稿",
            ],
            dependencies: DEPENDENCIES,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn backup_save(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace_id: Option<String>,
    ui: Ui,
) -> AppResult<Option<Manifest>> {
    check_ui_size(&ui)?;
    let _lease = state.claim_task(
        &new_id(),
        workspace_id.as_deref().unwrap_or("application-storage"),
        TaskKind::LocalMaintenance,
    )?;
    blocking(state.database.clone()?, move |db| {
        let path = app
            .dialog()
            .file()
            .set_title("保存 CortexOS 备份")
            .add_filter("CortexOS 备份", &["cortexbackup"])
            .set_file_name(format!("CortexOS-备份-{}.cortexbackup", new_id()))
            .blocking_save_file();
        let Some(path) = path else { return Ok(None) };
        let path = path.into_path().map_err(|_| "请选择本机文件位置。")?;
        save(db, workspace_id, ui, &path).map(Some)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenPreview {
    manifest: Manifest,
    path: String,
    conflicts: Vec<Workspace>,
    available_bytes: Option<u64>,
    pending: bool,
}

#[tauri::command]
pub(crate) async fn backup_open(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<OpenPreview>> {
    let _lease = state.claim_task(&new_id(), "application-storage", TaskKind::LocalMaintenance)?;
    let ticket = state.opened_backup.clone();
    blocking(state.database.clone()?, move |db| {
        *ticket.lock().map_err(|_| IO_ERROR)? = None;
        let path = app
            .dialog()
            .file()
            .set_title("打开 CortexOS 备份")
            .add_filter("CortexOS 备份", &["cortexbackup"])
            .blocking_pick_file();
        let Some(path) = path else { return Ok(None) };
        let path = path.into_path().map_err(|_| "请选择本机文件位置。")?;
        let work = Scratch::create(&root(&db.path))?;
        let (manifest, _) = unpack(&path, &work.0)?;
        let c = db.connection()?;
        let conflicts = workspaces(&c, None)?
            .into_iter()
            .filter(|w| manifest.workspaces.iter().any(|saved| w.id == saved.id))
            .collect();
        let preview = OpenPreview {
            manifest: manifest.clone(),
            path: path.to_string_lossy().into_owned(),
            conflicts,
            available_bytes: db.path.parent().and_then(crate::storage::disk_free_bytes),
            pending: restore::pending(&db.path).exists(),
        };
        *ticket.lock().map_err(|_| IO_ERROR)? = Some(OpenedBackup { path, manifest });
        Ok(Some(preview))
    })
    .await
}

#[tauri::command]
pub(crate) async fn backup_history(state: State<'_, AppState>) -> AppResult<Vec<History>> {
    blocking(state.database.clone()?, |db| {
        let dir = root(&db.path).join("history");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files = fs::read_dir(dir)
            .map_err(io_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error)?;
        files.sort_unstable_by_key(|file| file.metadata().and_then(|m| m.modified()).ok());
        files
            .into_iter()
            .rev()
            .filter(|file| file.path().extension().is_some_and(|ext| ext == "json"))
            .take(100)
            .map(|file| parse_json(File::open(file.path()).map_err(io_error)?, MAX_MANIFEST))
            .collect()
    })
    .await
}
