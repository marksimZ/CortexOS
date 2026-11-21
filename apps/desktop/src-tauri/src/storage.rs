use crate::{
    commands::{AppState, blocking},
    db::error,
    models::{AppResult, now},
    tasks::TaskKind,
};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use tauri::{AppHandle, Manager, State};
use tokio::sync::watch;

#[cfg(windows)]
pub(crate) mod recovery;

const SCOPE: &str = "application-storage";
const STOPPED: &str = "已停止读取存储清单。";
const ENTRY_LIMIT: usize = 20_000;
const DEPTH_LIMIT: usize = 24;
const CACHE_FOLDERS: [(&str, &str, &str); 3] = [
    ("web-cache", "界面资源缓存", "Cache"),
    ("code-cache", "界面代码缓存", "Code Cache"),
    ("gpu-cache", "界面绘图缓存", "GPUCache"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reference {
    label: &'static str,
    count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Size {
    logical_bytes: u64,
    allocated_bytes: Option<u64>,
    files: u64,
    skipped_entries: u64,
    read_errors: u64,
    limit_reached: bool,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            logical_bytes: 0,
            allocated_bytes: Some(0),
            files: 0,
            skipped_entries: 0,
            read_errors: 0,
            limit_reached: false,
        }
    }
}

impl Size {
    fn add_file(&mut self, path: &Path, metadata: &fs::Metadata) {
        self.files += 1;
        self.logical_bytes = self.logical_bytes.saturating_add(metadata.len());
        self.allocated_bytes = self
            .allocated_bytes
            .and_then(|sum| allocated_size(path).map(|size| sum.saturating_add(size)));
    }

    fn partial(&self) -> bool {
        self.skipped_entries > 0 || self.read_errors > 0 || self.limit_reached
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    id: &'static str,
    label: &'static str,
    owner: &'static str,
    location: Option<String>,
    size: Option<Size>,
    payload_bytes: Option<u64>,
    references: Option<Vec<Reference>>,
    reason: &'static str,
}

impl Item {
    fn new(
        id: &'static str,
        label: &'static str,
        owner: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            id,
            label,
            owner,
            location: None,
            size: None,
            payload_bytes: None,
            references: None,
            reason,
        }
    }

    fn at(mut self, path: &Path, size: Option<Size>) -> Self {
        self.location = Some(path.to_string_lossy().into_owned());
        self.size = size;
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Inventory {
    sampled_at: String,
    file_bytes: u64,
    partial_file_total: bool,
    active_task_observed: bool,
    reference_error: Option<String>,
    items: Vec<Item>,
    reclaim_preview: Option<ReclaimPreview>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReclaimPreview {
    recovery_directory: Option<String>,
    recovery_free_bytes: Option<u64>,
    cache_items: Vec<CacheItem>,
    approval_id: Option<String>,
    approval_caches: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheItem {
    item: Item,
    assessment: CacheAssessment,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum CacheAssessment {
    Rebuildable,
    Empty,
    Unverified,
}

fn check_cancel(cancel: &watch::Receiver<bool>) -> AppResult<()> {
    if *cancel.borrow() {
        Err(STOPPED.into())
    } else {
        Ok(())
    }
}

pub(crate) fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn allocated_size(path: &Path) -> Option<u64> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
    };
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .ok()?;
    if is_link(&file.metadata().ok()?) {
        return None;
    }
    let mut info = FILE_STANDARD_INFO::default();
    // The metadata-only handle and output buffer remain live for the call.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileStandardInfo,
            (&mut info as *mut FILE_STANDARD_INFO).cast(),
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    };
    (ok != 0)
        .then(|| u64::try_from(info.AllocationSize).ok())
        .flatten()
}

#[cfg(not(windows))]
fn allocated_size(_: &Path) -> Option<u64> {
    None
}

fn regular_file(path: &Path) -> Size {
    let mut size = Size::default();
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !is_link(&meta) => size.add_file(path, &meta),
        Ok(_) => size.skipped_entries += 1,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => size.read_errors += 1,
    }
    size
}

fn is_migration_backup(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_prefix("before-migration-")?
                .strip_suffix(".sqlite3")?
                .split_once('-')
        })
        .is_some_and(|(version, id)| {
            version.parse::<u32>().is_ok_and(|v| v > 1) && uuid::Uuid::parse_str(id).is_ok()
        })
}

