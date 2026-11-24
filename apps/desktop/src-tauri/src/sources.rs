use crate::{
    db::Database,
    models::{AppResult, new_id, now},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
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
    "txt", "md", "docx", "csv", "json", "toml", "yaml", "yml",
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h", "html", "css", "sql",
];

fn extract_docx_text(bytes: &[u8]) -> AppResult<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|_| "无法解析 .docx 文档结构。".to_string())?;
    let mut xml_bytes = Vec::new();
    {
        let doc = archive
            .by_name("word/document.xml")
            .map_err(|_| ".docx 文件缺少正文内容。".to_string())?;
        doc.take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut xml_bytes)
            .map_err(|_| "读取 .docx 正文失败。".to_string())?;
    }
    if xml_bytes.len() > 4 * 1024 * 1024 {
        return Err(".docx 解压后的正文过大，请精简后重试。".into());
    }
    let xml_str = std::str::from_utf8(&xml_bytes).map_err(|_| ".docx 正文编码无效。".to_string())?;
    let mut reader = quick_xml::Reader::from_str(xml_str);
    reader.config_mut().trim_text(false);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) => {
                let local = e.local_name();
                if local.as_ref() == "t" {
                    in_text = true;
                } else if local.as_ref() == "br" {
                    current.push('\n');
                }
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                let local = e.local_name();
                if local.as_ref() == "br" {
                    current.push('\n');
                } else if local.as_ref() == "tab" {
                    current.push('\t');
                }
            }
            Ok(quick_xml::events::Event::Text(e)) if in_text => {
                let raw = e.as_ref();
                let decoded = raw
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'");
                current.push_str(&decoded);
            }
            Ok(quick_xml::events::Event::End(e)) => {
                let local = e.local_name();
                if local.as_ref() == "t" {
                    in_text = false;
                } else if local.as_ref() == "p" {
                    let trimmed = current.trim_end();
                    if !trimmed.is_empty() {
                        paragraphs.push(trimmed.to_owned());
                    }
                    current.clear();
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => return Err(".docx 正文 XML 格式异常。".into()),
            _ => {}
        }
    }
    if !current.trim().is_empty() {
        paragraphs.push(current.trim_end().to_owned());
    }
    let joined = paragraphs.join("\n");
    if joined.trim().is_empty() {
        return Err(".docx 文档中没有可提取的文字内容。".into());
    }
    Ok(joined)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSyncReport {
    pub folder: String,
    pub imported: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: usize,
}

