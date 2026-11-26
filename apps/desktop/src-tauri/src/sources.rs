use crate::{
    db::Database,
    doc_extract,
    models::{AppResult, MessageImage, new_id, now, validate_message_images},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

fn error(_: rusqlite::Error) -> String {
    "文件资料保存或读取失败，请保留当前内容并重试。".into()
}
fn missing() -> String {
    "文件不存在，或不在当前工作区里。".into()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub latest_version: i64,
    pub enabled: bool,
    pub revision: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub id: String,
    pub workspace_id: String,
    pub path: String,
    pub last_synced_at: Option<String>,
    pub created_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub document: Source,
    pub version: i64,
    pub content: String,
    pub content_hash: String,
    pub created_at: String,
    pub(crate) structure: crate::source_chunks::StructureSummary,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRef {
    pub source_id: String,
    pub version: i64,
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePreview {
    pub ticket: String,
    pub name: String,
    pub content: String,
    pub content_hash: String,
    pub previous_version: Option<i64>,
    pub previous_revision: Option<i64>,
}
#[derive(Clone)]
pub struct PendingSource {
    pub workspace: String,
    pub path: String,
    pub raw_hash: String,
    pub preview: SourcePreview,
    pub created: Instant,
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "docx", "pptx", "xlsx", "epub", "pdf", "csv", "json", "toml", "yaml", "yml",
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h", "html", "css", "sql",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedKnowledgeItem {
    pub source_id: String,
    pub node_id: String,
    pub version: i64,
    pub title: String,
    pub file_name: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedFileError {
    pub file_name: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedFilesReport {
    pub documents: Vec<DroppedKnowledgeItem>,
    pub images: Vec<MessageImage>,
    pub errors: Vec<DroppedFileError>,
}

pub fn import_dropped_paths(
    db: &Database,
    workspace_id: &str,
    paths: &[String],
) -> AppResult<DroppedFilesReport> {
    db.source_previous(workspace_id, "")?;
    if paths.is_empty() {
        return Err("请至少拖入 1 个文件。".into());
    }
    if paths.len() > 20 {
        return Err("单次最多拖入 20 个文件。".into());
    }

    let mut report = DroppedFilesReport {
        documents: Vec::new(),
        images: Vec::new(),
        errors: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for p in paths.iter().filter(|p| seen.insert(*p)) {
        let path = Path::new(p);
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(p)
            .to_string();
        let ext = extension(path);
        let image_mime = match ext.as_str() {
            "jpg" | "jpeg" => Some("image/jpeg"),
            "png" => Some("image/png"),
            "webp" => Some("image/webp"),
            "gif" => Some("image/gif"),
            _ => None,
        };
        let result = if selected_metadata(path).is_ok_and(|meta| meta.is_dir()) {
            Err("暂不支持直接拖入文件夹，请点击「资料文件 → 同步文件夹」绑定目录。".into())
        } else if let Some(mime_type) = image_mime {
            if report.images.len() >= 4 {
                Err("单次最多附带 4 张图片。".into())
            } else {
                read_dropped_image(path, mime_type).map(|image| report.images.push(image))
            }
        } else if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
            import_dropped_document(db, workspace_id, path).map(|item| {
                if let Some(item) = item {
                    report.documents.push(item);
                }
            })
        } else {
            Err("不支持的文件格式（支持 PDF、PPTX、DOCX、XLSX、EPUB、TXT、MD、代码及常见图片）。".into())
        };
        if let Err(reason) = result {
            report.errors.push(DroppedFileError { file_name, reason });
        }
    }
    Ok(report)
}

fn import_dropped_document(
    db: &Database,
    workspace: &str,
    path: &Path,
) -> AppResult<Option<DroppedKnowledgeItem>> {
    let pending = read_selected(path, workspace, db)?;
    let previous = pending.preview.previous_version;
    let source = db.import_source(pending)?;
    let status = match previous {
        None => "imported",
        Some(version) if source.latest_version > version => "updated",
        _ => "unchanged",
    };
    db.connection()?
        .query_row(
            "SELECT n.id, v.version, v.title, 0 AS priority, n.updated_at
             FROM source_managed_knowledge m
             JOIN knowledge_nodes n ON n.id=m.node_id
             JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
             WHERE m.source_id=?1 AND n.workspace_id=?2
             UNION ALL
             SELECT n.id, v.version, v.title, 1 AS priority, n.updated_at
             FROM knowledge_source_links l
             JOIN knowledge_nodes n ON n.id=l.node_id
             JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
             WHERE l.source_id=?1 AND n.workspace_id=?2
             ORDER BY priority, updated_at DESC, id
             LIMIT 1",
            params![source.id, workspace],
            |row| {
                Ok(DroppedKnowledgeItem {
                    source_id: source.id.clone(),
                    node_id: row.get(0)?,
                    version: row.get(1)?,
                    title: row.get(2)?,
                    file_name: source.name.clone(),
                    status: status.into(),
                })
            },
        )
        .optional()
        .map_err(error)
}

fn read_dropped_image(path: &Path, mime_type: &str) -> AppResult<MessageImage> {
    let (file, _, _) = open_selected(path)?;
    let bytes = read_bounded(
        file,
        2 * 1024 * 1024,
        "图片文件超过 2 MiB，请压缩后重试或使用 Ctrl+V 粘贴截图。",
    )?;
    let image = MessageImage {
        mime_type: mime_type.into(),
        data_base64: encode_base64(&bytes),
    };
    validate_message_images(std::slice::from_ref(&image))?;
    Ok(image)
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        encoded.push(ALPHABET[usize::from(first >> 2)] as char);
        encoded.push(ALPHABET[usize::from(((first & 3) << 4) | (second >> 4))] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[usize::from(((second & 15) << 2) | (third >> 6))] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[usize::from(third & 63)] as char
        } else {
            '='
        });
    }

    encoded
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSyncReport {
    pub folder: String,
    pub imported: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub remaining: usize,
    pub error: Option<String>,
}

pub fn sync_folder(root: &Path, workspace: &str, db: &Database) -> AppResult<FolderSyncReport> {
    let canonical = canonical_folder(root)?;
    let path = canonical.to_str().ok_or("文件夹路径编码无效。")?;
    let id = db.bind_watched_folder(workspace, path)?;
    db.sync_watched_folder(workspace, &id)
}

fn canonical_folder(root: &Path) -> AppResult<PathBuf> {
    validate_local_path(root)?;
    for parent in root.ancestors() {
        let meta = std::fs::symlink_metadata(parent)
            .map_err(|_| "无法访问所选文件夹，请检查权限。".to_string())?;
        if linked(&meta) {
            return Err("不支持通过符号链接同步文件夹，请选择实际目录。".into());
        }
    }
    if !root.is_dir() {
        return Err("请选择有效的本地文件夹。".into());
    }
    let canonical = std::fs::canonicalize(root).map_err(|_| "无法解析文件夹位置。".to_string())?;
    validate_local_path(&canonical)?;
    Ok(canonical)
}

fn linked(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

fn scan_folder(
    root: &Path,
    workspace: &str,
    db: &Database,
    folder_id: &str,
    cursor: Option<&str>,
) -> AppResult<FolderSyncReport> {
    let canonical = canonical_folder(root)?;
    if canonical != root {
        return Err("该文件夹位置已变化，请移除绑定后重新选择。".into());
    }
    let mut stack = vec![canonical.clone()];
    let mut files = BTreeSet::new();
    let mut candidates = 0usize;
    let mut skipped = 0usize;
    const MAX_FILES: usize = 60;
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) if dir == canonical => return Err("无法读取绑定文件夹，请检查权限。".into()),
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                skipped += 1;
                continue;
            };
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                skipped += 1;
                continue;
            };
            if linked(&meta) {
                skipped += 1;
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') {
                continue;
            }
            if meta.is_dir() {
                if matches!(name.as_str(), "node_modules" | "target" | "dist" | "build" | "__pycache__" | ".git") {
                    continue;
                }
                stack.push(path);
            } else if meta.is_file() {
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
                    match path.to_str() {
                        Some(path) if cursor.is_none_or(|cursor| path > cursor) => {
                            candidates += 1;
                            files.insert(path.to_owned());
                            if files.len() > MAX_FILES {
                                files.pop_last();
                            }
                        }
                        Some(_) => {}
                        None => skipped += 1,
                    }
                }
            }
        }
    }
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut unchanged = 0usize;
    let remaining = candidates - files.len();
    let next_cursor = files.last().filter(|_| remaining > 0);
    for path in &files {
        match read_selected(Path::new(path), workspace, db) {
            Ok(pending) => {
                let prev = pending.preview.previous_version;
                match db.import_source_with_binding(pending, Some(folder_id)) {
                    Ok(doc) if !doc.enabled => skipped += 1,
                    Ok(doc) => match prev {
                        None => imported += 1,
                        Some(old) if doc.latest_version > old => updated += 1,
                        _ => unchanged += 1,
                    },
                    Err(_) => skipped += 1,
                }
            }
            Err(_) => skipped += 1,
        }
    }
    let c = db.connection()?;
    let changed = c.execute(
        "UPDATE watched_folders SET scan_cursor=?4,last_synced_at=?5
         WHERE workspace_id=?1 AND id=?2 AND scan_cursor IS ?3",
        params![workspace, folder_id, cursor, next_cursor, now()],
    ).map_err(error)?;
    if changed != 1 {
        return Err("文件夹绑定或同步进度已变化，请重新同步。".into());
    }
    let report = FolderSyncReport {
        folder: canonical.to_string_lossy().into_owned(),
        imported,
        updated,
        unchanged,
        skipped,
        remaining,
        error: None,
    };
    Ok(report)
}

