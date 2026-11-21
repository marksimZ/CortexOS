use super::{
    AppHandle, AppResult, CACHE_FOLDERS, CacheAssessment, ReclaimPreview, SCOPE, Size, State,
    TaskKind, directory_size, disk_free_bytes, expected_webview_data_dir, is_link,
    normal_directory, now, verified_webview_data_dir,
};
use crate::{
    benchmark::report_transfer::write_new,
    commands::{AppState, blocking},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
use tokio::sync::watch;
use uuid::Uuid;
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO,
        FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FileIdInfo,
        FileRenameInfo, GetFileInformationByHandleEx, SetFileInformationByHandle,
    },
    System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    },
};

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
struct DirectoryId {
    volume: u64,
    file: [u8; 16],
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
struct Browser {
    pid: u32,
    creation: u64,
}

pub(crate) struct PreviewTicket {
    pub(super) id: String,
    profile: PathBuf,
    browser: Browser,
    directories: Vec<(&'static str, DirectoryId)>,
}

impl PreviewTicket {
    pub(super) fn cache_ids(&self) -> Vec<&'static str> {
        self.directories.iter().map(|(cache, _)| *cache).collect()
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Action {
    Archive,
    Restore {
        archive: String,
        replacement: Option<DirectoryId>,
    },
}

#[derive(Deserialize, Serialize)]
struct Plan {
    cache: String,
    profile: PathBuf,
    source: DirectoryId,
    browser: Browser,
    action: Action,
    at: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    status: Status,
    detail: Option<String>,
    saved_bytes: Option<u64>,
    at: String,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    Archived,
    Restored,
    Blocked,
    Cancelled,
}

struct Entry {
    directory: PathBuf,
    id: String,
    plan: Plan,
    outcome: Option<Outcome>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Cleanup {
    bytes: u64,
    at: String,
}

const CLEANUP_PLAN: &str = "cleanup-plan.json";
const CLEANUP_RESULT: &str = "cleanup-result.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Record {
    id: String,
    label: &'static str,
    action: &'static str,
    requested_at: String,
    backup_location: String,
    outcome: Option<Outcome>,
    can_restore: bool,
    can_cleanup: bool,
    cleanup: Option<Cleanup>,
}

fn os_error(context: &str) -> String {
    format!("{context}（{}）", io::Error::last_os_error())
}

fn browser_process(pid: u32) -> AppResult<Option<(OwnedHandle, u64)>> {
    if pid == 0 {
        return Err("无法核实界面进程。".into());
    }
    // The owned read-only handle holds the process identity while Windows reports its creation time.
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if raw.is_null() {
        let err = io::Error::last_os_error();
        return if err.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(None)
        } else {
            Err(format!("无法核实界面进程是否已退出（{err}）。"))
        };
    }
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(os_error("无法核实界面进程"));
    }
    let creation = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    Ok(Some((process, creation)))
}

fn browser_creation(pid: u32) -> AppResult<Option<u64>> {
    Ok(browser_process(pid)?.map(|(_, creation)| creation))
}

fn browser_finished(browser: Browser) -> AppResult<()> {
    let Some((process, _)) =
        browser_process(browser.pid)?.filter(|(_, creation)| *creation == browser.creation)
    else {
        return Ok(());
    };
    // Wait only for the exact browser instance from the confirmed preview.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), 10_000) } {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err("上次的界面进程仍在运行。请退出所有 CortexOS 窗口后重新预约。".into()),
        _ => Err(os_error("无法确认上次界面进程已退出")),
    }
}

fn cache_info(cache: &str) -> AppResult<(&'static str, &'static str)> {
    CACHE_FOLDERS
        .iter()
        .find_map(|&(id, label, folder)| (id == cache).then_some((label, folder)))
        .ok_or_else(|| "该目录不在界面缓存清单中。".into())
}

fn cache_path(profile: &Path, cache: &str) -> AppResult<PathBuf> {
    Ok(profile
        .join("EBWebView")
        .join("Default")
        .join(cache_info(cache)?.1))
}

fn open_directory(path: &Path, access: u32) -> AppResult<File> {
    let file = OpenOptions::new()
        .access_mode(access | FILE_LIST_DIRECTORY)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|err| format!("无法打开目录 {}（{err}）。", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("无法读取目录属性（{err}）。"))?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err("目录位置异常或指向链接，已保留。".into());
    }
    Ok(file)
}