pub fn sync_folder(root: &Path, workspace: &str, db: &Database) -> AppResult<FolderSyncReport> {
    if !root.is_absolute() || !root.is_dir() {
        return Err("请选择有效的本地文件夹。".into());
    }
    for parent in root.ancestors() {
        let meta = std::fs::symlink_metadata(parent)
            .map_err(|_| "无法访问所选文件夹，请检查权限。".to_string())?;
        if meta.file_type().is_symlink() {
            return Err("不支持通过符号链接同步文件夹，请选择实际目录。".into());
        }
    }
    let canonical = std::fs::canonicalize(root).map_err(|_| "无法解析文件夹位置。".to_string())?;
    let mut stack = vec![canonical.clone()];
    let mut files = Vec::new();
    let mut skipped = 0usize;
    const MAX_FILES: usize = 60;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            skipped += 1;
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                skipped += 1;
                continue;
            };
            if meta.file_type().is_symlink() {
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
                    if files.len() < MAX_FILES {
                        files.push(path);
                    } else {
                        skipped += 1;
                    }
                }
            }
        }
    }
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut unchanged = 0usize;
    for path in files {
        match read_selected(&path, workspace, db) {
            Ok(pending) => {
                let prev = pending.preview.previous_version;
                match db.import_source(pending) {
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
    let report = FolderSyncReport {
        folder: canonical.to_string_lossy().into_owned(),
        imported,
        updated,
        unchanged,
        skipped,
    };
    let _ = db.upsert_watched_folder(workspace, &report.folder);
    Ok(report)
}

pub fn read_selected(path: &Path, workspace: &str, db: &Database) -> AppResult<PendingSource> {
    if !path.is_absolute() {
        return Err("请用文件选择窗口选择本机文件。".into());
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        {
            return Err("仅支持本机磁盘文件，不支持网络路径。".into());
        }
    }
    for parent in path.ancestors() {
        let meta = std::fs::symlink_metadata(parent)
            .map_err(|_| "无法读取所选文件，请检查权限。".to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(
                    "文件或所在文件夹有链接跳转（符号链接或重解析点），请从实际位置选择原文件。"
                        .into(),
                );
            }
        }
        if meta.file_type().is_symlink() {
            return Err("这个路径是符号链接，请从实际位置选择原文件。".into());
        }
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("文件名无效。")?
        .to_string();
    let lower = name.to_lowercase();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str())
        || lower.starts_with('.')
        || ["token", "credential", "secret", "password", "id_rsa"]
            .iter()
            .any(|term| lower.contains(term))
    {
        return Err(
            "支持 .txt/.md/.docx 及常见代码与配置文件；名称涉及密码、访问密钥等敏感信息的文件不能导入。".into(),
        );
    }
    let canonical =
        std::fs::canonicalize(path).map_err(|_| "无法找到所选文件的实际位置。".to_string())?;
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
    let metadata = file
        .metadata()
        .map_err(|_| "无法读取文件属性。".to_string())?;
    if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
        return Err("请选择不超过 2 MiB 的普通文本文件。".into());
    }
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
    let mut bytes = Vec::new();
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "文件读取失败。".to_string())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("文件超过 2 MiB，请缩小后重试。".into());
    }
    let raw_hash = hash(&bytes);
    let content = if extension == "docx" {
        extract_docx_text(&bytes)?
    } else {
        let text = String::from_utf8(bytes)
            .map_err(|_| "文件不是 UTF-8，请转换编码后重新选择。".to_string())?;
        text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
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
            name,
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
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
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
            if raw == pending.raw_hash && old.enabled {
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
        // Auto-sync imported document into an effective, directly searchable knowledge node.
        if kind != "reauthorized" {
            let line_count = pending.preview.content.split('\n').count().max(1) as i64;
            let content_excerpt: String = pending.preview.content.chars().take(20000).collect();
            let doc_title = pending
                .preview
                .name
                .strip_suffix(".md")
                .or_else(|| pending.preview.name.strip_suffix(".txt"))
                .unwrap_or(&pending.preview.name)
                .chars()
                .take(80)
                .collect::<String>();
            let existing_node: Option<(String, i64)> = tx
                .query_row(
                    "SELECT n.id, n.latest_version FROM knowledge_nodes n
                     JOIN knowledge_source_links l ON l.node_id=n.id AND l.knowledge_version=n.latest_version
                     WHERE n.workspace_id=?1 AND l.source_id=?2 AND l.start_line=1
                     ORDER BY n.updated_at DESC LIMIT 1",
                    params![pending.workspace, id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(error)?;
            let (node_id, node_version) = if let Some((existing_id, prev_ver)) = existing_node {
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
        }
        refresh_nodes(&tx, &id)?;
        tx.commit().map_err(error)?;
        drop(c);
        Ok(self
            .source_snapshot(&pending.workspace, &id, version)?
            .document)
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

    pub fn upsert_watched_folder(&self, workspace: &str, path: &str) -> AppResult<WatchedFolder> {
        let timestamp = now();
        let c = self.connection()?;
        let existing: Option<(String, String)> = c
            .query_row(
                "SELECT id, created_at FROM watched_folders WHERE workspace_id=?1 AND path=?2",
                params![workspace, path],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(error)?;
        let (id, created_at) = if let Some((id, created_at)) = existing {
            c.execute(
                "UPDATE watched_folders SET last_synced_at=?2 WHERE id=?1",
                params![id, timestamp],
            )
            .map_err(error)?;
            (id, created_at)
        } else {
            let id = new_id();
            c.execute(
                "INSERT INTO watched_folders(id, workspace_id, path, last_synced_at, created_at) VALUES(?1, ?2, ?3, ?4, ?4)",
                params![id, workspace, path, timestamp],
            )
            .map_err(error)?;
            (id, timestamp.clone())
        };
        Ok(WatchedFolder {
            id,
            workspace_id: workspace.to_string(),
            path: path.to_string(),
            last_synced_at: Some(timestamp),
            created_at,
        })
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
        let path: String = {
            let c = self.connection()?;
            c.query_row(
                "SELECT path FROM watched_folders WHERE workspace_id=?1 AND id=?2",
                params![workspace, folder_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?
            .ok_or_else(|| "未找到该同步文件夹绑定。".to_string())?
        };
        let dir = PathBuf::from(&path);
        if !dir.is_dir() {
            return Err("该文件夹当前不存在或已移动，请移除绑定或重新选择文件夹。".into());
        }
        sync_folder(&dir, workspace, self)
    }

    pub fn sync_workspace_watched_folders(
        &self,
        workspace: &str,
    ) -> AppResult<Vec<FolderSyncReport>> {
        let folders = self.list_watched_folders(workspace)?;
        let mut reports = Vec::new();
        for folder in folders {
            let dir = PathBuf::from(&folder.path);
            if dir.is_dir() {
                if let Ok(report) = sync_folder(&dir, workspace, self) {
                    reports.push(report);
                }
            }
        }
        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let folders = db.list_watched_folders(&workspace.id).unwrap();
        assert_eq!(folders.len(), 1);
        assert!(folders[0].last_synced_at.is_some());

        std::fs::write(notes.join("plan.md"), "# 计划\n第二版更新内容").unwrap();
        let rerun = db
            .sync_watched_folder(&workspace.id, &folders[0].id)
            .unwrap();
        assert_eq!(rerun.updated, 1);

        let all_reports = db.sync_workspace_watched_folders(&workspace.id).unwrap();
        assert_eq!(all_reports.len(), 1);
        assert_eq!(all_reports[0].unchanged, 1);

        db.remove_watched_folder(&workspace.id, &folders[0].id)
            .unwrap();
        assert!(db.list_watched_folders(&workspace.id).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