fn directory_size(
    root: &Path,
    backups_only: bool,
    cancel: &watch::Receiver<bool>,
) -> AppResult<Size> {
    let mut size = Size::default();
    let mut dirs = vec![(root.to_owned(), 0)];
    let mut scanned = 0;
    while let Some((dir, depth)) = dirs.pop() {
        check_cancel(cancel)?;
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() && !is_link(&meta) => {}
            Ok(_) => {
                size.skipped_entries += 1;
                continue;
            }
            Err(e) if depth == 0 && e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                size.read_errors += 1;
                continue;
            }
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            size.read_errors += 1;
            continue;
        };
        for entry in entries {
            check_cancel(cancel)?;
            scanned += 1;
            if scanned > ENTRY_LIMIT {
                size.limit_reached = true;
                return Ok(size);
            }
            let Ok(entry) = entry else {
                size.read_errors += 1;
                continue;
            };
            let path = entry.path();
            if backups_only && !is_migration_backup(&path) {
                continue;
            }
            match fs::symlink_metadata(&path) {
                Ok(meta) if is_link(&meta) => size.skipped_entries += 1,
                Ok(meta) if meta.is_file() => size.add_file(&path, &meta),
                Ok(meta) if meta.is_dir() && !backups_only => {
                    if depth < DEPTH_LIMIT {
                        dirs.push((path, depth + 1));
                    } else {
                        size.limit_reached = true;
                    }
                }
                Ok(_) => size.skipped_entries += 1,
                Err(_) => size.read_errors += 1,
            }
        }
    }
    Ok(size)
}

// Check every component before measuring a cache or a planned recovery folder.
fn normal_directory(path: &Path) -> Result<bool, ()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(());
    }
    #[cfg(windows)]
    if !matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(
            prefix.kind(),
            std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
        )
    ) {
        return Err(());
    }
    let ancestors: Vec<_> = path.ancestors().collect();
    for directory in ancestors.into_iter().rev() {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            _ => return Err(()),
        }
    }
    Ok(true)
}

#[cfg(windows)]
pub(crate) fn disk_free_bytes(directory: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let directory: Vec<_> = directory.as_os_str().encode_wide().chain([0]).collect();
    let mut available = 0;
    // The terminated path and output buffer remain live for the call.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            directory.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(not(windows))]
pub(crate) fn disk_free_bytes(_: &Path) -> Option<u64> {
    None
}

#[cfg(windows)]
fn expected_webview_data_dir(app: &AppHandle, data_dir: &Path) -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if std::env::var_os("CORTEXOS_TEST_DATA_DIR").as_deref() == Some(data_dir.as_os_str())
        && data_dir.is_absolute()
    {
        let fixture = data_dir.join("storage-test-webview");
        if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").as_deref() == Some(fixture.as_os_str()) {
            return Some(fixture);
        }
    }
    let _ = data_dir;
    let expected = app.path().app_local_data_dir().ok()?;
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER")
        .is_some_and(|value| value != expected.as_os_str())
    {
        return None;
    }
    Some(expected)
}