fn lock_ancestors(path: &Path) -> AppResult<Vec<File>> {
    if normal_directory(path) != Ok(true) {
        return Err("目录位置未核实，已保留。".into());
    }
    let ancestors = path.ancestors().collect::<Vec<_>>();
    ancestors
        .into_iter()
        .rev()
        // A drive root has no parent directory entry that could be renamed.
        .filter(|dir| dir.parent().is_some())
        .map(|dir| open_directory(dir, FILE_READ_ATTRIBUTES))
        .collect()
}

fn file_id(file: &File) -> AppResult<DirectoryId> {
    let mut info = FILE_ID_INFO::default();
    // Windows writes a fixed-width identifier into the live structure.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(os_error("无法核实目录身份"));
    }
    Ok(DirectoryId {
        volume: info.VolumeSerialNumber,
        file: info.FileId.Identifier,
    })
}

fn directory_id(path: &Path) -> AppResult<Option<DirectoryId>> {
    match normal_directory(path) {
        Ok(false) => Ok(None),
        Ok(true) => {
            let _parents = lock_ancestors(path.parent().ok_or("目录位置不完整。")?)?;
            file_id(&open_directory(path, FILE_READ_ATTRIBUTES)?).map(Some)
        }
        Err(()) => Err("目录位置异常、指向链接或无权读取，已保留。".into()),
    }
}

fn checked_size(path: &Path) -> AppResult<Size> {
    let (_send, receive) = watch::channel(false);
    let size = directory_size(path, false, &receive)?;
    if size.partial() {
        return Err("目录内含有链接、读取错误或超过读取上限，已保留。".into());
    }
    Ok(size)
}

// One same-volume rename, bound to the verified source handle. The destination must be absent.
fn move_verified(from: &Path, to: &Path, expected: DirectoryId) -> AppResult<u64> {
    let _from_parents = lock_ancestors(from.parent().ok_or("来源目录无效。")?)?;
    let to_parent = to.parent().ok_or("目标目录无效。")?;
    let to_parents = lock_ancestors(to_parent)?;
    let file = open_directory(from, FILE_READ_ATTRIBUTES | DELETE)?;
    if file_id(&file)? != expected {
        return Err("目录已变化，请重新预览。".into());
    }
    if file_id(to_parents.last().ok_or("目标目录无效。")?)?.volume != expected.volume {
        return Err("缓存与备份位置位于不同磁盘，已保留。".into());
    }
    if directory_id(to)?.is_some() {
        return Err("目标位置已有目录，已保留原有文件。".into());
    }
    let size = checked_size(from)?;
    if disk_free_bytes(to_parent).ok_or("未能读取目标磁盘可用空间。")? == 0 {
        return Err("目标磁盘没有可用空间。".into());
    }
    rename_by_handle(&file, to)?;
    Ok(size.logical_bytes)
}

fn rename_by_handle(file: &File, to: &Path) -> AppResult<()> {
    // FILE_RENAME_INFO has a variable-length NUL-terminated UTF-16 filename.
    #[repr(C)]
    struct Rename {
        replace: u32,
        root: HANDLE,
        bytes: u32,
        name: [u16; 32_768],
    }
    let mut rename = Box::new(Rename {
        replace: 0,
        root: std::ptr::null_mut(),
        bytes: 0,
        name: [0; 32_768],
    });
    let mut len = 0;
    for unit in to.as_os_str().encode_wide() {
        if len >= rename.name.len() - 1 {
            return Err("目标路径过长。".into());
        }
        rename.name[len] = unit;
        len += 1;
    }
    rename.bytes = (len * size_of::<u16>()) as u32;
    let length =
        (std::mem::offset_of!(Rename, name) + (len + 1) * size_of::<u16>()).max(size_of::<
            windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO,
        >());
    // The application keeps both parents and the verified directory handle open for this call.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileRenameInfo,
            (&*rename as *const Rename).cast(),
            length as u32,
        )
    } == 0
    {
        return Err(os_error("移动目录失败，目标文件未被替换"));
    }
    Ok(())
}