fn validate_local_path(path: &Path) -> AppResult<()> {
    if !path.is_absolute() {
        return Err("请用选择窗口选择本机位置。".into());
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        {
            return Err("请选择本机磁盘上的位置。".into());
        }
    }
    Ok(())
}

fn selected_metadata(path: &Path) -> AppResult<std::fs::Metadata> {
    validate_local_path(path)?;
    if path.components().any(|component| match component {
        std::path::Component::ParentDir => true,
        #[cfg(windows)]
        std::path::Component::Normal(name) => name.to_string_lossy().contains(':'),
        _ => false,
    }) {
        return Err("请选择文件的实际位置。".into());
    }
    // Check from the root down so metadata lookup doesn't traverse a known link.
    let ancestors: Vec<_> = path.ancestors().collect();
    for parent in ancestors.into_iter().rev() {
        let meta = std::fs::symlink_metadata(parent)
            .map_err(|_| "无法读取所选文件，请检查权限。".to_string())?;
        if linked(&meta) {
            return Err("文件或所在文件夹有链接跳转，请从实际位置选择原文件。".into());
        }
        if parent == path {
            return Ok(meta);
        }
    }
    Err("无法读取所选文件，请检查权限。".into())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

const UNSAFE_SELECTION: &str = "支持 .pdf / .pptx / .docx / .xlsx / .epub / .md / .txt 及常见代码与配置文件；名称涉及密码、访问密钥等敏感信息的文件不能导入。";

fn checked_file_name(path: &Path) -> AppResult<&str> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("文件名无效。")?;
    let lower = name.to_lowercase();
    if lower.starts_with('.')
        || ["token", "credential", "secret", "password", "id_rsa"]
            .iter()
            .any(|term| lower.contains(term))
    {
        return Err(UNSAFE_SELECTION.into());
    }
    Ok(name)
}

