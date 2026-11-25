use crate::{
    benchmark::dataset::require_workspace,
    commands::{AppState, blocking},
    db::{error, json_error},
    knowledge_scope::SOURCE_AVAILABLE,
    models::{AppResult, new_id, now},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GlobalPersona {
    pub display_name: String,
    pub role_background: String,
    pub communication_style: String,
    pub preferences: Vec<String>,
    pub rules: Vec<String>,
    pub updated_at: String,
}

impl GlobalPersona {
    pub(crate) fn validate(&mut self) -> AppResult<()> {
        self.display_name = self.display_name.trim().to_owned();
        self.role_background = self.role_background.trim().to_owned();
        self.communication_style = self.communication_style.trim().to_owned();
        if self.display_name.chars().count() > 60
            || self.role_background.chars().count() > 800
            || self.communication_style.chars().count() > 800
        {
            return Err("称呼最多 60 字，背景与沟通风格最多各 800 字。".into());
        }
        let clean = |items: &mut Vec<String>, max_items: usize| -> AppResult<()> {
            let mut deduped = Vec::new();
            for item in items.drain(..) {
                let text = item.trim();
                if text.is_empty() {
                    continue;
                }
                if text.chars().count() > 240 {
                    return Err("单条偏好或长期规则最多 240 字。".into());
                }
                if !deduped.iter().any(|existing: &String| existing == text) {
                    deduped.push(text.to_owned());
                }
            }
            if deduped.len() > max_items {
                return Err(format!("最多保存 {max_items} 条记录。"));
            }
            *items = deduped;
            Ok(())
        };
        clean(&mut self.preferences, 40)?;
        clean(&mut self.rules, 40)?;
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.display_name.is_empty()
            && self.role_background.is_empty()
            && self.communication_style.is_empty()
            && self.preferences.is_empty()
            && self.rules.is_empty()
    }

    pub(crate) fn format_prompt(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut lines = vec!["关于当前用户（全局长期画像，跨工作区生效）：".to_string()];
        if !self.display_name.is_empty() {
            lines.push(format!("- 称呼：{}", self.display_name));
        }
        if !self.role_background.is_empty() {
            lines.push(format!("- 身份与背景：{}", self.role_background));
        }
        if !self.communication_style.is_empty() {
            lines.push(format!("- 偏好表达风格：{}", self.communication_style));
        }
        if !self.preferences.is_empty() {
            lines.push("- 个人习惯与偏好：".into());
            for item in &self.preferences {
                lines.push(format!("  * {item}"));
            }
        }
        if !self.rules.is_empty() {
            lines.push("- 必须遵守的长期规则：".into());
            for rule in &self.rules {
                lines.push(format!("  * {rule}"));
            }
        }
        Some(lines.join("\n"))
    }
}

pub(crate) fn read_persona(c: &Connection) -> AppResult<GlobalPersona> {
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='global_persona')",
            [],
            |r| r.get(0),
        )
        .map_err(error)?;
    if !exists {
        return Ok(GlobalPersona::default());
    }
    let row: Option<(String, String, String, String, String, String)> = c
        .query_row(
            "SELECT display_name,role_background,communication_style,preferences_json,rules_json,updated_at
             FROM global_persona WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(error)?;
    match row {
        Some((display_name, role_background, communication_style, prefs_raw, rules_raw, updated_at)) => {
            Ok(GlobalPersona {
                display_name,
                role_background,
                communication_style,
                preferences: serde_json::from_str(&prefs_raw).map_err(json_error)?,
                rules: serde_json::from_str(&rules_raw).map_err(json_error)?,
                updated_at,
            })
        }
        None => Ok(GlobalPersona::default()),
    }
}