fn read_record<T: DeserializeOwned>(path: &Path) -> AppResult<Option<T>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("无法读取缓存操作记录（{err}）。")),
    };
    let metadata = file.metadata().map_err(|_| "无法读取缓存操作记录属性。")?;
    if !metadata.is_file() || is_link(&metadata) || metadata.len() > 16_384 {
        return Err("缓存操作记录异常，已保留。".into());
    }
    let mut bytes = Vec::new();
    file.take(16_385)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取缓存操作记录。")?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "缓存操作记录损坏，已保留。".into())
}

fn write_record<T: Serialize>(directory: &Path, name: &str, record: &T) -> AppResult<()> {
    let _locks = lock_ancestors(directory)?;
    let bytes = serde_json::to_vec(record).map_err(|_| "无法保存缓存操作记录。")?;
    if bytes.len() > 16_384 {
        return Err("缓存操作记录过大，未保存。".into());
    }
    write_new(&directory.join(name), &bytes)
}

fn valid_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
}

fn entries(root: &Path) -> AppResult<Vec<Entry>> {
    match normal_directory(root) {
        Ok(false) => return Ok(Vec::new()),
        Ok(true) => {}
        Err(()) => return Err("备份位置异常，无法读取缓存操作记录。".into()),
    }
    let _locks = lock_ancestors(root)?;
    let mut entries = Vec::new();
    for (index, entry) in fs::read_dir(root)
        .map_err(|_| "无法读取缓存备份目录。")?
        .enumerate()
    {
        if index >= super::ENTRY_LIMIT {
            return Err("缓存备份目录超出读取上限。".into());
        }
        let entry = entry.map_err(|_| "无法读取缓存备份目录。")?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if !valid_id(&id) {
            continue;
        }
        let directory = entry.path();
        let _entry_locks = lock_ancestors(&directory)?;
        if let Some(plan) = read_record(&directory.join("plan.json"))? {
            let outcome = read_record(&directory.join("result.json"))?;
            entries.push(Entry {
                directory,
                id,
                plan,
                outcome,
            });
        }
    }
    entries.sort_by(|a, b| b.plan.at.cmp(&a.plan.at).then_with(|| b.id.cmp(&a.id)));
    Ok(entries)
}

fn archive_path(root: &Path, id: &str) -> AppResult<PathBuf> {
    if !valid_id(id) {
        return Err("备份编号无效。".into());
    }
    Ok(root.join(id).join("files"))
}

fn cached_backup(entry: &Entry) -> AppResult<Option<DirectoryId>> {
    directory_id(&entry.directory.join("files"))
}

fn cleanup_cache(entry: &Entry, profile: &Path) -> AppResult<()> {
    let _parents = lock_ancestors(&entry.directory)?;
    let current = cache_path(profile, &entry.plan.cache)?;
    if directory_id(&current)?.is_none_or(|id| id == entry.plan.source)
        || checked_size(&current)?.files == 0
    {
        return Err("当前界面缓存尚未确认重建，旧备份已保留。".into());
    }
    let prior = read_record::<Cleanup>(&entry.directory.join(CLEANUP_PLAN))?;
    if read_record::<Cleanup>(&entry.directory.join(CLEANUP_RESULT))?.is_some() {
        return Ok(());
    }
    let saved = entry.directory.join("files");
    let retiring = entry.directory.join("files-purging");
    let plan = match prior {
        Some(plan) => plan,
        None => {
            if cached_backup(entry)? != Some(entry.plan.source)
                || directory_id(&retiring)?.is_some()
            {
                return Err("缓存备份已变化，请重新读取。".into());
            }
            let plan = Cleanup {
                bytes: checked_size(&saved)?.logical_bytes,
                at: now(),
            };
            write_record(&entry.directory, CLEANUP_PLAN, &plan)?;
            plan
        }
    };
    if let Some(id) = cached_backup(entry)? {
        if id != entry.plan.source || directory_id(&retiring)?.is_some() {
            return Err("缓存备份已变化，现有文件已保留。".into());
        }
        move_verified(&saved, &retiring, id)?;
    }
    if let Some(id) = directory_id(&retiring)? {
        if id != entry.plan.source {
            return Err("待清理目录身份已变化，现有文件已保留。".into());
        }
        checked_size(&retiring)?;
        // The path is the verified archive under locked application-owned parents.
        fs::remove_dir_all(&retiring).map_err(|_| "缓存未清理完，请确认没有进程占用后重试。")?;
    }
    write_record(
        &entry.directory,
        CLEANUP_RESULT,
        &Cleanup {
            bytes: plan.bytes,
            at: now(),
        },
    )
}

