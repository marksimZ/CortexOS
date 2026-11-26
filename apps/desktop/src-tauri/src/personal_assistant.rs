use crate::{
    benchmark::dataset::require_workspace,
    commands::{AppState, blocking},
    db::error,
    models::AppResult,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Serialize;
use tauri::State;

pub(crate) fn template_preset(template: &str) -> AppResult<Option<(&'static str, &'static str)>> {
    match template {
        "" | "blank" => Ok(None),
        "study" => Ok(Some((
            "学习备考教练",
            "你是我在这个工作区的学习备考教练。优先结合我导入的 PDF/PPT 课件与笔记回答，讲解概念时先用通俗类比讲清直觉再给严谨定义；涉及易错点主动提醒；当我请你出题或抽背时，先出题等待我作答再逐题批改并总结错因。",
        ))),
        "work" => Ok(Some((
            "职场项目搭档",
            "你是我在这个工作区的职场项目搭档。回答遵循结论先行、分点清晰、可直接复用的原则；整理会议纪要、表格数据或方案时自动提炼关键决策、风险点与下一步待办；避免空洞客套。",
        ))),
        "writing" => Ok(Some((
            "阅读写作编辑",
            "你是我在这个工作区的阅读与写作编辑。帮我拆解电子书/长文核心论点、串联素材并起草大纲或成稿；润色文字时保留我的原始观点与口吻，消除冗余和公文腔，优先用具体案例和清晰过渡替代套话。",
        ))),
        "life" => Ok(Some((
            "私人生活管家",
            "你是我在这个工作区的私人生活管家。解读体检报告、保险条款、合同或账单时用大白话讲清重点、注意事项和潜在坑点；涉及医疗或法律问题时客观提示风险并建议咨询专业人士；简洁暖心，条理清晰。",
        ))),
        _ => Err("不支持的工作区场景模板。".into()),
    }
}

pub(crate) fn apply_template_in_tx(
    tx: &Connection,
    workspace_id: &str,
    template: Option<&str>,
) -> AppResult<()> {
    if let Some((name, instructions)) = template_preset(template.unwrap_or("blank").trim())? {
        tx.execute(
            "INSERT INTO assistant_versions(workspace_id,version,name,instructions)
             VALUES (?1,1,?2,?3)",
            params![workspace_id, name, instructions],
        )
        .map_err(error)?;
        tx.execute(
            "INSERT INTO assistant_drafts(workspace_id,name,instructions,active_version)
             VALUES (?1,?2,?3,1)",
            params![workspace_id, name, instructions],
        )
        .map_err(error)?;
    }
    Ok(())
}

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
