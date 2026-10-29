use super::super::dataset::hash;
use crate::{
    cloud::CloudState,
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path},
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

const FILE_LIMIT: u64 = 512 * 1024;
const TOTAL_LIMIT: usize = 20 * 1024 * 1024;
const COUNT_LIMIT: usize = 2000;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFile {
    pub path: String,
    pub language: String,
    pub content: String,
    pub content_hash: String,
    pub symbols: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySnapshot {
    pub id: String,
    pub name: String,
    pub content_hash: String,
    pub profile: Value,
    pub files: Vec<RepositoryFile>,
    pub created_at: String,
}

fn excluded(path: &str) -> bool {
    path.split('/').any(|p| {
        let p = p.to_ascii_lowercase();
        p.starts_with('.')
            || [
                "node_modules",
                "target",
                "dist",
                "build",
                "vendor",
                "coverage",
                "__pycache__",
                "venv",
            ]
            .contains(&p.as_str())
            || [
                "token",
                "secret",
                "credential",
                "password",
                "id_rsa",
                "private_key",
            ]
            .iter()
            .any(|x| p.contains(x))
            || ["pem", "key", "p12", "pfx", "keystore"]
                .iter()
                .any(|x| p.ends_with(&format!(".{x}")))
    })
}
fn language(path: &str) -> Option<&'static str> {
    Some(
        match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
            "rs" => "Rust",
            "ts" | "tsx" => "TypeScript",
            "js" | "jsx" | "mjs" => "JavaScript",
            "py" => "Python",
            "sql" => "SQL",
            "go" => "Go",
            "java" => "Java",
            "c" | "h" | "cpp" => "C/C++",
            "md" | "txt" | "rst" => "文档",
            "json" | "toml" | "yaml" | "yml" => "配置",
            _ => return None,
        },
    )
}
fn no_link(path: &Path) -> AppResult<()> {
    for ancestor in path.ancestors() {
        let m =
            std::fs::symlink_metadata(ancestor).map_err(|_| "无法检查所选路径，请确认权限。")?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("所选路径含链接跳转，请选择实际文件夹或文件。".into());
            }
        }
        if m.file_type().is_symlink() {
            return Err("不读取符号链接。".into());
        }
    }
    Ok(())
}
fn open_checked(path: &Path, root: &Path) -> AppResult<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000).share_mode(1);
    }
    let file = options.open(path).map_err(|_| "文件不可读或正在被修改。")?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
        let mut buffer = vec![0u16; 32768];
        let len = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                0,
            )
        };
        if len == 0 || len as usize >= buffer.len() {
            return Err("无法验证打开文件的实际路径。".into());
        }
        let actual = std::path::PathBuf::from(
            String::from_utf16(&buffer[..len as usize]).map_err(|_| "文件路径无效。")?,
        );
        if !actual.starts_with(root) {
            return Err("打开的文件位于授权目录外。".into());
        }
        use std::os::windows::fs::MetadataExt;
        if file
            .metadata()
            .map_err(|_| "文件属性不可读。")?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err("不读取链接文件。".into());
        }
    }
    #[cfg(not(windows))]
    if !path
        .canonicalize()
        .map_err(|_| "路径不可读。")?
        .starts_with(root)
    {
        return Err("路径越界。".into());
    }
    Ok(file)
}
fn zip_preflight(file: &mut std::fs::File) -> AppResult<()> {
    use std::io::{Seek, SeekFrom};
    let len = file.metadata().map_err(|_| "ZIP不可读。")?.len();
    if len > 50 * 1024 * 1024 || len < 22 {
        return Err("ZIP大小不受支持。".into());
    }
    let size = len.min(65557) as usize;
    let mut tail = vec![0; size];
    file.seek(SeekFrom::End(-(size as i64)))
        .map_err(|_| "ZIP不可读。")?;
    file.read_exact(&mut tail).map_err(|_| "ZIP不可读。")?;
    let position = (0..size.saturating_sub(21))
        .rev()
        .find(|&i| {
            tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]
                && i + 22 + u16::from_le_bytes([tail[i + 20], tail[i + 21]]) as usize == size
        })
        .ok_or("ZIP目录记录无效。")?;
    let n = u16::from_le_bytes([tail[position + 10], tail[position + 11]]) as usize;
    if (position >= 20 && tail[position - 20..position - 16] == [0x50, 0x4b, 0x06, 0x07])
        || tail[position + 8..position + 10] != tail[position + 10..position + 12]
        || tail[position + 12..position + 16] == [255; 4]
        || tail[position + 16..position + 20] == [255; 4]
        || u32::from_le_bytes(tail[position + 12..position + 16].try_into().unwrap())
            > 4 * 1024 * 1024
    {
        return Err("ZIP64、分卷或过大的目录记录不受支持。".into());
    }
    if n == 0 || n > COUNT_LIMIT || tail[position + 4..position + 8] != [0, 0, 0, 0] {
        return Err("不支持分卷、ZIP64或超过2000条目的ZIP。".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|_| "ZIP不可读。")?;
    Ok(())
}
fn safe_relative(raw: &str) -> AppResult<String> {
    if raw.is_empty() || raw.len() > 512 || raw.contains(['\\', ':', '\0']) || raw.starts_with('/')
    {
        return Err("ZIP 中含不安全路径。".into());
    }
    if Path::new(raw)
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
        || raw
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.ends_with([' ', '.']))
    {
        return Err("ZIP 中含越界或歧义路径。".into());
    }
    Ok(raw.into())
}
fn secret_text(text: &str) -> bool {
    text.contains("-----BEGIN ")
        || text.lines().any(|line| {
            let low = line.to_ascii_lowercase();
            [
                "api_key",
                "api-key",
                "access_token",
                "client_secret",
                "authorization:",
                "password=",
            ]
            .iter()
            .any(|s| low.contains(s))
                && (line.contains('=') || line.contains(':'))
        })
}
fn add(
    files: &mut Vec<RepositoryFile>,
    skipped: &mut BTreeMap<String, u32>,
    path: String,
    bytes: Vec<u8>,
    total: &mut usize,
) -> AppResult<()> {
    let Some(lang) = language(&path) else {
        *skipped.entry("不支持的文件类型".into()).or_default() += 1;
        return Ok(());
    };
    if excluded(&path) {
        *skipped.entry("敏感名称或忽略目录".into()).or_default() += 1;
        return Ok(());
    }
    let Ok(content) = String::from_utf8(bytes) else {
        *skipped.entry("非 UTF-8 文本".into()).or_default() += 1;
        return Ok(());
    };
    if content.contains('\0') || secret_text(&content) {
        *skipped.entry("二进制或疑似凭据内容".into()).or_default() += 1;
        return Ok(());
    }
    *total += content.len();
    if *total > TOTAL_LIMIT {
        return Err("项目文本超过20MB，请选择更小的项目或子文件夹。".into());
    }
    let symbols=content.lines().enumerate().filter_map(|(i,l)| {
        let l=l.trim();
        ["def ","async def ","fn ","pub fn ","pub async fn ","export function ","function ","class ","struct "].iter().find_map(|prefix|l.strip_prefix(prefix)).map(|s|json!({"line":i+1,"name":s.split(|c:char|!c.is_alphanumeric()&&c!='_').next().unwrap_or(""),"method":"line_heuristic"}))
    }).take(200).collect();
    files.push(RepositoryFile {
        path,
        language: lang.into(),
        content_hash: hash(content.as_bytes()),
        content,
        symbols,
    });
    Ok(())
}
pub fn scan(path: &Path, is_zip: bool) -> AppResult<RepositorySnapshot> {
    if !path.is_absolute() {
        return Err("请通过系统选择窗口选择项目。".into());
    }
    no_link(path)?;
    #[cfg(windows)]
    {
        use std::path::Prefix;
        if !matches!(path.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
        {
            return Err("只支持本机磁盘项目。".into());
        }
    }
    let mut files = vec![];
    let mut skipped = BTreeMap::new();
    let mut total = 0;
    let mut visited = 0;
    if is_zip {
        if std::fs::metadata(path).map_err(|_| "无法读取ZIP。")?.len() > 50 * 1024 * 1024 {
            return Err("ZIP 超过50MB。".into());
        }
        let parent = path
            .parent()
            .ok_or("ZIP路径无效。")?
            .canonicalize()
            .map_err(|_| "ZIP位置不可读。")?;
        let mut file = open_checked(path, &parent)?;
        zip_preflight(&mut file)?;
        let mut zip = zip::ZipArchive::new(file).map_err(|_| "ZIP格式无效。")?;
        if zip.len() > COUNT_LIMIT {
            return Err("ZIP条目超过2000个。".into());
        }
        let mut names = BTreeSet::new();
        let mut expanded = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|_| "ZIP条目不可读或已加密。")?;
            let name = entry.name().trim_end_matches('/');
            let relative = safe_relative(name)?;
            if !names.insert(relative.to_ascii_lowercase()) {
                return Err("ZIP含重复或大小写冲突路径。".into());
            }
            if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("ZIP含符号链接。".into());
            }
            if entry.is_dir() {
                continue;
            }
            expanded = expanded.checked_add(entry.size()).ok_or("ZIP大小无效。")?;
            if expanded > TOTAL_LIMIT as u64
                || (entry.size() > 1024 * 1024
                    && entry.size() > entry.compressed_size().saturating_mul(100))
            {
                return Err("ZIP展开体积或压缩比超过限制。".into());
            }
            if excluded(&relative) || language(&relative).is_none() {
                *skipped
                    .entry("忽略目录、敏感名称或文件类型".into())
                    .or_default() += 1;
                continue;
            }
            if entry.size() > FILE_LIMIT {
                *skipped.entry("单文件超过512KB".into()).or_default() += 1;
                continue;
            }
            let mut bytes = vec![];
            entry
                .by_ref()
                .take(FILE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "ZIP内容损坏。")?;
            if bytes.len() > FILE_LIMIT as usize {
                return Err("ZIP实际内容超过声明限制。".into());
            }
            add(&mut files, &mut skipped, relative, bytes, &mut total)?;
        }
    } else {
        if !path.is_dir() {
            return Err("请选择文件夹。".into());
        }
        let root = path.canonicalize().map_err(|_| "无法定位所选文件夹。")?;
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            let mut entries = std::fs::read_dir(&dir)
                .map_err(|_| "项目目录不可读，请检查权限。")?
                .take(COUNT_LIMIT + 1)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "读取目录条目失败。")?;
            entries.sort_by_key(|e| e.file_name());
            if entries.len() > COUNT_LIMIT {
                return Err("目录条目超过2000个。".into());
            }
            for entry in entries {
                visited += 1;
                if visited > COUNT_LIMIT {
                    return Err("项目条目超过2000个，请选择更小的子文件夹。".into());
                }
                let p = entry.path();
                let rel = p
                    .strip_prefix(&root)
                    .map_err(|_| "路径越界。")?
                    .to_str()
                    .ok_or("文件路径不是UTF-8。")?
                    .replace('\\', "/");
                safe_relative(&rel)?;
                if rel.split('/').count() > 32 {
                    return Err("项目目录层级超过32层。".into());
                }
                if excluded(&rel) {
                    *skipped.entry("敏感名称或忽略目录".into()).or_default() += 1;
                    continue;
                }
                if no_link(&p).is_err() {
                    *skipped.entry("链接跳转".into()).or_default() += 1;
                    continue;
                }
                let meta = entry.metadata().map_err(|_| "文件属性不可读。")?;
                if meta.is_dir() {
                    pending.push(p);
                    continue;
                }
                if !meta.is_file() {
                    continue;
                }
                if meta.len() > FILE_LIMIT {
                    *skipped.entry("单文件超过512KB".into()).or_default() += 1;
                    continue;
                }
                if language(&rel).is_none() {
                    *skipped.entry("不支持的文件类型".into()).or_default() += 1;
                    continue;
                }
                let mut bytes = vec![];
                open_checked(&p, &root)?
                    .take(FILE_LIMIT + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "文件读取失败。")?;
                if bytes.len() > FILE_LIMIT as usize {
                    return Err("扫描时文件大小发生变化，请重试。".into());
                }
                no_link(&p)?;
                if !p
                    .canonicalize()
                    .map_err(|_| "文件已移动。")?
                    .starts_with(&root)
                {
                    return Err("文件离开已授权目录。".into());
                }
                add(&mut files, &mut skipped, rel, bytes, &mut total)?;
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if files.is_empty() {
        return Err("没有可分析的安全文本文件。".into());
    }
    let mut languages = BTreeMap::<String, usize>::new();
    let mut modules = BTreeSet::new();
    let mut dependencies = BTreeSet::new();
    let mut tests = vec![];
    for f in &files {
        *languages.entry(f.language.clone()).or_default() += 1;
        modules.insert(f.path.split('/').next().unwrap_or("").to_string());
        if f.path.ends_with("package.json")
            && let Ok(v) = serde_json::from_str::<Value>(&f.content)
        {
            for key in ["dependencies", "devDependencies"] {
                if let Some(o) = v[key].as_object() {
                    dependencies.extend(o.keys().take(200).cloned());
                }
            }
        }
        if f.path.contains("test") || f.path.contains("spec") {
            tests.push(f.path.clone());
        }
    }
    let hints = |needles: &[&str]| {
        files
            .iter()
            .flat_map(|f| {
                f.content
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| needles.iter().any(|n| line.contains(n)))
                    .take(10)
                    .map(|(i, _)| json!({"path":f.path,"line":i+1}))
            })
            .take(100)
            .collect::<Vec<_>>()
    };
    let profile = json!({"languages":languages,"modules":modules,"dependencies":dependencies,"testFiles":tests,"fileCount":files.len(),"textBytes":total,"skipped":skipped,"analysisMethod":"bounded_static_heuristic","execution":"none","cloudUpload":false,"apiCandidates":hints(&["@app.route","@app.get","router.get(",".route(","#[tauri::command]"]),"databaseCandidates":hints(&["SELECT ","CREATE TABLE ","rusqlite","sqlx::","prisma."]),"testFrameworkHints":hints(&["pytest","unittest","vitest","jest","#[test]","#[tokio::test]"]),"candidateAreas":files.iter().filter(|f|!f.symbols.is_empty()).take(40).map(|f|json!({"path":f.path,"tasks":["项目理解","函数边界检查"],"symbols":f.symbols})).collect::<Vec<_>>()});
    let fingerprint = json!(
        files
            .iter()
            .map(|f| json!({"path":f.path,"hash":f.content_hash}))
            .collect::<Vec<_>>()
    );
    Ok(RepositorySnapshot {
        id: new_id(),
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("项目")
            .into(),
        content_hash: hash(fingerprint.to_string().as_bytes()),
        profile,
        files,
        created_at: now(),
    })
}
impl Database {
    pub fn save_repository_snapshot(
        &self,
        workspace: &str,
        snapshot: RepositorySnapshot,
    ) -> AppResult<RepositorySnapshot> {
        let c = self.connection()?;
        super::super::dataset::require_workspace(&c, workspace)?;
        c.execute(
            "INSERT INTO benchmark_repository_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                snapshot.id,
                workspace,
                snapshot.name,
                snapshot.content_hash,
                snapshot.profile.to_string(),
                serde_json::to_string(&snapshot.files).map_err(json_error)?,
                snapshot.created_at
            ],
        )
        .map_err(error)?;
        Ok(snapshot)
    }
    pub fn repository_snapshot(&self, workspace: &str, id: &str) -> AppResult<RepositorySnapshot> {
        self.connection()?.query_row("SELECT id,name,content_hash,profile_json,files_json,created_at FROM benchmark_repository_snapshots WHERE workspace_id=?1 AND id=?2",params![workspace,id],|r|{let profile:String=r.get(3)?;let files:String=r.get(4)?;Ok(RepositorySnapshot{id:r.get(0)?,name:r.get(1)?,content_hash:r.get(2)?,profile:serde_json::from_str(&profile).map_err(|_|rusqlite::Error::InvalidQuery)?,files:serde_json::from_str(&files).map_err(|_|rusqlite::Error::InvalidQuery)?,created_at:r.get(5)?})}).map_err(error)
    }
    pub fn repository_summaries(&self, workspace: &str) -> AppResult<Vec<Value>> {
        let c = self.connection()?;
        super::super::dataset::require_workspace(&c, workspace)?;
        let mut s=c.prepare("SELECT id,name,content_hash,profile_json,created_at FROM benchmark_repository_snapshots WHERE workspace_id=?1 ORDER BY created_at DESC LIMIT 50").map_err(error)?;
        s.query_map([workspace],|r|{let raw:String=r.get(3)?;Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"contentHash":r.get::<_,String>(2)?,"profile":serde_json::from_str::<Value>(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?,"createdAt":r.get::<_,String>(4)?}))}).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)
    }
}
#[tauri::command]
pub async fn import_benchmark_repository(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    kind: String,
) -> AppResult<Option<RepositorySnapshot>> {
    cloud.guard()?;
    if !["folder", "zip"].contains(&kind.as_str()) {
        return Err("不支持的项目来源。".into());
    }
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        if kind == "folder" {
            app.dialog()
                .file()
                .blocking_pick_folder()
                .map(|p| (p, false))
        } else {
            app.dialog()
                .file()
                .add_filter("ZIP项目", &["zip"])
                .blocking_pick_file()
                .map(|p| (p, true))
        }
    })
    .await
    .map_err(|_| "选择项目窗口异常。")?;
    let Some((chosen, is_zip)) = chosen else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|_| "只支持本地项目。")?;
    let db = state.database.clone()?;
    let snapshot = tauri::async_runtime::spawn_blocking(move || scan(&path, is_zip))
        .await
        .map_err(|_| "本地项目分析异常。")??;
    cloud.guard()?;
    blocking(db, move |db| {
        db.save_repository_snapshot(&workspace_id, snapshot)
    })
    .await
    .map(Some)
}
#[tauri::command]
pub fn list_benchmark_repositories(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
) -> AppResult<Vec<Value>> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .repository_summaries(&workspace_id)
    })
}
#[tauri::command]
pub fn get_benchmark_repository(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    snapshot_id: String,
) -> AppResult<RepositorySnapshot> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .repository_snapshot(&workspace_id, &snapshot_id)
    })
}