fn outcome(status: Status, detail: Option<String>, saved_bytes: Option<u64>) -> Outcome {
    Outcome {
        status,
        detail,
        saved_bytes,
        at: now(),
    }
}

fn archive(entry: &Entry, cache: &Path) -> AppResult<u64> {
    let backup = entry.directory.join("files");
    match cached_backup(entry)? {
        Some(id) if id == entry.plan.source => return Ok(checked_size(&backup)?.logical_bytes),
        Some(_) => return Err("备份位置已有其他目录，已保留。".into()),
        None => {}
    }
    move_verified(cache, &backup, entry.plan.source)
}

fn restore(
    root: &Path,
    entry: &Entry,
    cache: &Path,
    archive: &str,
    replacement: Option<DirectoryId>,
) -> AppResult<u64> {
    let original = archive_path(root, archive)?;
    let source = entry.plan.source;
    let current = directory_id(cache)?;
    let backup_id = directory_id(&original)?;
    if current == Some(source) && backup_id.is_none() {
        return Ok(checked_size(cache)?.logical_bytes);
    }
    if backup_id != Some(source) {
        return Err("备份身份已变化，已保留现有文件。".into());
    }
    checked_size(&original)?;
    let previous = entry.directory.join("previous-cache");
    let staged = directory_id(&previous)?;
    if let Some(id) = staged {
        if staged != replacement || current.is_some() {
            return Err("恢复期间检测到目录冲突，两份文件均已保留。".into());
        }
        checked_size(&previous)?;
        finish_restore(&original, cache, source, Some((&previous, id)))
    } else {
        if current != replacement {
            return Err("目前的缓存目录已变化，请重新确认恢复。".into());
        }
        if let Some(id) = current {
            move_verified(cache, &previous, id)?;
        }
        finish_restore(
            &original,
            cache,
            source,
            current.map(|id| (previous.as_path(), id)),
        )
    }
}

fn finish_restore(
    from: &Path,
    to: &Path,
    source: DirectoryId,
    replacement: Option<(&Path, DirectoryId)>,
) -> AppResult<u64> {
    let result = move_verified(from, to, source);
    if let Err(error) = &result {
        if let Some((previous, id)) = replacement {
            if let Err(rollback) = move_verified(previous, to, id) {
                return Err(format!(
                    "{error} 回退未能完成（{rollback}），旧备份和当前缓存仍保存在备份目录。"
                ));
            }
            return Err(format!("{error} 已放回恢复前的缓存。"));
        }
    }
    result
}

fn apply_pending(root: &Path, profile: Option<&Path>) -> AppResult<()> {
    let rows = entries(root)?;
    let mut pending = rows.iter().filter(|entry| entry.outcome.is_none());
    let Some(entry) = pending.next() else {
        return Ok(());
    };
    if pending.next().is_some() {
        return Err("有多个缓存预约，需先撤销多余预约。".into());
    }
    let execution = (|| {
        if profile != Some(entry.plan.profile.as_path()) {
            return Err("应用界面数据的位置已变化，已保留缓存。".into());
        }
        browser_finished(entry.plan.browser)?;
        let cache = cache_path(&entry.plan.profile, &entry.plan.cache)?;
        match &entry.plan.action {
            Action::Archive => archive(entry, &cache).map(|bytes| (Status::Archived, bytes)),
            Action::Restore {
                archive,
                replacement,
            } => {
                if !rows.iter().any(|backup| {
                    backup.id == *archive
                        && backup.plan.cache == entry.plan.cache
                        && backup.plan.profile == entry.plan.profile
                        && backup.plan.source == entry.plan.source
                        && matches!(backup.plan.action, Action::Archive)
                        && backup
                            .outcome
                            .as_ref()
                            .is_some_and(|result| result.status == Status::Archived)
                }) {
                    return Err("备份记录与当前预约不匹配，已保留文件。".into());
                }
                restore(root, entry, &cache, archive, *replacement)
                    .map(|bytes| (Status::Restored, bytes))
            }
        }
    })();
    let result = match execution {
        Ok((status, bytes)) => outcome(status, None, Some(bytes)),
        Err(error) => outcome(Status::Blocked, Some(error), None),
    };
    write_record(&entry.directory, "result.json", &result)
}