fn open_selected(path: &Path) -> AppResult<(std::fs::File, PathBuf, &str)> {
    let name = checked_file_name(path)?;
    if !selected_metadata(path)?.is_file() {
        return Err("请选择本地磁盘上的普通文件。".into());
    }

    let canonical =
        std::fs::canonicalize(path).map_err(|_| "无法找到所选文件的实际位置。".to_string())?;
    validate_local_path(&canonical)?;
    checked_file_name(&canonical)?;

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000);
    }
    let file = options
        .open(path)
        .map_err(|_| "文件被占用或不可读，请关闭写入程序后重试。".to_string())?;
    // Validate the opened handle, so a changed parent path cannot redirect the read.
    #[cfg(windows)]
    {
        use std::{
            ffi::{OsString, c_void},
            os::windows::{ffi::OsStringExt, io::AsRawHandle},
        };
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFinalPathNameByHandleW(
                handle: *mut c_void,
                buffer: *mut u16,
                length: u32,
                flags: u32,
            ) -> u32;
        }
        let mut buffer = vec![0u16; 32768];
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                0,
            )
        } as usize;
        if length == 0
            || length >= buffer.len()
            || PathBuf::from(OsString::from_wide(&buffer[..length])) != canonical
        {
            return Err("文件路径在读取前发生变化，请重新选择。".into());
        }
    }
    Ok((file, canonical, name))
}

fn read_bounded(file: std::fs::File, max_bytes: u64, oversized: &str) -> AppResult<Vec<u8>> {
    let metadata = file
        .metadata()
        .map_err(|_| "无法读取文件属性。".to_string())?;
    if !metadata.is_file() || linked(&metadata) {
        return Err("请选择本地磁盘上的普通文件。".into());
    }
    if metadata.len() > max_bytes {
        return Err(oversized.into());
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "文件读取失败。".to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err(oversized.into());
    }
    Ok(bytes)
}

