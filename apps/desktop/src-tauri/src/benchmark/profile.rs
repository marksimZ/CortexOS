use crate::{
    cloud::CloudState,
    commands::AppState,
    db::{Database, error, json_error},
    models::{AppResult, now},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tauri::State;

pub const DOMAINS: &[&str] = &[
    "software",
    "education",
    "finance",
    "data",
    "science",
    "writing",
    "product",
];
pub const TASKS: &[&str] = &[
    "coding",
    "debugging",
    "review",
    "math",
    "reading",
    "analysis",
    "research",
    "qa",
    "automation",
];
pub const WEIGHTS: &[&str] = &[
    "quality",
    "speed",
    "cost",
    "resources",
    "privacy",
    "coding",
    "context",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub domains: Vec<String>,
    pub other_domain: String,
    pub level: Option<String>,
    pub tasks: Vec<String>,
    pub task_notes: String,
    pub tools: Vec<String>,
    pub weights: BTreeMap<String, u8>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            domains: vec![],
            other_domain: String::new(),
            level: None,
            tasks: vec![],
            task_notes: String::new(),
            tools: vec![],
            weights: WEIGHTS.iter().map(|k| (k.to_string(), 3)).collect(),
        }
    }
}
impl Profile {
    fn validate(&self) -> AppResult<()> {
        fn unique(values: &[String], max: usize) -> bool {
            values.len() <= max
                && values
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == values.len()
        }
        fn text(value: &str, max: usize) -> bool {
            value.chars().count() <= max
                && !value
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
        }
        if !unique(&self.domains, DOMAINS.len())
            || self.domains.iter().any(|v| !DOMAINS.contains(&v.as_str()))
            || !unique(&self.tasks, TASKS.len())
            || self.tasks.iter().any(|v| !TASKS.contains(&v.as_str()))
            || !unique(&self.tools, 20)
            || self
                .tools
                .iter()
                .any(|v| v.trim().is_empty() || !text(v, 60))
            || !text(&self.other_domain, 120)
            || !text(&self.task_notes, 2000)
            || self
                .level
                .as_ref()
                .is_some_and(|v| !["beginner", "intermediate", "advanced"].contains(&v.as_str()))
            || self.weights.len() != WEIGHTS.len()
            || WEIGHTS.iter().any(|k| !self.weights.contains_key(*k))
            || self.weights.values().any(|v| *v > 5)
            || self.weights.values().all(|v| *v == 0)
        {
            return Err("请检查使用偏好：选项需有效，工具最多 20 个，至少一项偏好大于 0。".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveProfile {
    pub expected_version: u32,
    pub action: String,
    pub profile: Profile,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileVersion {
    pub workspace_id: String,
    pub version: u32,
    pub action: String,
    pub profile: Profile,
    pub content_hash: String,
    pub created_at: String,
}
fn workspace_exists(c: &rusqlite::Connection, workspace: &str) -> AppResult<()> {
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
            [workspace],
            |r| r.get(0),
        )
        .map_err(error)?;
    if exists {
        Ok(())
    } else {
        Err("工作区不存在，请重新选择。".into())
    }
}
impl Database {
    pub fn benchmark_profile(&self, workspace: &str) -> AppResult<Option<ProfileVersion>> {
        let c = self.connection()?;
        workspace_exists(&c, workspace)?;
        let row: Option<(u32,String,String,String,String)>=c.query_row("SELECT version,action,profile_json,content_hash,created_at FROM benchmark_profile_versions WHERE workspace_id=?1 ORDER BY version DESC LIMIT 1",[workspace],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(error)?;
        row.map(|(version, action, raw, content_hash, created_at)| {
            let profile: Profile = serde_json::from_str(&raw).map_err(json_error)?;
            profile.validate()?;
            if format!("{:x}", Sha256::digest(raw.as_bytes())) != content_hash {
                return Err("使用偏好记录校验失败，请保留本地数据。".into());
            }
            Ok(ProfileVersion {
                workspace_id: workspace.into(),
                version,
                action,
                profile,
                content_hash,
                created_at,
            })
        })
        .transpose()
    }
    pub fn save_benchmark_profile(
        &self,
        workspace: &str,
        input: SaveProfile,
    ) -> AppResult<ProfileVersion> {
        if !["save", "skip", "reset"].contains(&input.action.as_str()) {
            return Err("不支持的保存操作。".into());
        }
        let profile = if input.action == "save" {
            input.profile
        } else {
            Profile::default()
        };
        profile.validate()?;
        let raw = serde_json::to_string(&profile).map_err(json_error)?;
        let hash = format!("{:x}", Sha256::digest(raw.as_bytes()));
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        workspace_exists(&tx, workspace)?;
        let current:u32=tx.query_row("SELECT COALESCE(MAX(version),0) FROM benchmark_profile_versions WHERE workspace_id=?1",[workspace],|r|r.get(0)).map_err(error)?;
        if current != input.expected_version {
            return Err("使用偏好已在其他窗口更新。请先复制未保存内容，再重新读取后修改。".into());
        }
        let version = current.checked_add(1).ok_or("版本数量超出限制。")?;
        let created_at = now();
        tx.execute(
            "INSERT INTO benchmark_profile_versions VALUES(?1,?2,?3,?4,?5,?6)",
            params![workspace, version, input.action, raw, hash, created_at],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(ProfileVersion {
            workspace_id: workspace.into(),
            version,
            action: input.action,
            profile,
            content_hash: hash,
            created_at,
        })
    }
}
#[tauri::command]
pub fn get_benchmark_profile(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
) -> AppResult<Option<ProfileVersion>> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .benchmark_profile(&workspace_id)
    })
}
#[tauri::command]
pub fn save_benchmark_profile(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    input: SaveProfile,
) -> AppResult<ProfileVersion> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .save_benchmark_profile(&workspace_id, input)
    })
}