pub(crate) fn before_webview(app: &AppHandle, data_dir: &Path) {
    let profile = expected_webview_data_dir(app, data_dir);
    if let Err(err) = apply_pending(&data_dir.join("storage-recovery"), profile.as_deref()) {
        eprintln!("CortexOS storage recovery: {err}");
    }
}

pub(super) fn preview_ticket(
    profile: PathBuf,
    pid: u32,
    preview: &ReclaimPreview,
) -> Option<PreviewTicket> {
    preview.recovery_directory.as_ref()?;
    if preview.recovery_free_bytes? == 0 {
        return None;
    }
    let creation = browser_creation(pid).ok()??;
    let directories = preview
        .cache_items
        .iter()
        .filter(|row| row.assessment == CacheAssessment::Rebuildable)
        .filter_map(|row| {
            let path = cache_path(&profile, row.item.id).ok()?;
            directory_id(&path)
                .ok()?
                .map(|identity| (row.item.id, identity))
        })
        .collect::<Vec<_>>();
    (!directories.is_empty()).then(|| PreviewTicket {
        id: Uuid::new_v4().to_string(),
        profile,
        browser: Browser { pid, creation },
        directories,
    })
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Request {
    Archive { cache: String, approval_id: String },
    Restore { id: String },
    Cancel { id: String },
    CleanupCache { id: String, confirmed: bool },
}

fn schedule(root: &Path, plan: &Plan) -> AppResult<()> {
    let data = root.parent().ok_or("备份位置无效。")?;
    let data_locks = lock_ancestors(data)?;
    let target = if normal_directory(root) == Ok(true) {
        root
    } else {
        data
    };
    let target_dir = if target == data {
        file_id(data_locks.last().ok_or("备份位置无效。")?)?
    } else {
        directory_id(root)?.ok_or("备份位置无效。")?
    };
    if target_dir.volume != plan.source.volume {
        return Err("缓存与备份位置位于不同磁盘，已保留。".into());
    }
    if disk_free_bytes(target).ok_or("未能读取备份磁盘可用空间。")? == 0 {
        return Err("备份磁盘没有可用空间。".into());
    }
    match normal_directory(root) {
        Ok(false) => fs::create_dir(root).map_err(|_| "无法创建备份记录目录。")?,
        Ok(true) => {}
        Err(()) => return Err("备份位置异常，已保留。".into()),
    }
    let _root_guard = lock_ancestors(root)?;
    let directory = root.join(Uuid::new_v4().to_string());
    fs::create_dir(&directory).map_err(|_| "无法创建本次缓存操作记录。")?;
    write_record(&directory, "plan.json", plan)
}

fn card_records(root: &Path) -> AppResult<Vec<Record>> {
    let rows = entries(root)?;
    let mut records = Vec::new();
    for entry in rows {
        let (label, _) = cache_info(&entry.plan.cache)?;
        let cleanup_started =
            read_record::<Cleanup>(&entry.directory.join(CLEANUP_PLAN))?.is_some();
        let cleanup = read_record::<Cleanup>(&entry.directory.join(CLEANUP_RESULT))?;
        let (action, backup, can_restore) = match &entry.plan.action {
            Action::Archive => (
                "archive",
                entry.directory.join("files"),
                entry
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| outcome.status == Status::Archived)
                    && cached_backup(&entry).is_ok_and(|id| id == Some(entry.plan.source)),
            ),
            Action::Restore { .. } => ("restore", entry.directory.join("previous-cache"), false),
        };
        records.push(Record {
            id: entry.id,
            label,
            action,
            requested_at: entry.plan.at,
            backup_location: backup.to_string_lossy().into_owned(),
            outcome: entry.outcome,
            can_restore: can_restore && !cleanup_started,
            can_cleanup: cleanup.is_none() && (can_restore || cleanup_started),
            cleanup,
        });
    }
    Ok(records)
}

#[tauri::command]
pub(crate) async fn storage_recovery_history(state: State<'_, AppState>) -> AppResult<Vec<Record>> {
    blocking(state.database.clone()?, |db| {
        let data = db.path.parent().ok_or("无法定位应用的数据目录。")?;
        card_records(&data.join("storage-recovery"))
    })
    .await
}

