pub(crate) mod aider_channel;
mod extract;
mod fit;
pub mod preflight;
pub mod runtime;
pub mod session;
pub mod workspace;
pub mod programming;
pub mod tasks;
#[cfg(test)]
mod tests;
use crate::{
    benchmark::{
        dataset::{hash, require_workspace},
        personalized::repository::{RepositoryFile, RepositorySnapshot},
    },
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
};
use extract::{Document, Requirement};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub snapshot_ids: Vec<String>,
    pub requirements: Vec<Requirement>,
    pub weights: Value,
    pub expected_version: u32,
    pub note: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub version: u32,
    pub body: Value,
    pub content_hash: String,
    pub created_at: String,
}

impl Database {
    fn fit_documents(&self, workspace: &str, ids: &[String]) -> AppResult<Vec<Document>> {
        if ids.is_empty()
            || ids.len() > 12
            || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err("请选择1至12份不同的项目资料。".into());
        }
        let mut docs = vec![];
        let mut total = 0;
        for id in ids {
            let s = self.repository_snapshot(workspace, id)?;
            for f in s.files.iter().filter(|f| {
                let path = f.path.to_ascii_lowercase();
                path.ends_with(".md") || path.ends_with(".txt")
            }) {
                if hash(f.content.as_bytes()) != f.content_hash {
                    return Err("项目资料内容校验失败。".into());
                }
                docs.push(Document {
                    source_id: id.clone(),
                    path: f.path.clone(),
                    content_hash: f.content_hash.clone(),
                    content: f.content.clone(),
                });
            }
            if s.profile["kind"] != "harness_document" {
                let content = serde_json::to_string_pretty(&s.profile).map_err(json_error)?;
                docs.push(Document {
                    source_id: id.clone(),
                    path: "[Repository Profile].json".into(),
                    content_hash: hash(content.as_bytes()),
                    content,
                });
            }
        }
        for d in &docs {
            total += d.content.len();
            if d.content.lines().count() > 10000
                || d.content.lines().any(|l| l.chars().count() > 4096)
            {
                return Err("资料的行数或单行过长，请拆分后导入。".into());
            }
        }
        if docs.is_empty() || docs.len() > 100 || total > 4 * 1024 * 1024 {
            return Err("本次分析支持最多100份文档、合计4 MiB；请选择较小的快照。".into());
        }
        Ok(docs)
    }
    fn fit_body(&self, workspace: &str, input: &Input) -> AppResult<Value> {
        let docs = self.fit_documents(workspace, &input.snapshot_ids)?;
        let originals = extract::extract(&docs);
        if originals.len() > 800 {
            return Err("候选要求超过800条，请缩小资料范围后分次分析。".into());
        }
        let requirements = if input.requirements.is_empty() {
            originals.clone()
        } else {
            if input.requirements.len() != originals.len() {
                return Err("需求清单已变化，请重新分析资料。".into());
            }
            let mut ids = BTreeSet::new();
            for r in &input.requirements {
                let original = originals
                    .iter()
                    .find(|o| o.id == r.id)
                    .ok_or("需求来源不在当前资料中。")?;
                if !ids.insert(&r.id)
                    || r.category != original.category
                    || r.original_value != original.original_value
                    || r.confidence != original.confidence
                    || serde_json::to_value(&r.evidence).map_err(json_error)?
                        != serde_json::to_value(&original.evidence).map_err(json_error)?
                    || !matches!(
                        r.status.as_str(),
                        "candidate" | "unknown" | "confirmed" | "edited" | "rejected"
                    )
                    || r.value.chars().count() > 4096
                    || r.criterion
                        .as_ref()
                        .is_some_and(|k| extract::criteria().get(k).is_none())
                    || (r.hard && !matches!(r.status.as_str(), "confirmed" | "edited"))
                    || (matches!(r.status.as_str(), "confirmed" | "edited")
                        && r.value.trim().is_empty())
                    || (r.value != original.value && r.status != "edited" && r.status != "rejected")
                {
                    return Err("需求内容、来源或确认状态无效；修改内容后请选择“用户修改”。".into());
                }
            }
            input.requirements.clone()
        };
        if input.note.chars().count() > 1000
            || input.weights.as_object().is_none_or(|m| {
                m.len() != 4
                    || ["compatibility", "automation", "governance", "context"]
                        .iter()
                        .any(|k| m.get(*k).and_then(Value::as_u64).is_none_or(|v| v > 5))
            })
            || input
                .weights
                .as_object()
                .is_none_or(|m| m.values().all(|v| v.as_u64() == Some(0)))
        {
            return Err("请设置0至5的四类权重（至少一项非0），修改说明不超过1000字。".into());
        }
        let catalog = fit::catalog();
        let report = fit::assess(&requirements, &input.weights, &catalog);
        Ok(
            json!({"snapshotIds":input.snapshot_ids,"documents":docs.iter().map(|d|json!({"sourceId":d.source_id,"path":d.path,"contentHash":d.content_hash})).collect::<Vec<_>>(),
   "requirements":requirements,"weights":input.weights,"note":input.note,"catalogHash":hash(catalog.to_string().as_bytes()),"catalog":catalog,"report":report,
   "confidencePolicy":"literal_candidates_require_review; edited_values_are_user_statements; no_runtime_validation"}),
        )
    }
    pub fn fit_read(&self, workspace: &str, version: Option<u32>) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let selected:Option<(u32,String,String,String)>=c.query_row("SELECT version,body_json,content_hash,created_at FROM harness_fit_assessments WHERE workspace_id=?1 AND (?2 IS NULL OR version=?2) ORDER BY version DESC LIMIT 1",params![workspace,version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(error)?;
        if version.is_some() && selected.is_none() {
            return Err("此工作区没有该分析版本。".into());
        }
        let selected = selected
            .map(
                |(version, raw, content_hash, created_at)| -> AppResult<Assessment> {
                    if hash(raw.as_bytes()) != content_hash {
                        return Err("分析版本校验失败，请保留数据。".into());
                    }
                    Ok(Assessment {
                        version,
                        body: serde_json::from_str(&raw).map_err(json_error)?,
                        content_hash,
                        created_at,
                    })
                },
            )
            .transpose()?;
        let mut q=c.prepare("SELECT version,content_hash,created_at FROM harness_fit_assessments WHERE workspace_id=?1 ORDER BY version DESC LIMIT 100").map_err(error)?;
        let history=q.query_map([workspace],|r|Ok(json!({"version":r.get::<_,u32>(0)?,"contentHash":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        Ok(
            json!({"selected":selected,"history":history,"snapshots":self.repository_summaries(workspace)?,"catalog":fit::catalog(),"categories":extract::CATEGORIES.iter().map(|(id,label)|json!({"id":id,"label":label})).collect::<Vec<_>>(),"criteria":extract::criteria()}),
        )
    }
    pub fn fit_save(&self, workspace: &str, input: Input) -> AppResult<Assessment> {
        let mut c = self.connection()?;
        require_workspace(&c, workspace)?;
        // Source snapshots are immutable, so validation can finish before the write transaction.
        drop(c);
        let mut body = self.fit_body(workspace, &input)?;
        c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let current:u32=tx.query_row("SELECT COALESCE(MAX(version),0) FROM harness_fit_assessments WHERE workspace_id=?1",[workspace],|r|r.get(0)).map_err(error)?;
        if current != input.expected_version {
            return Err("分析已在其他窗口更新。草稿已保留，请重新读取最新版本后修改。".into());
        }
        let previous:Option<String>=tx.query_row("SELECT content_hash FROM harness_fit_assessments WHERE workspace_id=?1 AND version=?2",params![workspace,current],|r|r.get(0)).optional().map_err(error)?;
        body["previousContentHash"] = json!(previous);
        let raw = body.to_string();
        let content_hash = hash(raw.as_bytes());
        let version = current.checked_add(1).ok_or("版本数量超出限制。")?;
        let created_at = now();
        tx.execute(
            "INSERT INTO harness_fit_assessments VALUES(?1,?2,?3,?4,?5)",
            params![workspace, version, raw, content_hash, created_at],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(Assessment {
            version,
            body,
            content_hash,
            created_at,
        })
    }
    pub fn fit_import(
        &self,
        workspace: &str,
        path: &std::path::Path,
    ) -> AppResult<RepositorySnapshot> {
        {
            let c = self.connection()?;
            require_workspace(&c, workspace)?;
        }
        let pending = crate::sources::read_selected(path, workspace, self)?;
        let p = pending.preview;
        let file = RepositoryFile {
            path: p.name.clone(),
            language: "文档".into(),
            content: p.content,
            content_hash: p.content_hash.clone(),
            symbols: vec![],
        };
        self.save_repository_snapshot(
            workspace,
            RepositorySnapshot {
                id: new_id(),
                name: p.name,
                content_hash: p.content_hash,
                profile: json!({"kind":"harness_document","execution":"none","cloudUpload":false}),
                files: vec![file],
                created_at: now(),
            },
        )
    }
}

#[tauri::command]
pub async fn get_harness_fit(
    state: State<'_, AppState>,
    workspace_id: String,
    version: Option<u32>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.fit_read(&workspace_id, version)
    })
    .await
}
#[tauri::command]
pub async fn analyze_harness_fit(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.fit_body(&workspace_id, &input)
    })
    .await
}
#[tauri::command]
pub async fn save_harness_fit(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
) -> AppResult<Assessment> {
    blocking(state.database.clone()?, move |db| {
        db.fit_save(&workspace_id, input)
    })
    .await
}
#[tauri::command]
pub async fn get_harness_document(
    state: State<'_, AppState>,
    workspace_id: String,
    snapshot_id: String,
) -> AppResult<Vec<Document>> {
    blocking(state.database.clone()?, move |db| {
        db.fit_documents(&workspace_id, &[snapshot_id])
    })
    .await
}
#[tauri::command]
pub async fn import_harness_document(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Option<Value>> {
    let db = state.database.clone()?;
    {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
    }
    let path =
        tauri::async_runtime::spawn_blocking(move || -> AppResult<Option<std::path::PathBuf>> {
            #[cfg(debug_assertions)]
            if let Ok(path) = std::env::var("CORTEXOS_TEST_SOURCE_PATH") {
                return Ok(Some(path.into()));
            }
            app.dialog()
                .file()
                .set_title("选择项目规范文档")
                .add_filter("UTF-8项目文档", &["md", "txt"])
                .blocking_pick_file()
                .map(|p| p.into_path().map_err(|_| "仅支持本机文档。".into()))
                .transpose()
        })
        .await
        .map_err(|_| "无法打开文件选择窗口。")??;
    let Some(path) = path else {
        return Ok(None);
    };
    blocking(db, move |db| {
        let s = db.fit_import(&workspace_id, &path)?;
        Ok(Some(
            json!({"id":s.id,"name":s.name,"contentHash":s.content_hash}),
        ))
    })
    .await
}
