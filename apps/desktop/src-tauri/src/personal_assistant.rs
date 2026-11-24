use crate::{
    benchmark::dataset::require_workspace,
    commands::{AppState, blocking},
    db::error,
    models::AppResult,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Serialize;
use tauri::State;

#[derive(Default, Serialize)]
struct AssistantDraft {
    name: String,
    instructions: String,
}

impl AssistantDraft {
    fn validate(name: String, instructions: String, publish: bool) -> AppResult<Self> {
        let name = name.trim();
        if name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("助手名称须为最多 80 个字符的单行文本。".into());
        }
        if instructions.chars().count() > 2000
            || instructions
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\n' | '\r'))
        {
            return Err("助手指令须为最多 2000 个字符的文本。".into());
        }
        if publish && (name.is_empty() || instructions.trim().is_empty()) {
            return Err("发布前请填写助手名称和指令。".into());
        }
        Ok(Self {
            name: name.to_owned(),
            instructions,
        })
    }

    fn save(&self, conn: &Connection, workspace_id: &str) -> AppResult<()> {
        conn.execute(
            "INSERT INTO assistant_drafts(workspace_id,name,instructions) VALUES (?1,?2,?3)
             ON CONFLICT(workspace_id) DO UPDATE SET name=excluded.name,instructions=excluded.instructions",
            params![workspace_id, self.name, self.instructions],
        ).map_err(error)?;
        Ok(())
    }
}

#[derive(Serialize)]
pub(crate) struct AssistantVersion {
    pub version: i64,
    pub name: String,
    pub instructions: String,
}

impl AssistantVersion {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            version: row.get(0)?,
            name: row.get(1)?,
            instructions: row.get(2)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantStatus {
    draft: AssistantDraft,
    active_version: Option<i64>,
    versions: Vec<AssistantVersion>,
}

pub(crate) fn active_version(
    conn: &Connection,
    workspace_id: &str,
) -> AppResult<Option<AssistantVersion>> {
    conn.query_row(
        "SELECT v.version,v.name,v.instructions FROM assistant_drafts d
         JOIN assistant_versions v ON v.workspace_id=d.workspace_id AND v.version=d.active_version
         WHERE d.workspace_id=?1",
        [workspace_id],
        AssistantVersion::read,
    )
    .optional()
    .map_err(error)
}

#[tauri::command]
pub(crate) async fn read_assistant(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<AssistantStatus> {
    blocking(state.database.clone()?, move |db| {
        let mut conn = db.connection()?;
        let tx = conn.transaction().map_err(error)?;
        require_workspace(&tx, &workspace_id)?;
        let (draft, active_version) = tx.query_row(
            "SELECT name,instructions,active_version FROM assistant_drafts WHERE workspace_id=?1",
            [&workspace_id],
            |row| Ok((AssistantDraft { name: row.get(0)?, instructions: row.get(1)? }, row.get(2)?)),
        ).optional().map_err(error)?.unwrap_or_default();
        let mut statement = tx.prepare(
            "SELECT version,name,instructions FROM assistant_versions
             WHERE workspace_id=?1 ORDER BY version DESC",
        ).map_err(error)?;
        let versions = statement.query_map([&workspace_id], AssistantVersion::read)
            .map_err(error)?.collect::<rusqlite::Result<_>>().map_err(error)?;
        Ok(AssistantStatus { draft, active_version, versions })
    }).await
}

#[tauri::command]
pub(crate) async fn save_assistant_draft(
    state: State<'_, AppState>,
    workspace_id: String,
    name: String,
    instructions: String,
) -> AppResult<()> {
    let draft = AssistantDraft::validate(name, instructions, false)?;
    blocking(state.database.clone()?, move |db| {
        let conn = db.connection()?;
        require_workspace(&conn, &workspace_id)?;
        draft.save(&conn, &workspace_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn publish_assistant(
    state: State<'_, AppState>,
    workspace_id: String,
    name: String,
    instructions: String,
) -> AppResult<()> {
    let draft = AssistantDraft::validate(name, instructions, true)?;
    blocking(state.database.clone()?, move |db| {
        let mut conn = db.connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
        require_workspace(&tx, &workspace_id)?;
        let version: i64 = tx.query_row(
            "INSERT INTO assistant_versions(workspace_id,version,name,instructions)
             VALUES (?1,(SELECT COALESCE(MAX(version),0)+1 FROM assistant_versions WHERE workspace_id=?1),?2,?3)
             RETURNING version",
            params![workspace_id, draft.name, draft.instructions],
            |row| row.get(0),
        ).map_err(error)?;
        draft.save(&tx, &workspace_id)?;
        tx.execute(
            "UPDATE assistant_drafts SET active_version=?2 WHERE workspace_id=?1",
            params![workspace_id, version],
        ).map_err(error)?;
        tx.commit().map_err(error)
    }).await
}

#[tauri::command]
pub(crate) async fn activate_assistant(
    state: State<'_, AppState>,
    workspace_id: String,
    version: Option<i64>,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        let conn = db.connection()?;
        require_workspace(&conn, &workspace_id)?;
        let updated = conn
            .execute(
                "UPDATE assistant_drafts SET active_version=?2 WHERE workspace_id=?1
               AND (?2 IS NULL OR EXISTS(
                   SELECT 1 FROM assistant_versions WHERE workspace_id=?1 AND version=?2
               ))",
                params![workspace_id, version],
            )
            .map_err(error)?;
        if version.is_some() && updated == 0 {
            return Err("当前工作区没有此助手版本。".into());
        }
        Ok(())
    })
    .await
}