pub fn read_selected(path: &Path, workspace: &str, db: &Database) -> AppResult<PendingSource> {
    let extension = extension(path);
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(UNSAFE_SELECTION.into());
    }
    let (file, canonical, name) = open_selected(path)?;
    let max_bytes = if matches!(extension.as_str(), "docx" | "pptx" | "xlsx" | "epub" | "pdf") {
        doc_extract::MAX_BINARY_FILE_BYTES
    } else {
        2 * 1024 * 1024
    };
    let max_mib = max_bytes / (1024 * 1024);
    let bytes = read_bounded(
        file,
        max_bytes,
        &format!("请选择不超过 {max_mib} MiB 的普通文件。"),
    )?;
    let raw_hash = hash(&bytes);
    let content = match doc_extract::extract_document_text(&extension, &bytes) {
        Some(text) => text?,
        None => {
            let text = String::from_utf8(bytes)
                .map_err(|_| "文件不是 UTF-8，请转换编码后重新选择。".to_string())?;
            text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
        }
    };
    if content.trim().is_empty() || content.contains('\0') {
        return Err("文件为空或含有二进制内容，请选择普通文档或文本。".into());
    }
    let path = canonical.to_str().ok_or("文件路径编码无效。")?.to_string();
    let previous = db.source_previous(workspace, &path)?;
    Ok(PendingSource {
        workspace: workspace.into(),
        path,
        raw_hash,
        created: Instant::now(),
        preview: SourcePreview {
            ticket: new_id(),
            name: name.into(),
            content_hash: hash(content.as_bytes()),
            content,
            previous_version: previous.map(|p| p.0),
            previous_revision: previous.map(|p| p.1),
        },
    })
}