#[tauri::command]
pub(crate) async fn storage_recovery_request(
    app: AppHandle,
    state: State<'_, AppState>,
    request: Request,
) -> AppResult<Vec<Record>> {
    let db = state.database.clone()?;
    let data = db.path.parent().ok_or("无法定位应用的数据目录。")?;
    let request_id = Uuid::new_v4().to_string();
    let _lease = state
        .storage_scans
        .claim(&request_id, SCOPE, TaskKind::LocalMaintenance)?;
    if state.has_active() || state.closing.load(Ordering::SeqCst) {
        return Err("请等待应用任务结束后再操作。".into());
    }
    let live = if matches!(request, Request::Cancel { .. }) {
        None
    } else {
        Some(
            verified_webview_data_dir(&app, data)
                .await
                .ok_or("无法核实当前界面数据位置。")?,
        )
    };
    let state = state.inner().clone();
    blocking(db, move |db| {
        if state.has_active() || state.closing.load(Ordering::SeqCst) {
            return Err("请等待应用任务结束后再操作。".into());
        }
        let root = db
            .path
            .parent()
            .ok_or("无法定位应用的数据目录。")?
            .join("storage-recovery");
        let rows = entries(&root)?;
        if let Request::Cancel { id } = request {
            let entry = rows
                .iter()
                .find(|row| row.id == id && row.outcome.is_none())
                .ok_or("该预约已结束。")?;
            let cache = cache_path(&entry.plan.profile, &entry.plan.cache)?;
            let started = match &entry.plan.action {
                Action::Archive => cached_backup(entry)?.is_some(),
                Action::Restore { archive, .. } => {
                    directory_id(&entry.directory.join("previous-cache"))?.is_some()
                        || directory_id(&archive_path(&root, archive)?)?.is_none()
                        || directory_id(&cache)? == Some(entry.plan.source)
                }
            };
            if started {
                return Err("预约已开始处理，下一次启动会核对并留下结果。".into());
            }
            write_record(
                &entry.directory,
                "result.json",
                &outcome(Status::Cancelled, None, None),
            )?;
            return card_records(&root);
        }
        if rows.iter().any(|entry| entry.outcome.is_none()) {
            return Err("已有一项预约，请先重开应用处理或撤销它。".into());
        }
        let (profile, pid) = live.ok_or("无法核实当前界面数据位置。")?;
        if let Request::CleanupCache { ref id, confirmed } = request {
            if !confirmed {
                return Err("请先确认清理所选缓存备份。".into());
            }
            let entry = rows
                .iter()
                .find(|row| {
                    row.id == *id
                        && matches!(row.plan.action, Action::Archive)
                        && row.plan.profile == profile
                        && row
                            .outcome
                            .as_ref()
                            .is_some_and(|result| result.status == Status::Archived)
                })
                .ok_or("未找到可以清理的缓存备份。")?;
            cleanup_cache(entry, &profile)?;
            return card_records(&root);
        }
        let browser = Browser {
            pid,
            creation: browser_creation(pid)?.ok_or("界面进程已变化，请重新预览。")?,
        };
        let (cache, source, action) = match request {
            Request::Archive { cache, approval_id } => {
                let approval = state.storage_preview.lock().map_err(|_| "请重新预览。")?;
                let ticket = approval
                    .as_ref()
                    .filter(|ticket| {
                        ticket.id == approval_id
                            && ticket.profile == profile
                            && ticket.browser == browser
                    })
                    .ok_or("预览已过期，请重新读取。")?;
                let source = ticket
                    .directories
                    .iter()
                    .find_map(|(id, source)| (*id == cache).then_some(*source))
                    .ok_or("此项缓存尚未通过预览核对。")?;
                let path = cache_path(&profile, &cache)?;
                if directory_id(&path)? != Some(source) || checked_size(&path)?.files == 0 {
                    return Err("缓存已经变化，请重新预览。".into());
                }
                (cache, source, Action::Archive)
            }
            Request::Restore { id } => {
                let entry = rows
                    .iter()
                    .find(|row| {
                        row.id == id
                            && matches!(row.plan.action, Action::Archive)
                            && row.plan.profile == profile
                            && row
                                .outcome
                                .as_ref()
                                .is_some_and(|result| result.status == Status::Archived)
                    })
                    .ok_or("未找到可以恢复的备份。")?;
                if read_record::<Cleanup>(&entry.directory.join(CLEANUP_PLAN))?.is_some() {
                    return Err("此缓存备份已安排清理。".into());
                }
                if cached_backup(entry)? != Some(entry.plan.source) {
                    return Err("备份目录已变化或已恢复。".into());
                }
                checked_size(&entry.directory.join("files"))?;
                let path = cache_path(&profile, &entry.plan.cache)?;
                let replacement = directory_id(&path)?;
                if replacement.is_some() {
                    checked_size(&path)?;
                }
                (
                    entry.plan.cache.clone(),
                    entry.plan.source,
                    Action::Restore {
                        archive: id,
                        replacement,
                    },
                )
            }
            Request::Cancel { .. } | Request::CleanupCache { .. } => unreachable!(),
        };
        schedule(
            &root,
            &Plan {
                cache,
                profile,
                source,
                browser,
                action,
                at: now(),
            },
        )?;
        if let Ok(mut ticket) = state.storage_preview.lock() {
            *ticket = None;
        }
        card_records(&root)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_replays_interruption_and_rolls_back_a_locked_backup() {
        let folder = std::env::temp_dir().join(format!("cortexos-recovery-{}", Uuid::new_v4()));
        let profile = folder.join("profile");
        let root = folder.join("data").join("storage-recovery");
        let cache = cache_path(&profile, "code-cache").unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(root.parent().unwrap()).unwrap();
        fs::write(cache.join("original"), b"original").unwrap();
        let source = directory_id(&cache).unwrap().unwrap();
        let pid = std::process::id();
        let retired = Browser {
            pid,
            creation: browser_creation(pid).unwrap().unwrap() ^ 1,
        };
        let plan = |action| Plan {
            cache: "code-cache".into(),
            profile: profile.clone(),
            source,
            browser: retired,
            action,
            at: now(),
        };
        let pending = || {
            entries(&root)
                .unwrap()
                .into_iter()
                .find(|row| row.outcome.is_none())
                .unwrap()
        };
        schedule(&root, &plan(Action::Archive)).unwrap();
        let entry = pending();
        let original = archive_path(&root, &entry.id).unwrap();
        assert_eq!(move_verified(&cache, &original, source).unwrap(), 8);
        apply_pending(&root, Some(&profile)).unwrap();
        apply_pending(&root, Some(&profile)).unwrap();
        assert!(entries(&root).unwrap().iter().any(|row| {
            row.id == entry.id
                && row
                    .outcome
                    .as_ref()
                    .is_some_and(|result| result.status == Status::Archived)
        }));

        fs::create_dir(&cache).unwrap();
        fs::write(cache.join("regenerated"), b"regenerated").unwrap();
        let current = directory_id(&cache).unwrap().unwrap();
        let restoration = || {
            plan(Action::Restore {
                archive: entry.id.clone(),
                replacement: Some(current),
            })
        };
        schedule(&root, &restoration()).unwrap();
        let denied = pending();
        let held = open_directory(&original, FILE_READ_ATTRIBUTES).unwrap();
        apply_pending(&root, Some(&profile)).unwrap();
        let failure = read_record::<Outcome>(&denied.directory.join("result.json"))
            .unwrap()
            .unwrap();
        assert!(failure.status == Status::Blocked);
        assert!(failure.detail.unwrap().contains("已放回恢复前的缓存"));
        assert_eq!(fs::read(cache.join("regenerated")).unwrap(), b"regenerated");
        assert_eq!(fs::read(original.join("original")).unwrap(), b"original");
        drop(held);

        schedule(&root, &restoration()).unwrap();
        let replay = pending();
        let previous = replay.directory.join("previous-cache");
        assert_eq!(move_verified(&cache, &previous, current).unwrap(), 11);
        apply_pending(&root, Some(&profile)).unwrap();
        apply_pending(&root, Some(&profile)).unwrap();
        assert!(entries(&root).unwrap().iter().any(|row| {
            row.id == replay.id
                && row
                    .outcome
                    .as_ref()
                    .is_some_and(|result| result.status == Status::Restored)
        }));
        assert_eq!(fs::read(cache.join("original")).unwrap(), b"original");
        assert_eq!(
            fs::read(previous.join("regenerated")).unwrap(),
            b"regenerated"
        );
        assert!(!original.exists());
    }
}