#[cfg(windows)]
async fn verified_webview_data_dir(app: &AppHandle, data_dir: &Path) -> Option<(PathBuf, u32)> {
    use webview2_com::{
        CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::ICoreWebView2Environment7,
    };
    use windows_core::{Interface, PWSTR};

    let expected = expected_webview_data_dir(app, data_dir)?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.get_webview_window("main")?
        .with_webview(move |webview| {
            let actual = (|| {
                let environment = webview
                    .environment()
                    .cast::<ICoreWebView2Environment7>()
                    .ok()?;
                let mut folder = PWSTR::null();
                // Read the running browser's directory on its UI thread and own the COM string.
                let result = unsafe { environment.UserDataFolder(&mut folder) };
                let folder = CoTaskMemPWSTR::from(folder);
                result.ok()?;
                let controller = webview.controller();
                // Read the browser PID from the same live WebView as the profile path.
                let view = unsafe { controller.CoreWebView2() }.ok()?;
                let mut pid = 0;
                unsafe { view.BrowserProcessId(&mut pid) }.ok()?;
                Some((PathBuf::from(folder.to_string()), pid))
            })();
            let _ = send.send(actual);
        })
        .ok()?;
    let (actual, pid) = tokio::time::timeout(std::time::Duration::from_secs(2), receive)
        .await
        .ok()?
        .ok()??;
    // Custom locations, including policy overrides, have ownership outside this check.
    (actual == expected && pid != 0).then_some((expected, pid))
}

#[cfg(not(windows))]
async fn verified_webview_data_dir(_: &AppHandle, _: &Path) -> Option<(PathBuf, u32)> {
    None
}

fn reclaim_preview(
    data_dir: &Path,
    webview_data_dir: Option<&Path>,
    cancel: &watch::Receiver<bool>,
) -> AppResult<ReclaimPreview> {
    let recovery = data_dir.join("storage-recovery");
    let recovery_volume = match normal_directory(&recovery) {
        Ok(true) => Some(recovery.as_path()),
        Ok(false) if normal_directory(data_dir) == Ok(true) => Some(data_dir),
        _ => None,
    };
    let mut cache_items = Vec::new();
    let cache_root = webview_data_dir.map(|path| path.join("EBWebView").join("Default"));
    for (id, label, folder) in CACHE_FOLDERS {
        check_cancel(cancel)?;
        let path = cache_root.as_ref().map(|root| root.join(folder));
        let mut item = Item::new(
            id,
            label,
            "WebView2",
            "无法确认应用使用的是默认界面数据位置。",
        );
        let mut assessment = CacheAssessment::Unverified;
        if let Some(path) = path.as_deref() {
            item = item.at(path, None);
            match normal_directory(path) {
                Ok(exists) => {
                    let size = if exists {
                        directory_size(path, false, cancel)?
                    } else {
                        Size::default()
                    };
                    (assessment, item.reason) = if size.partial() {
                        (
                            CacheAssessment::Unverified,
                            "目录含链接、读取错误或超出读取上限，暂时保护。",
                        )
                    } else if size.files == 0 {
                        (CacheAssessment::Empty, "当前没有测到缓存文件。")
                    } else {
                        (
                            CacheAssessment::Rebuildable,
                            "界面目前正在使用 WebView2。缓存由 WebView2 自动重建，需退出应用后复查。",
                        )
                    };
                    item.size = Some(size);
                }
                Err(()) => item.reason = "目录含链接、无权读取或位置异常，暂时保护。",
            }
        }
        cache_items.push(CacheItem { item, assessment });
    }
    check_cancel(cancel)?;
    Ok(ReclaimPreview {
        recovery_directory: recovery_volume.map(|_| recovery.to_string_lossy().into_owned()),
        recovery_free_bytes: recovery_volume.and_then(disk_free_bytes),
        cache_items,
        approval_id: None,
        approval_caches: Vec::new(),
    })
}

fn count(c: &Connection, sql: &str) -> AppResult<u64> {
    let count = c
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(error)?;
    u64::try_from(count).map_err(|_| "存储计数无效。".into())
}