pub(crate) fn write_persona(c: &Connection, persona: &mut GlobalPersona) -> AppResult<()> {
    persona.validate()?;
    let timestamp = now();
    persona.updated_at = timestamp.clone();
    let prefs_json = serde_json::to_string(&persona.preferences).map_err(json_error)?;
    let rules_json = serde_json::to_string(&persona.rules).map_err(json_error)?;
    c.execute(
        "INSERT INTO global_persona(id,display_name,role_background,communication_style,preferences_json,rules_json,updated_at)
         VALUES(1,?1,?2,?3,?4,?5,?6)
         ON CONFLICT(id) DO UPDATE SET
           display_name=excluded.display_name,
           role_background=excluded.role_background,
           communication_style=excluded.communication_style,
           preferences_json=excluded.preferences_json,
           rules_json=excluded.rules_json,
           updated_at=excluded.updated_at",
        params![
            persona.display_name,
            persona.role_background,
            persona.communication_style,
            prefs_json,
            rules_json,
            timestamp
        ],
    )
    .map_err(error)?;
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryCandidate {
    pub id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub source_message_id: Option<String>,
    pub scope: String,
    pub title: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
struct ExtractedCandidate {
    scope: &'static str,
    title: String,
    content: String,
}

/// Pure, zero-GPU-cost deterministic extractor for explicit user preferences,
/// corrections/rules, and project facts stated during conversation.
fn extract_from_user_message(user_text: &str) -> Vec<ExtractedCandidate> {
    let trimmed = user_text.trim();
    if trimmed.chars().count() < 5 || trimmed.chars().count() > 1200 {
        return vec![];
    }
    let mut results = Vec::new();
    let segments: Vec<&str> = trimmed
        .split(['\n', '。', '！', '；'])
        .map(str::trim)
        .filter(|s| s.chars().count() >= 5 && s.chars().count() <= 220)
        .collect();

    let rule_markers = [
        "以后", "下次", "记住", "请总是", "不要再", "别再", "不要用", "别用",
        "太啰嗦", "精简", "直接给", "先给结论", "结论放前面", "少解释",
    ];
    let pref_markers = [
        "我是", "我做", "我在做", "我习惯", "我喜欢", "我偏好", "我常用", "我的职业", "我主要用",
    ];
    let project_markers = [
        "我们项目", "这个项目", "当前项目", "技术栈是", "目标是", "决定采用", "统一使用", "约定",
    ];

    for seg in segments {
        if seg.ends_with('?') || seg.ends_with('？') {
            continue;
        }
        let (scope, title_prefix) = if rule_markers.iter().any(|m| seg.contains(m)) {
            ("global_rule", "长期规则")
        } else if pref_markers.iter().any(|m| seg.contains(m)) {
            ("global_preference", "个人偏好")
        } else if project_markers.iter().any(|m| seg.contains(m)) {
            ("workspace_fact", "项目事实")
        } else {
            continue;
        };
        let snippet: String = seg.chars().take(28).collect();
        let title = format!("{title_prefix}：{snippet}");
        if !results.iter().any(|c: &ExtractedCandidate| c.content == seg) {
            results.push(ExtractedCandidate {
                scope,
                title,
                content: seg.to_owned(),
            });
        }
        if results.len() >= 2 {
            break;
        }
    }
    results
}

/// Called after a chat turn finishes: records pending memory candidates and refreshes
/// the lightweight cross-conversation episodic summary.
pub(crate) fn record_turn_insights(
    c: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    source_run_id: Option<&str>,
    user_message_id: &str,
    user_text: &str,
    assistant_text: &str,
) -> AppResult<()> {
    let table_exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_candidates')",
            [],
            |r| r.get(0),
        )
        .map_err(error)?;
    if !table_exists {
        return Ok(());
    }
    let timestamp = now();
    let persona = read_persona(c)?;
    for candidate in extract_from_user_message(user_text) {
        let already_known = match candidate.scope {
            "global_rule" => persona.rules.iter().any(|r| r == &candidate.content),
            "global_preference" => persona.preferences.iter().any(|p| p == &candidate.content),
            _ => false,
        };
        if already_known {
            continue;
        }
        let dup: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_candidates WHERE workspace_id=?1 AND content=?2 AND status IN ('pending','accepted'))",
                params![workspace_id, candidate.content],
                |r| r.get(0),
            )
            .map_err(error)?;
        if dup {
            continue;
        }
        c.execute(
            "INSERT INTO memory_candidates(id,workspace_id,conversation_id,source_message_id,scope,title,content,status,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,'pending',?8)",
            params![
                new_id(),
                workspace_id,
                conversation_id,
                user_message_id,
                candidate.scope,
                candidate.title,
                candidate.content,
                timestamp
            ],
        )
        .map_err(error)?;
    }

    // Update lightweight cross-conversation episodic summary.
    let q_brief: String = user_text.trim().chars().take(60).collect();
    let a_brief: String = assistant_text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(100)
        .collect();
    if !q_brief.is_empty() && !a_brief.is_empty() {
        let count: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE workspace_id=?1 AND conversation_id=?2 AND status='completed'",
                params![workspace_id, conversation_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        let summary_line = format!("讨论「{q_brief}」→ {a_brief}");
        c.execute(
            "INSERT INTO conversation_summaries(conversation_id,workspace_id,summary,message_count,updated_at,source_run_id)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(conversation_id) DO UPDATE SET
               summary=excluded.summary,
               message_count=excluded.message_count,
               updated_at=excluded.updated_at,
               source_run_id=excluded.source_run_id",
            params![conversation_id, workspace_id, summary_line, count, timestamp, source_run_id],
        )
        .map_err(error)?;
    }
    Ok(())
}