fn source(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: row.get(0)?,
        name: row.get(1)?,
        latest_version: row.get(2)?,
        enabled: row.get(3)?,
        revision: row.get(4)?,
    })
}
pub fn available(connection: &Connection, node: &str, version: i64) -> AppResult<bool> {
    let exists:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='knowledge_source_links')",[],|r|r.get(0)).map_err(error)?;
    if !exists {
        return Ok(true);
    }
    connection.query_row("SELECT NOT EXISTS(SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id WHERE l.node_id=?1 AND l.knowledge_version=?2 AND s.enabled=0)",params![node,version],|r|r.get(0)).map_err(error)
}
pub fn evidence(connection: &Connection, node: &str, version: i64) -> AppResult<Vec<Value>> {
    let mut stmt=connection.prepare("SELECT l.source_id,l.source_version,s.name,s.enabled,l.start_line,l.end_line,l.excerpt,v.content_hash FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id JOIN source_versions v ON v.source_id=l.source_id AND v.version=l.source_version WHERE l.node_id=?1 AND l.knowledge_version=?2").map_err(error)?;
    stmt.query_map(params![node,version],|r|Ok(json!({"sourceId":r.get::<_,String>(0)?,"version":r.get::<_,i64>(1)?,"name":r.get::<_,String>(2)?,"enabled":r.get::<_,bool>(3)?,"startLine":r.get::<_,i64>(4)?,"endLine":r.get::<_,i64>(5)?,"excerpt":r.get::<_,String>(6)?,"contentHash":r.get::<_,String>(7)?}))).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)
}
pub fn excerpt(
    connection: &Connection,
    workspace: &str,
    reference: &SourceRef,
) -> AppResult<String> {
    let content:Option<String>=connection.query_row("SELECT v.content FROM source_versions v JOIN source_documents s ON s.id=v.source_id WHERE s.workspace_id=?1 AND s.id=?2 AND v.version=?3 AND s.enabled=1",params![workspace,reference.source_id,reference.version],|r|r.get(0)).optional().map_err(error)?;
    let content = content.ok_or("文件不存在、已停用，或不在当前工作区里。")?;
    let lines: Vec<_> = content.split('\n').collect();
    if reference.start_line == 0
        || reference.end_line < reference.start_line
        || reference.end_line > lines.len()
    {
        return Err("请选择有效的起止行。".into());
    }
    let text = lines[reference.start_line - 1..reference.end_line].join("\n");
    if text.trim().is_empty() || text.chars().count() > 20000 {
        return Err("选中的文字需为1–20000字，请减少选择的行数。".into());
    }
    Ok(text)
}
fn refresh_nodes(c: &Connection, id: &str) -> AppResult<()> {
    let ids = c
        .prepare("SELECT DISTINCT node_id FROM knowledge_source_links WHERE source_id=?1")
        .map_err(error)?
        .query_map(params![id], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    for id in ids {
        crate::retrieval::index_node(c, &id)?;
    }
    Ok(())
}
impl Database {
    pub fn source_previous(&self, workspace: &str, path: &str) -> AppResult<Option<(i64, i64)>> {
        let c = self.connection()?;
        let valid: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
                params![workspace],
                |r| r.get(0),
            )
            .map_err(error)?;
        if !valid {
            return Err(missing());
        }
        c.query_row("SELECT latest_version,revision FROM source_documents WHERE workspace_id=?1 AND canonical_path=?2",params![workspace,path],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(error)
    }
    pub fn import_source(&self, pending: PendingSource) -> AppResult<Source> {
        self.import_source_with_binding(pending, None)
    }
    fn import_source_with_binding(
        &self,
        pending: PendingSource,
        folder_id: Option<&str>,
    ) -> AppResult<Source> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        if let Some(folder_id) = folder_id {
            let path: String = tx.query_row(
                "SELECT path FROM watched_folders WHERE workspace_id=?1 AND id=?2",
                params![pending.workspace, folder_id],
                |r| r.get(0),
            ).optional().map_err(error)?.ok_or("未找到该同步文件夹绑定。")?;
            if !Path::new(&pending.path).starts_with(Path::new(&path)) {
                return Err("文件位置已变化，或不在绑定文件夹中。".into());
            }
        }
        let previous:Option<(Source,String)>=tx.query_row("SELECT s.id,s.name,s.latest_version,s.enabled,s.revision,v.raw_hash FROM source_documents s JOIN source_versions v ON v.source_id=s.id AND v.version=s.latest_version WHERE s.workspace_id=?1 AND s.canonical_path=?2",params![pending.workspace,pending.path],|r|Ok((source(r)?,r.get(5)?))).optional().map_err(error)?;
        if previous
            .as_ref()
            .map(|(s, _)| (s.latest_version, s.revision))
            != pending
                .preview
                .previous_version
                .zip(pending.preview.previous_revision)
        {
            return Err("文件资料已被更新，请重新选择文件并预览。".into());
        }
        let timestamp = now();
        let (id, version, kind) = if let Some((old, raw)) = previous {
            if (raw == pending.raw_hash && old.enabled) || (folder_id.is_some() && !old.enabled) {
                tx.commit().map_err(error)?;
                return Ok(old);
            }
            let version = old.latest_version + i64::from(raw != pending.raw_hash);
            tx.execute("UPDATE source_documents SET latest_version=?2,enabled=1,revision=revision+1,updated_at=?3 WHERE id=?1",params![old.id,version,timestamp]).map_err(error)?;
            (
                old.id,
                version,
                if raw == pending.raw_hash {
                    "reauthorized"
                } else {
                    "reimported"
                },
            )
        } else {
            let id = new_id();
            tx.execute(
                "INSERT INTO source_documents VALUES(?1,?2,?3,?4,1,1,1,?5,?5)",
                params![
                    id,
                    pending.workspace,
                    pending.path,
                    pending.preview.name,
                    timestamp
                ],
            )
            .map_err(error)?;
            (id, 1, "imported")
        };
        if kind != "reauthorized" {
            tx.execute(
                "INSERT INTO source_versions VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    id,
                    version,
                    pending.preview.content,
                    pending.preview.content_hash,
                    pending.raw_hash,
                    timestamp
                ],
            )
            .map_err(error)?;
        }
        tx.execute(
            "INSERT INTO source_events VALUES(?1,?2,?3,?4)",
            params![new_id(), id, kind, timestamp],
        )
        .map_err(error)?;
        let managed: Option<(String, i64, bool)> = tx.query_row(
            "SELECT n.id,n.latest_version,n.latest_version=m.managed_version
                    AND n.effective_version IS m.managed_version
             FROM source_managed_knowledge m JOIN knowledge_nodes n ON n.id=m.node_id
             WHERE m.source_id=?1 AND n.workspace_id=?2",
            params![id, pending.workspace],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).optional().map_err(error)?;
        // Continue the imported knowledge history while its active version is still source-managed.
        if kind != "reauthorized" && managed.as_ref().is_none_or(|(_, _, active)| *active) {
            let line_count = pending.preview.content.split('\n').count().max(1) as i64;
            let content_excerpt: String = pending.preview.content.chars().take(20000).collect();
            let name = &pending.preview.name;
            let path = Path::new(name);
            let title = if matches!(
                extension(path).as_str(),
                "pdf" | "pptx" | "docx" | "xlsx" | "epub" | "md" | "txt" | "csv"
            ) {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(name)
            } else {
                name
            };
            let doc_title: String = title.chars().take(80).collect();
            let (node_id, node_version) = if let Some((existing_id, prev_ver, _)) = managed {
                let next_ver = prev_ver + 1;
                tx.execute(
                    "UPDATE knowledge_nodes SET latest_version=?2,effective_version=?2,revision=revision+1,updated_at=?3 WHERE id=?1",
                    params![existing_id, next_ver, timestamp],
                )
                .map_err(error)?;
                (existing_id, next_ver)
            } else {
                let nid = new_id();
                tx.execute(
                    "INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at,source_conversation_id,source_message_id,source_excerpt,source_hash) VALUES(?1,?2,1,1,2,?3,?3,NULL,NULL,NULL,NULL)",
                    params![nid, pending.workspace, timestamp],
                )
                .map_err(error)?;
                (nid, 1)
            };
            tx.execute(
                "INSERT INTO knowledge_versions(node_id,version,title,kind,content,author,change_reason,created_at) VALUES(?1,?2,?3,'knowledge',?4,'user','导入资料自动生效',?5)",
                params![node_id, node_version, doc_title, content_excerpt, timestamp],
            )
            .map_err(error)?;
            tx.execute(
                "INSERT INTO knowledge_approval_events(id,node_id,version,actor,created_at) VALUES(?1,?2,?3,'user',?4)",
                params![new_id(), node_id, node_version, timestamp],
            )
            .map_err(error)?;
            tx.execute(
                "INSERT INTO knowledge_source_links VALUES(?1,?2,?3,?4,1,?5,?6)",
                params![node_id, node_version, id, version, line_count, content_excerpt],
            )
            .map_err(error)?;
            tx.execute(
                "INSERT INTO source_managed_knowledge VALUES(?1,?2,?3)
                 ON CONFLICT(source_id) DO UPDATE SET managed_version=excluded.managed_version",
                params![id, node_id, node_version],
            )
            .map_err(error)?;
        }
        refresh_nodes(&tx, &id)?;
        let document = tx.query_row(
            "SELECT id,name,latest_version,enabled,revision FROM source_documents WHERE id=?1",
            params![id],
            source,
        ).map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(document)
    }
    pub fn sources(&self, workspace: &str) -> AppResult<Vec<Source>> {
        let c = self.connection()?;
        let mut stmt=c.prepare("SELECT id,name,latest_version,enabled,revision FROM source_documents WHERE workspace_id=?1 ORDER BY updated_at DESC,id").map_err(error)?;
        stmt.query_map(params![workspace], source)
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)
    }
    pub fn source_snapshot(&self, workspace: &str, id: &str, version: i64) -> AppResult<Snapshot> {
        let c = self.connection()?;
        let document=c.query_row("SELECT id,name,latest_version,enabled,revision FROM source_documents WHERE workspace_id=?1 AND id=?2",params![workspace,id],source).optional().map_err(error)?.ok_or_else(missing)?;
        let (content, content_hash, created_at): (String, String, String) = c.query_row(
            "SELECT content,content_hash,created_at FROM source_versions WHERE source_id=?1 AND version=?2",
            params![id,version], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).optional().map_err(error)?.ok_or_else(missing)?;
        drop(c);
        let structure = crate::source_chunks::build(workspace, id, version, &document.name, &content).summary();
        Ok(Snapshot { document, version, content, content_hash, created_at, structure })
    }
    pub fn revoke_source(&self, workspace: &str, id: &str, expected: i64) -> AppResult<()> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let changed=tx.execute("UPDATE source_documents SET enabled=0,revision=revision+1,updated_at=?4 WHERE workspace_id=?1 AND id=?2 AND revision=?3 AND enabled=1",params![workspace,id,expected,now()]).map_err(error)?;
        if changed != 1 {
            return Err("文件状态已变化，或不在当前工作区里，请刷新。".into());
        }
        tx.execute(
            "INSERT INTO source_events VALUES(?1,?2,'revoked',?3)",
            params![new_id(), id, now()],
        )
        .map_err(error)?;
        refresh_nodes(&tx, id)?;
        tx.commit().map_err(error)
    }

    fn bind_watched_folder(&self, workspace: &str, path: &str) -> AppResult<String> {
        let timestamp = now();
        let c = self.connection()?;
        c.execute(
            "INSERT INTO watched_folders(id, workspace_id, path, created_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id,path) DO NOTHING",
            params![new_id(), workspace, path, timestamp],
        ).map_err(error)?;
        c.query_row(
            "SELECT id FROM watched_folders WHERE workspace_id=?1 AND path=?2",
            params![workspace, path],
            |r| r.get(0),
        ).map_err(error)
    }

    pub fn list_watched_folders(&self, workspace: &str) -> AppResult<Vec<WatchedFolder>> {
        let c = self.connection()?;
        let mut stmt = c
            .prepare(
                "SELECT id, workspace_id, path, last_synced_at, created_at
                 FROM watched_folders
                 WHERE workspace_id=?1
                 ORDER BY created_at DESC, id",
            )
            .map_err(error)?;
        stmt.query_map(params![workspace], |r| {
            Ok(WatchedFolder {
                id: r.get(0)?,
                workspace_id: r.get(1)?,
                path: r.get(2)?,
                last_synced_at: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)
    }

    pub fn remove_watched_folder(&self, workspace: &str, folder_id: &str) -> AppResult<()> {
        let c = self.connection()?;
        let changed = c
            .execute(
                "DELETE FROM watched_folders WHERE workspace_id=?1 AND id=?2",
                params![workspace, folder_id],
            )
            .map_err(error)?;
        if changed != 1 {
            return Err("未找到该同步文件夹绑定。".into());
        }
        Ok(())
    }

    pub fn sync_watched_folder(
        &self,
        workspace: &str,
        folder_id: &str,
    ) -> AppResult<FolderSyncReport> {
        let (path, cursor): (String, Option<String>) = {
            let c = self.connection()?;
            c.query_row(
                "SELECT path,scan_cursor FROM watched_folders WHERE workspace_id=?1 AND id=?2",
                params![workspace, folder_id],
                |r| Ok((r.get(0)?,r.get(1)?)),
            )
            .optional()
            .map_err(error)?
            .ok_or_else(|| "未找到该同步文件夹绑定。".to_string())?
        };
        let dir = PathBuf::from(&path);
        scan_folder(&dir, workspace, self, folder_id, cursor.as_deref())
    }

    pub fn sync_workspace_watched_folders(
        &self,
        workspace: &str,
    ) -> AppResult<Vec<FolderSyncReport>> {
        let folders = self.list_watched_folders(workspace)?;
        Ok(folders.into_iter().map(|folder| {
            self.sync_watched_folder(workspace, &folder.id)
                .unwrap_or_else(|error| FolderSyncReport {
                    folder: folder.path,
                    imported: 0,
                    updated: 0,
                    unchanged: 0,
                    skipped: 0,
                    remaining: 0,
                    error: Some(error),
                })
        }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_dropped_paths_handles_documents_images_and_rejects_secrets_and_dirs() {
        let root = std::env::temp_dir().join(format!("cortexos-dropped-{}", new_id()));
        let folder = root.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        let db = Database::open(&root.join("data")).unwrap();
        let ws = db.create_workspace("拖入文件测试").unwrap().id;
        let note = root.join("plan.MD");
        let image = root.join("pixel.png");
        let secret = root.join("my_secret_notes.txt");
        std::fs::write(&note, "# 计划\n第一版").unwrap();
        std::fs::write(
            &image,
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01\
              \x08\x00\x00\x00\x00\x3a\x7e\x9b\x55\x00\x00\x00\x0aIDAT\x78\x9c\x63\
              \x60\x00\x00\x00\x02\x00\x01\x48\xaf\xa4\x71\x00\x00\x00\x00IEND\xae\x42\x60\x82",
        )
        .unwrap();
        std::fs::write(&secret, "敏感内容").unwrap();
        let paths = [&secret, &note, &image, &note, &folder]
            .map(|path| path.to_str().unwrap().to_owned());

        let report = import_dropped_paths(&db, &ws, &paths).unwrap();
        assert_eq!(report.documents.len(), 1);
        let item = &report.documents[0];
        assert_eq!(item.status, "imported");
        assert_eq!(item.title, "plan");
        assert_eq!(item.file_name, "plan.MD");
        assert!(uuid::Uuid::parse_str(&item.node_id).is_ok());
        assert_eq!(item.version, 1);
        assert_eq!(report.images.len(), 1);
        assert_eq!(report.images[0].mime_type, "image/png");
        assert_eq!(
            report.images[0].data_base64,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg=="
        );
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.errors[0].file_name, "my_secret_notes.txt");
        assert!(report.errors[0].reason.contains("敏感信息"));
        assert_eq!(report.errors[1].file_name, "folder");
        assert!(report.errors[1].reason.contains("同步文件夹"));

        let repeat = import_dropped_paths(&db, &ws, &paths[1..2]).unwrap();
        assert!(repeat.errors.is_empty());
        assert_eq!(repeat.documents.len(), 1);
        assert_eq!(repeat.documents[0].status, "unchanged");
        assert_eq!(repeat.documents[0].node_id, item.node_id);

        std::fs::write(&note, "# 计划\n第二版").unwrap();
        let update = import_dropped_paths(&db, &ws, &paths[1..2]).unwrap();
        assert!(update.errors.is_empty());
        assert_eq!(update.documents.len(), 1);
        assert_eq!(update.documents[0].status, "updated");
        assert_eq!(update.documents[0].node_id, item.node_id);
        assert_eq!(update.documents[0].version, 2);

        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn watched_folders_upsert_sync_and_remove() {
        let root = std::env::temp_dir().join(format!("cortexos-watched-{}", new_id()));
        std::fs::create_dir_all(&root).unwrap();
        let db = Database::open(&root.join("data")).unwrap();
        let workspace = db.create_workspace("同步测试").unwrap();
        let notes = root.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("plan.md"), "# 计划\n第一版内容").unwrap();

        let report = sync_folder(&notes, &workspace.id, &db).unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.updated, 0);
        let source = db.sources(&workspace.id).unwrap().remove(0);
        let managed = db.knowledge_list(&workspace.id).unwrap().remove(0);
        let mut edit = crate::knowledge::SaveKnowledge {
            workspace_id: workspace.id.clone(),
            node_id: None,
            expected_revision: None,
            title: "我的引用".into(),
            kind: "knowledge".into(),
            content: "手工整理的内容".into(),
            change_reason: "测试手工编辑".into(),
            source_message_id: None,
            source_excerpt: None,
        };
        let manual = db.save_knowledge_with_evidence(edit.clone(), Some(SourceRef {
            source_id: source.id.clone(),
            version: 1,
            start_line: 1,
            end_line: 1,
        })).unwrap();

        let folders = db.list_watched_folders(&workspace.id).unwrap();
        assert_eq!(folders.len(), 1);
        assert!(folders[0].last_synced_at.is_some());

        std::fs::write(notes.join("plan.md"), "# 计划\n第二版更新内容").unwrap();
        let rerun = db
            .sync_watched_folder(&workspace.id, &folders[0].id)
            .unwrap();
        assert_eq!(rerun.updated, 1);
        let nodes = db.knowledge_list(&workspace.id).unwrap();
        assert_eq!(nodes.iter().find(|node| node.id == manual).unwrap().latest_version, 1);
        let managed = nodes.iter().find(|node| node.id == managed.id).unwrap();
        assert_eq!(managed.latest_version, 2);
        edit.node_id = Some(managed.id.clone());
        edit.expected_revision = Some(managed.revision);
        db.save_knowledge(edit).unwrap();

        std::fs::write(notes.join("plan.md"), "# 计划\n第三版更新内容").unwrap();
        assert_eq!(db.sync_watched_folder(&workspace.id, &folders[0].id).unwrap().updated, 1);
        let nodes = db.knowledge_list(&workspace.id).unwrap();
        assert_eq!(nodes.len(), 2);
        let edited = nodes.iter().find(|node| node.id == managed.id).unwrap();
        assert_eq!(edited.title, "我的引用");
        assert_eq!(edited.latest_version, 3);

        let all_reports = db.sync_workspace_watched_folders(&workspace.id).unwrap();
        assert_eq!(all_reports.len(), 1);
        assert_eq!(all_reports[0].unchanged, 1);

        let source = db.sources(&workspace.id).unwrap().remove(0);
        db.revoke_source(&workspace.id, &source.id, source.revision).unwrap();
        std::fs::write(notes.join("plan.md"), "# 计划\n停用后的文件变更").unwrap();
        assert_eq!(db.sync_watched_folder(&workspace.id, &folders[0].id).unwrap().skipped, 1);
        let disabled = db.sources(&workspace.id).unwrap().remove(0);
        assert!(!disabled.enabled);
        assert_eq!(disabled.latest_version, 3);

        let pending = read_selected(&notes.join("plan.md"), &workspace.id, &db).unwrap();
        db.remove_watched_folder(&workspace.id, &folders[0].id)
            .unwrap();
        assert!(db.import_source_with_binding(pending.clone(), Some(&folders[0].id)).is_err());
        assert!(db.list_watched_folders(&workspace.id).unwrap().is_empty());
        let reauthorized = db.import_source(pending).unwrap();
        assert!(reauthorized.enabled);
        assert_eq!(reauthorized.latest_version, 4);

        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn watched_folder_sync_resumes_in_path_order() {
        let root = std::env::temp_dir().join(format!("cortexos-watched-{}", new_id()));
        let notes = root.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        let db = Database::open(&root.join("data")).unwrap();
        let workspace = db.create_workspace("分页同步测试").unwrap();
        for n in 0..61 {
            std::fs::write(notes.join(format!("{n:02}.md")), "文件内容").unwrap();
        }
        let first = sync_folder(&notes, &workspace.id, &db).unwrap();
        assert_eq!((first.imported, first.remaining), (60, 1));
        let folder = db.list_watched_folders(&workspace.id).unwrap().remove(0);

        std::fs::write(notes.join("00-new.md"), "应在下一轮读取的内容").unwrap();
        let second = db.sync_watched_folder(&workspace.id, &folder.id).unwrap();
        assert_eq!((second.imported, second.remaining), (1, 0));
        let third = db.sync_watched_folder(&workspace.id, &folder.id).unwrap();
        assert_eq!((third.imported, third.remaining), (1, 2));

        let moved = root.join("moved");
        std::fs::create_dir(&moved).unwrap();
        sync_folder(&moved, &workspace.id, &db).unwrap();
        std::fs::remove_dir(&moved).unwrap();
        let reports = db.sync_workspace_watched_folders(&workspace.id).unwrap();
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().any(|report| report.error.is_some()));
        assert!(reports.iter().any(|report| report.error.is_none() && report.unchanged == 2));

        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }
}