fn references(c: &Connection, queries: &[(&'static str, &str)]) -> AppResult<Vec<Reference>> {
    queries
        .iter()
        .map(|(label, sql)| {
            Ok(Reference {
                label,
                count: count(c, sql)?,
            })
        })
        .collect()
}

fn evidence_groups() -> Vec<Item> {
    vec![
        Item::new(
            "sources",
            "已保存的资料副本",
            "工作区",
            "知识引用和历史资料依赖这些版本。",
        ),
        Item::new(
            "history",
            "知识版本与历史记录",
            "工作区",
            "保留对话、处理记录、评测结果和已固定的知识版本。",
        ),
        Item::new(
            "indexes",
            "查找索引",
            "工作区",
            "索引与模型、版本和授权绑定。回收前需核对引用和重建条件。",
        ),
        Item::new(
            "knowledge-content",
            "知识正文与版本",
            "工作区",
            "正文和版本保存在数据库中，可用于对照与恢复。",
        ),
        Item::new(
            "benchmark-results",
            "评测历史与结果",
            "工作区",
            "统计评测配置、评分明细与资源数据的库内字节。",
        ),
        Item::new(
            "harness",
            "项目任务记录与已保存副本",
            "工作区",
            "统计已保存的项目资料、草案与计划。当前容器里的未提交副本由 Docker 管理。",
        ),
        Item::new(
            "external-models",
            "Ollama 模型",
            "Ollama",
            "由 Ollama 管理。模型大小和共享情况需从运行环境核实。",
        ),
        Item::new(
            "external-docker",
            "Docker 镜像与容器",
            "Docker",
            "由 Docker 管理。任务登记不代表容器当前存在。",
        ),
        Item::new(
            "exports",
            "原始文件与外部导出",
            "用户",
            "原始文件与导出由用户保管。导出位置随每次保存选择。",
        ),
    ]
}

fn read_references(
    path: &Path,
    groups: &mut [Item],
    cancel: &watch::Receiver<bool>,
) -> AppResult<()> {
    check_cancel(cancel)?;
    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file() && !is_link(&metadata)) {
        return Err("无法从受管数据库文件读取引用。".into());
    }
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    c.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(error)?;
    c.execute_batch("PRAGMA query_only=ON; BEGIN DEFERRED")
        .map_err(error)?;
    for item in groups {
        check_cancel(cancel)?;
        let (counts, payload): (&[(&str, &str)], Option<&str>) = match item.id {
            "sources" => (
                &[
                    ("文件版本", "SELECT count(*) FROM source_versions"),
                    (
                        "知识到文件版本的引用",
                        "SELECT count(*) FROM knowledge_source_links",
                    ),
                ],
                Some("SELECT coalesce(sum(length(CAST(content AS BLOB))),0) FROM source_versions"),
            ),
            "history" => (
                &[
                    ("对话与评测执行记录", "SELECT count(*) FROM execution_runs"),
                    ("处理快照", "SELECT count(*) FROM context_snapshots"),
                    (
                        "处理快照中的知识引用",
                        "SELECT count(*) FROM context_knowledge_items",
                    ),
                    (
                        "知识快照中的版本引用",
                        "SELECT count(*) FROM knowledge_snapshot_items",
                    ),
                    (
                        "评测批次中的结果引用",
                        "SELECT count(run_id) FROM benchmark_batch_items",
                    ),
                ],
                None,
            ),
            "indexes" => (
                &[
                    (
                        "工作区配置关联的本机向量索引",
                        "SELECT count(*) FROM rag_workspace_settings",
                    ),
                    (
                        "云端连接关联的向量索引",
                        "SELECT count(*) FROM model_cloud_indexes",
                    ),
                    (
                        "本机和云端向量数",
                        "SELECT (SELECT count(*) FROM rag_vectors)+(SELECT count(*) FROM model_cloud_vectors)",
                    ),
                ],
                Some(
                    "SELECT (SELECT coalesce(sum(length(vector)),0) FROM rag_vectors)+(SELECT coalesce(sum(length(vector)),0) FROM model_cloud_vectors)+(SELECT coalesce(sum(length(block)),0) FROM knowledge_fts_data)",
                ),
            ),
            "knowledge-content" => (
                &[("知识版本", "SELECT count(*) FROM knowledge_versions")],
                Some("SELECT coalesce(sum(length(CAST(title AS BLOB))+length(CAST(content AS BLOB))),0) FROM knowledge_versions"),
            ),
            "benchmark-results" => (
                &[
                    ("评测运行", "SELECT count(*) FROM benchmark_runs"),
                    ("独立评分记录", "SELECT count(*) FROM benchmark_evaluations"),
                ],
                Some("SELECT
                    (SELECT coalesce(sum(length(CAST(config_snapshot_json AS BLOB))+length(CAST(hardware_snapshot_json AS BLOB))),0) FROM benchmark_runs)
                    +(SELECT coalesce(sum(length(CAST(metrics_json AS BLOB))+length(CAST(details_json AS BLOB))),0) FROM benchmark_evaluations)
                    +(SELECT coalesce(sum(length(CAST(values_json AS BLOB))),0) FROM benchmark_resource_samples)"),
            ),
            "harness" => (
                &[
                    (
                        "项目资料快照",
                        "SELECT count(*) FROM benchmark_repository_snapshots",
                    ),
                    (
                        "项目适配审核",
                        "SELECT count(*) FROM harness_fit_assessments",
                    ),
                    (
                        "项目任务的修订与结果记录",
                        "SELECT count(*) FROM benchmark_generated_cases r JOIN benchmark_generation_jobs j ON j.id=r.job_id WHERE substr(json_extract(j.plan_json,'$.purpose'),1,8)='harness_'",
                    ),
                ],
                Some("SELECT
                    (SELECT coalesce(sum(length(CAST(files_json AS BLOB))),0) FROM benchmark_repository_snapshots)
                    +(SELECT coalesce(sum(length(CAST(plan_json AS BLOB))),0) FROM benchmark_generation_jobs WHERE substr(json_extract(plan_json,'$.purpose'),1,8)='harness_')
                    +(SELECT coalesce(sum(length(CAST(candidate_json AS BLOB))),0) FROM benchmark_generated_cases r JOIN benchmark_generation_jobs j ON j.id=r.job_id WHERE substr(json_extract(j.plan_json,'$.purpose'),1,8)='harness_')"),
            ),
            "external-models" => (
                &[
                    (
                        "引用本机模型的运行方案",
                        "SELECT count(*) FROM model_runtime_profiles WHERE 'local' IN (json_extract(profile_json,'$.generation.kind'),json_extract(profile_json,'$.embedding.kind'),json_extract(profile_json,'$.reranking.kind'))",
                    ),
                    (
                        "配置的本机向量索引",
                        "SELECT count(*) FROM rag_workspace_settings",
                    ),
                ],
                None,
            ),
            "external-docker" => (
                &[
                    (
                        "带镜像或容器编号的任务计划",
                        "SELECT count(*) FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.imageId') IS NOT NULL OR json_extract(plan_json,'$.containerName') IS NOT NULL",
                    ),
                    (
                        "运行中或需检查清理的项目任务",
                        "SELECT count(*) FROM benchmark_generation_jobs WHERE substr(json_extract(plan_json,'$.purpose'),1,8)='harness_' AND status IN ('checking','opening','ready','validating','running','grading','interrupted','cleanup_required')",
                    ),
                ],
                None,
            ),
            "exports" => (
                &[(
                    "已添加的原始文件记录",
                    "SELECT count(*) FROM source_documents",
                )],
                None,
            ),
            _ => continue,
        };
        item.references = Some(references(&c, counts)?);
        item.payload_bytes = payload.map(|sql| count(&c, sql)).transpose()?;
    }
    check_cancel(cancel)
}

fn inventory(
    path: &Path,
    profile: Option<&Path>,
    cancel: &watch::Receiver<bool>,
) -> AppResult<Inventory> {
    let directory = path
        .parent()
        .filter(|path| path.is_absolute())
        .ok_or("无法定位应用的数据目录。")?;
    if !fs::symlink_metadata(directory)
        .is_ok_and(|metadata| metadata.is_dir() && !is_link(&metadata))
    {
        return Err("应用的数据目录无法读取，或是目录链接。".into());
    }
    let mut groups = evidence_groups();
    let reference_error = read_references(path, &mut groups, cancel).err();
    check_cancel(cancel)?;
    let files = [
        (
            "database",
            "主数据库",
            path.to_owned(),
            "工作区、会话、知识、评测和项目任务都保存在这里。",
        ),
        (
            "wal",
            "数据库待写回记录（WAL）",
            directory.join("cortexos.sqlite3-wal"),
            "SQLite 使用它保存尚未写回的已提交内容。",
        ),
        (
            "shm",
            "数据库运行文件（SHM）",
            directory.join("cortexos.sqlite3-shm"),
            "SQLite 正在管理此文件。",
        ),
    ];
    let mut items: Vec<Item> = files
        .into_iter()
        .map(|(id, label, path, reason)| {
            Item::new(id, label, "CortexOS", reason).at(&path, Some(regular_file(&path)))
        })
        .collect();
    items.push(
        Item::new(
            "backups",
            "应用升级备份",
            "CortexOS",
            "升级前的完整副本，可用于迁移失败时恢复。",
        )
        .at(directory, Some(directory_size(directory, true, cancel)?)),
    );
    for (id, label, dir, reason) in [
        ("portable-data", "备份与恢复记录", directory.join("data-portability"), "包含恢复前自动备份、恢复暂存及本机备份记录。用户另选位置的备份由用户保管。"),
        ("cache-recovery-data", "已移出的界面缓存", directory.join("storage-recovery"), "界面缓存移出后的副本，用于撤销缓存回收。"),
    ] {
        items.push(Item::new(id, label, "CortexOS", reason).at(&dir, Some(directory_size(&dir, false, cancel)?)));
    }
    let mut webview = Item::new(
        "webview",
        "应用界面数据与缓存",
        "WebView2",
        "可能包含界面设置。自定义位置需另行核对。",
    );
    if let Some(profile) = profile.filter(|path| {
        path.parent().is_some_and(|parent| {
            fs::symlink_metadata(parent)
                .is_ok_and(|metadata| metadata.is_dir() && !is_link(&metadata))
        })
    }) {
        webview = webview.at(profile, Some(directory_size(profile, false, cancel)?));
    }
    items.push(webview);
    let sizes = items.iter().filter_map(|item| item.size.as_ref());
    let file_bytes = sizes.clone().map(|size| size.logical_bytes).sum();
    let partial_file_total =
        sizes.clone().any(Size::partial) || items.iter().any(|item| item.size.is_none());
    items.extend(groups.into_iter().map(|item| Item {
        location: (item.owner == "工作区").then(|| path.to_string_lossy().into_owned()),
        ..item
    }));
    Ok(Inventory {
        sampled_at: now(),
        file_bytes,
        partial_file_total,
        active_task_observed: false,
        reference_error,
        items,
        reclaim_preview: None,
    })
}

fn validate_scan_id(id: &str) -> AppResult<()> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "读取编号无效。".into())
}

#[tauri::command]
pub(crate) async fn storage_inventory(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: String,
    preview: Option<bool>,
) -> AppResult<Inventory> {
    validate_scan_id(&scan_id)?;
    let db = state.database.clone()?;
    let lease = state
        .storage_scans
        .claim(&scan_id, SCOPE, TaskKind::LocalMaintenance)?;
    if state.closing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("应用正在关闭，已停止读取存储清单。".into());
    }
    let receiver = lease.receiver();
    let preview = preview.unwrap_or(false);
    let active = state.has_active();
    let webview = if preview {
        verified_webview_data_dir(&app, db.path.parent().ok_or("无法定位应用的数据目录。")?).await
    } else {
        None
    };
    // The preview reads only known cache subdirectories.
    let profile =
        if !preview && cfg!(windows) && std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
            app.path()
                .app_local_data_dir()
                .ok()
                .map(|path| path.join("EBWebView"))
        } else {
            None
        };
    #[cfg(windows)]
    let approval = state.storage_preview.clone();
    let mut report = blocking(db, move |db| {
        let mut report = inventory(&db.path, profile.as_deref(), &receiver)?;
        if preview {
            let data_dir = db.path.parent().ok_or("无法定位应用的数据目录。")?;
            #[allow(unused_mut)]
            let mut preview = reclaim_preview(
                data_dir,
                webview.as_ref().map(|(path, _)| path.as_path()),
                &receiver,
            )?;
            #[cfg(windows)]
            {
                let ticket = webview
                    .and_then(|(profile, pid)| recovery::preview_ticket(profile, pid, &preview));
                preview.approval_id = ticket.as_ref().map(|ticket| ticket.id.clone());
                preview.approval_caches = ticket
                    .as_ref()
                    .map_or_else(Vec::new, recovery::PreviewTicket::cache_ids);
                *approval
                    .lock()
                    .map_err(|_| "无法保存本次预览，请重新读取。")? = ticket;
            }
            report.reclaim_preview = Some(preview);
        }
        Ok(report)
    })
    .await?;
    report.active_task_observed = active || state.has_active();
    Ok(report)
}