/// Reads recent episodic summaries from other conversations in the same workspace
/// so a fresh conversation still recalls what was recently discussed.
pub(crate) fn recent_workspace_episodes(
    c: &Connection,
    workspace_id: &str,
    current_conversation_id: &str,
) -> AppResult<Vec<(String, String, Option<String>)>> {
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='conversation_summaries')",
            [],
            |r| r.get(0),
        )
        .map_err(error)?;
    if !exists {
        return Ok(vec![]);
    }
    let mut stmt = c
        .prepare(
            "SELECT c.title, s.summary, s.source_run_id
             FROM conversation_summaries s
             JOIN conversations c ON c.id=s.conversation_id
             WHERE s.workspace_id=?1 AND s.conversation_id<>?2
             ORDER BY s.updated_at DESC
             LIMIT 3",
        )
        .map_err(error)?;
    let rows = stmt
        .query_map(params![workspace_id, current_conversation_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    Ok(rows)
}

#[tauri::command]
pub(crate) async fn get_global_persona(state: State<'_, AppState>) -> AppResult<GlobalPersona> {
    blocking(state.database.clone()?, |db| {
        let c = db.connection()?;
        read_persona(&c)
    })
    .await
}

#[tauri::command]
pub(crate) async fn save_global_persona(
    state: State<'_, AppState>,
    mut input: GlobalPersona,
) -> AppResult<GlobalPersona> {
    input.validate()?;
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        write_persona(&c, &mut input)?;
        Ok(input)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_memory_candidates(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: Option<String>,
) -> AppResult<Vec<MemoryCandidate>> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let (sql, filter_conv) = match conversation_id.as_deref() {
            Some(cid) if !cid.is_empty() => (
                "SELECT id,workspace_id,conversation_id,source_message_id,scope,title,content,status,created_at
                 FROM memory_candidates
                 WHERE workspace_id=?1 AND conversation_id=?2 AND status='pending'
                 ORDER BY created_at DESC LIMIT 20",
                Some(cid.to_owned()),
            ),
            _ => (
                "SELECT id,workspace_id,conversation_id,source_message_id,scope,title,content,status,created_at
                 FROM memory_candidates
                 WHERE workspace_id=?1 AND status='pending'
                 ORDER BY created_at DESC LIMIT 30",
                None,
            ),
        };
        let mut stmt = c.prepare(sql).map_err(error)?;
        let mapper = |r: &rusqlite::Row<'_>| {
            Ok(MemoryCandidate {
                id: r.get(0)?,
                workspace_id: r.get(1)?,
                conversation_id: r.get(2)?,
                source_message_id: r.get(3)?,
                scope: r.get(4)?,
                title: r.get(5)?,
                content: r.get(6)?,
                status: r.get(7)?,
                created_at: r.get(8)?,
            })
        };
        let rows = match filter_conv {
            Some(cid) => stmt
                .query_map(params![workspace_id, cid], mapper)
                .map_err(error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(error)?,
            None => stmt
                .query_map(params![workspace_id], mapper)
                .map_err(error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(error)?,
        };
        Ok(rows)
    })
    .await
}