#[tauri::command]
pub(crate) fn cancel_storage_inventory(
    state: State<'_, AppState>,
    scan_id: String,
) -> AppResult<()> {
    validate_scan_id(&scan_id)?;
    state
        .storage_scans
        .cancel(&scan_id, SCOPE, TaskKind::LocalMaintenance)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn storage_reclaim_candidates_fail_closed_and_leave_files_intact() {
        let root = std::env::temp_dir().join(format!("cortexos-storage-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        let webview = root.join("webview");
        let profile = webview.join("EBWebView").join("Default");
        let cache = profile.join("Cache");
        let site_data = profile.join("Local Storage");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&site_data).unwrap();
        fs::create_dir(&data).unwrap();
        let cached = cache.join("test-cache");
        let stored = site_data.join("test-site");
        let invalid = profile.join("Code Cache");
        fs::write(&cached, b"cache").unwrap();
        fs::write(&stored, b"do not measure").unwrap();
        fs::write(&invalid, b"unexpected file").unwrap();

        let (cancel, receive) = watch::channel(false);
        let report = reclaim_preview(&data, Some(&webview), &receive).unwrap();
        let recovery = data.join("storage-recovery");
        assert_eq!(report.recovery_directory.as_deref(), recovery.to_str());
        assert!(report.recovery_free_bytes.is_some());
        assert!(!recovery.exists());
        let [regular, invalid_folder, absent] = report.cache_items.as_slice() else {
            panic!("expected exactly three cache locations");
        };
        assert_eq!(regular.assessment, CacheAssessment::Rebuildable);
        assert_eq!(regular.item.size.as_ref().unwrap().logical_bytes, 5);
        assert_eq!(invalid_folder.assessment, CacheAssessment::Unverified);
        assert!(invalid_folder.item.size.is_none());
        assert_eq!(absent.assessment, CacheAssessment::Empty);

        fs::write(&recovery, b"unexpected recovery file").unwrap();
        let blocked = reclaim_preview(&data, None, &receive).unwrap();
        assert!(blocked.recovery_directory.is_none());
        assert!(blocked.recovery_free_bytes.is_none());
        assert!(blocked.cache_items.iter().all(|row| {
            row.assessment == CacheAssessment::Unverified && row.item.location.is_none()
        }));
        assert_eq!(fs::read(&cached).unwrap(), b"cache");
        assert_eq!(fs::read(&stored).unwrap(), b"do not measure");
        assert_eq!(fs::read(&invalid).unwrap(), b"unexpected file");
        assert_eq!(fs::read(&recovery).unwrap(), b"unexpected recovery file");
        cancel.send(true).unwrap();
        assert!(
            matches!(reclaim_preview(&data, Some(&webview), &receive), Err(message) if message == STOPPED)
        );
    }
}