#[tauri::command]
pub(crate) async fn resolve_memory_candidate(
    state: State<'_, AppState>,
    workspace_id: String,
    candidate_id: String,
    action: String,
    edited_content: Option<String>,
) -> AppResult<()> {
    if !matches!(action.as_str(), "accept" | "dismiss") {
        return Err("不支持的记忆操作。".into());
    }
    blocking(state.database.clone()?, move |db| {
        let mut conn = db.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        require_workspace(&tx, &workspace_id)?;
        let row: Option<(String, String, String, Option<String>)> = tx
            .query_row(
                "SELECT scope,title,content,source_message_id FROM memory_candidates
                 WHERE id=?1 AND workspace_id=?2 AND status='pending'",
                params![candidate_id, workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(error)?;
        let Some((scope, title, original_content, source_message_id)) = row else {
            return Err("该候选记忆已处理或不存在。".into());
        };
        let final_content = edited_content
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&original_content)
            .to_owned();
        let timestamp = now();
        let next_status = if action == "accept" { "accepted" } else { "dismissed" };
        tx.execute(
            "UPDATE memory_candidates SET status=?2,content=?3,resolved_at=?4 WHERE id=?1",
            params![candidate_id, next_status, final_content, timestamp],
        )
        .map_err(error)?;

        if action == "accept" {
            match scope.as_str() {
                "global_rule" => {
                    let mut persona = read_persona(&tx)?;
                    if !persona.rules.iter().any(|r| r == &final_content) {
                        persona.rules.push(final_content);
                        write_persona(&tx, &mut persona)?;
                    }
                }
                "global_preference" => {
                    let mut persona = read_persona(&tx)?;
                    if !persona.preferences.iter().any(|p| p == &final_content) {
                        persona.preferences.push(final_content);
                        write_persona(&tx, &mut persona)?;
                    }
                }
                _ => {
                    let node_id = new_id();
                    let short_title: String = title.chars().take(80).collect();
                    tx.execute(
                        "INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at,source_conversation_id,source_message_id,source_excerpt,source_hash)
                         VALUES(?1,?2,1,1,2,?3,?3,NULL,?4,NULL,NULL)",
                        params![node_id, workspace_id, timestamp, source_message_id],
                    )
                    .map_err(error)?;
                    tx.execute(
                        "INSERT INTO knowledge_versions(node_id,version,title,kind,content,author,change_reason,created_at)
                         VALUES(?1,1,?2,'knowledge',?3,'user','对话中确认的记忆',?4)",
                        params![node_id, short_title, final_content, timestamp],
                    )
                    .map_err(error)?;
                    tx.execute(
                        "INSERT INTO knowledge_approval_events(id,node_id,version,actor,created_at)
                         VALUES(?1,?2,1,'user',?3)",
                        params![new_id(), node_id, timestamp],
                    )
                    .map_err(error)?;
                    crate::retrieval::index_node(&tx, &node_id)?;
                }
            }
        }
        tx.commit().map_err(error)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeeklyDigestDraft {
    pub workspace_id: String,
    pub period_days: u32,
    pub episode_count: usize,
    pub knowledge_count: usize,
    pub markdown: String,
}

#[tauri::command]
pub(crate) async fn generate_weekly_digest_draft(
    state: State<'_, AppState>,
    workspace_id: String,
    days: Option<u32>,
) -> AppResult<WeeklyDigestDraft> {
    let period_days = days.unwrap_or(7).clamp(1, 30);
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let ws_name: String = c
            .query_row("SELECT name FROM workspaces WHERE id=?1", [&workspace_id], |r| r.get(0))
            .map_err(error)?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(i64::from(period_days)))
            .to_rfc3339();
        let persona = read_persona(&c)?;

        let mut ep_stmt = c
            .prepare(
                "SELECT c.title, s.summary, s.updated_at
                 FROM conversation_summaries s
                 JOIN conversations c ON c.id=s.conversation_id AND c.workspace_id=s.workspace_id
                 WHERE s.workspace_id=?1 AND s.updated_at>=?2 AND c.purpose='chat'
                 ORDER BY s.updated_at DESC LIMIT 20",
            )
            .map_err(error)?;
        let episodes: Vec<(String, String, String)> = ep_stmt
            .query_map(params![workspace_id, cutoff], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(error)?;

        let mut kn_stmt = c
            .prepare(&format!(
                "SELECT v.title, v.kind, v.content
                 FROM knowledge_nodes n
                 JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
                 LEFT JOIN knowledge_validity d ON d.node_id=v.node_id AND d.version=v.version
                 WHERE n.workspace_id=?1 AND n.updated_at>=?2
                   AND (d.valid_from IS NULL OR d.valid_from<=date('now','localtime'))
                   AND (d.valid_to IS NULL OR d.valid_to>date('now','localtime'))
                   AND {SOURCE_AVAILABLE}
                 ORDER BY n.updated_at DESC LIMIT 20",
            ))
            .map_err(error)?;
        let knowledges: Vec<(String, String, String)> = kn_stmt
            .query_map(params![workspace_id, cutoff], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(error)?;

        let mut lines = vec![
            format!("# {ws_name} · 近 {period_days} 天进展周报草稿"),
            String::new(),
        ];
        if !persona.display_name.is_empty() || !persona.role_background.is_empty() {
            lines.push(format!(
                "> 汇报人：{}{}",
                if persona.display_name.is_empty() { "当前用户" } else { &persona.display_name },
                if persona.role_background.is_empty() { String::new() } else { format!("（{}）", persona.role_background) }
            ));
            lines.push(String::new());
        }
        lines.push("## 一、近期讨论与推进".into());
        if episodes.is_empty() {
            lines.push("- 近期待补充：本周期内暂无已沉淀的会话摘要。".into());
        } else {
            for (title, summary, _) in &episodes {
                lines.push(format!("- **{title}**：{summary}"));
            }
        }
        lines.push(String::new());
        lines.push("## 二、新增与更新的项目记忆 / 资料".into());
        if knowledges.is_empty() {
            lines.push("- 本周期内暂无新增生效记忆或资料。".into());
        } else {
            for (title, _, content) in &knowledges {
                let preview: String = content.lines().next().unwrap_or("").chars().take(90).collect();
                lines.push(format!("- **{title}**：{preview}"));
            }
        }
        lines.push(String::new());
        lines.push("## 三、后续关注与待办".into());
        lines.push("- [ ] 核对上述进展细节并补充下周核心目标".into());

        Ok(WeeklyDigestDraft {
            workspace_id,
            period_days,
            episode_count: episodes.len(),
            knowledge_count: knowledges.len(),
            markdown: lines.join("\n"),
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn promote_rule_from_chat(
    state: State<'_, AppState>,
    rule: String,
) -> AppResult<GlobalPersona> {
    let text = rule.trim().to_owned();
    if text.is_empty() || text.chars().count() > 240 {
        return Err("长期规则需为 1 至 240 字。".into());
    }
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        let mut persona = read_persona(&c)?;
        if !persona.rules.iter().any(|r| r == &text) {
            persona.rules.push(text);
        }
        write_persona(&c, &mut persona)?;
        Ok(persona)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn global_persona_and_memory_extraction_cycle() {
        let db = Database::rag_evaluation_database().unwrap();
        let ws = db.create_workspace("专属AI验证").unwrap().id;
        let conv1 = db.create_conversation(&ws).unwrap().id;
        let conv2 = db.create_conversation(&ws).unwrap().id;

        {
            let c = db.connection().unwrap();
            let mut p = GlobalPersona {
                display_name: "阿明".into(),
                role_background: "独立开发者，专注 Rust 与桌面端产品".into(),
                communication_style: "结论先行，直接给代码示例".into(),
                preferences: vec!["优先用 TypeScript".into()],
                rules: vec!["不要输出客套话".into()],
                updated_at: String::new(),
            };
            write_persona(&c, &mut p).unwrap();
            let loaded = read_persona(&c).unwrap();
            assert_eq!(loaded.display_name, "阿明");
            assert!(loaded.format_prompt().unwrap().contains("不要输出客套话"));

            record_turn_insights(
                &c,
                &ws,
                &conv1,
                None,
                "msg-1",
                "以后回答技术问题请直接给 TypeScript 例子。我们项目统一使用 SQLite WAL 模式。",
                "好的，后续技术示例将优先采用 TypeScript。",
            )
            .unwrap();

            let count: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM memory_candidates WHERE workspace_id=?1 AND status='pending'",
                    [&ws],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 2);

            let episodes = recent_workspace_episodes(&c, &ws, &conv2).unwrap();
            assert_eq!(episodes.len(), 1);
            assert!(episodes[0].1.contains("TypeScript"));
        }
    }
}
