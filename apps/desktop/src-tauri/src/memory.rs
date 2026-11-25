use crate::{
    benchmark::dataset::{hash, require_workspace},
    commands::{AppState, blocking},
    db::{error, json_error},
    knowledge_scope::SOURCE_AVAILABLE,
    models::{AppResult, new_id, now},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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
    pub replaces_node_id: Option<String>,
    pub replaces_title: Option<String>,
    pub replaces_value: Option<String>,
    pub conflict_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiMemorySuggestion {
    pub(crate) scope: String,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) replaces_node_id: Option<String>,
    pub(crate) replaces_value: Option<String>,
    pub(crate) conflict_reason: Option<String>,
}

impl AiMemorySuggestion {
    pub(crate) fn valid(&self) -> bool {
        let length = |value: &str, max| (1..=max).contains(&value.trim().chars().count());
        matches!(
            self.scope.as_str(),
            "global_rule" | "global_preference" | "workspace_fact"
        ) && length(&self.title, 80)
            && length(&self.content, 240)
            && self
                .replaces_node_id
                .as_deref()
                .is_none_or(|id| length(id, 128))
            && self
                .replaces_value
                .as_deref()
                .is_none_or(|value| length(value, 240))
            && self
                .conflict_reason
                .as_deref()
                .is_none_or(|reason| length(reason, 240))
    }
}

const CHANGE_MARKERS: &[&str] = &[
    "改用",
    "换成",
    "迁移到",
    "切换为",
    "不再",
    "弃用",
    "方案改",
    "从",
];
const MEMORY_EXCERPT_LENGTH: usize = 160;

/// Extracts candidate preferences, long-term rules and workspace facts from user text.
fn extract_from_user_message(user_text: &str) -> Vec<AiMemorySuggestion> {
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
        "以后",
        "下次",
        "记住",
        "请总是",
        "不要再",
        "别再",
        "不要用",
        "别用",
        "太啰嗦",
        "精简",
        "直接给",
        "先给结论",
        "结论放前面",
        "少解释",
    ];
    let pref_markers = [
        "我是",
        "我做",
        "我在",
        "我习惯",
        "我喜欢",
        "我偏好",
        "我常用",
        "我的职业",
        "我主要用",
        "目前",
        "准备考",
        "备考",
        "正在学",
        "大一",
        "大二",
        "大三",
        "大四",
        "研一",
        "研二",
    ];
    let project_markers = [
        "我们项目",
        "这个项目",
        "当前项目",
        "技术栈是",
        "目标是",
        "决定采用",
        "统一使用",
        "约定",
        "改用",
        "换成",
        "迁移到",
        "切换为",
        "不再使用",
        "弃用",
        "架构是",
        "方案改",
    ];

    for seg in segments {
        if seg.ends_with('?') || seg.ends_with('？') {
            continue;
        }
        let (scope, title_prefix) = if rule_markers.iter().any(|m| seg.contains(m)) {
            ("global_rule", "长期规则")
        } else if pref_markers.iter().any(|m| seg.contains(m))
            && !project_markers
                .iter()
                .any(|m| !matches!(*m, "目标是" | "约定") && seg.contains(m))
        {
            ("global_preference", "个人偏好")
        } else if project_markers.iter().any(|m| seg.contains(m)) {
            ("workspace_fact", "项目事实")
        } else {
            continue;
        };
        let snippet: String = seg.chars().take(28).collect();
        let title = format!("{title_prefix}：{snippet}");
        if !results
            .iter()
            .any(|c: &AiMemorySuggestion| c.content == seg)
        {
            results.push(AiMemorySuggestion {
                scope: scope.into(),
                title,
                content: seg.to_owned(),
                ..Default::default()
            });
        }
        if results.len() >= 2 {
            break;
        }
    }
    results
}

fn persona_items<'a>(persona: &'a GlobalPersona, scope: &str) -> &'a [String] {
    match scope {
        "global_rule" => &persona.rules,
        "global_preference" => &persona.preferences,
        _ => &[],
    }
}

fn detect_conflict(
    c: &Connection,
    workspace_id: &str,
    persona: &GlobalPersona,
    candidate: &mut AiMemorySuggestion,
) -> AppResult<()> {
    use crate::retrieval::terms;

    let candidate_terms: BTreeSet<_> = terms(&candidate.content).into_iter().collect();
    let overlap = |text: &str| {
        let old_terms: BTreeSet<_> = terms(text).into_iter().collect();
        candidate_terms.intersection(&old_terms).count()
    };
    if candidate.scope == "workspace_fact" {
        let explicit_change = CHANGE_MARKERS
            .iter()
            .any(|marker| candidate.content.contains(marker));
        let min_overlap = if explicit_change { 1 } else { 2 };
        let mut stmt = c
            .prepare(&format!(
                "SELECT n.id,v.title,v.content FROM knowledge_nodes n
             JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
             WHERE n.workspace_id=?1 AND {SOURCE_AVAILABLE}
             ORDER BY n.updated_at DESC,n.id"
            ))
            .map_err(error)?;
        let rows = stmt
            .query_map([workspace_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(error)?;
        let mut best_overlap = min_overlap - 1;
        for row in rows {
            let (id, title, content) = row.map_err(error)?;
            let score = overlap(&format!("{title}\n{content}"));
            if content.trim() != candidate.content && score > best_overlap {
                best_overlap = score;
                candidate.replaces_node_id = Some(id);
                candidate.replaces_value =
                    Some(content.trim().chars().take(MEMORY_EXCERPT_LENGTH).collect());
                candidate.conflict_reason = Some(format!("与既有记忆《{title}》存在更新或冲突"));
            }
        }
    } else {
        let mut best_overlap = 1;
        for item in persona_items(persona, &candidate.scope) {
            let score = overlap(item);
            if item != &candidate.content && score > best_overlap {
                best_overlap = score;
                candidate.replaces_value = Some(item.clone());
                candidate.conflict_reason = Some("与「关于我」中既有条目存在更新或冲突".into());
            }
        }
    }
    Ok(())
}

fn insert_memory_candidate(
    c: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    user_message_id: Option<&str>,
    item: &AiMemorySuggestion,
) -> AppResult<usize> {
    c.execute(
        "INSERT INTO memory_candidates(id,workspace_id,conversation_id,source_message_id,scope,title,content,status,created_at,replaces_node_id,replaces_value,conflict_reason)
         SELECT ?1,?2,?3,?4,?5,?6,?7,'pending',?8,?9,?10,?11
         WHERE EXISTS(SELECT 1 FROM conversations WHERE id=?3 AND workspace_id=?2)
           AND NOT EXISTS(SELECT 1 FROM memory_candidates
               WHERE workspace_id=?2 AND content=?7 AND status IN ('pending','accepted'))
           AND (?5<>'workspace_fact' OR NOT EXISTS(
               SELECT 1 FROM knowledge_nodes n
               JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
               WHERE n.workspace_id=?2 AND v.content=?7))",
        params![
            new_id(), workspace_id, conversation_id, user_message_id, item.scope, item.title,
            item.content, now(), item.replaces_node_id, item.replaces_value, item.conflict_reason,
        ],
    ).map_err(error)
}

pub(crate) fn cloud_memory_context(
    c: &Connection,
    workspace_id: &str,
) -> AppResult<Option<String>> {
    if !crate::cloud::privacy::Rules::read(c, workspace_id)?.sync_memory_and_history {
        return Ok(None);
    }

    let mut lines = read_persona(c)?
        .format_prompt()
        .into_iter()
        .collect::<Vec<_>>();
    let mut stmt = c
        .prepare(&format!(
            "SELECT n.id,v.title,substr(v.content,1,{MEMORY_EXCERPT_LENGTH})
         FROM knowledge_nodes n
         JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
         LEFT JOIN knowledge_local_only p ON p.workspace_id=n.workspace_id AND p.node_id=n.id
         WHERE n.workspace_id=?1 AND COALESCE(p.local_only,0)=0 AND {SOURCE_AVAILABLE}
         ORDER BY n.updated_at DESC,n.id LIMIT 8"
        ))
        .map_err(error)?;
    let rows = stmt
        .query_map([workspace_id], |r| {
            let (id, title, excerpt): (String, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
            Ok(format!("[nodeId={id}] {title}: {excerpt}"))
        })
        .map_err(error)?;
    for row in rows {
        lines.push(row.map_err(error)?);
    }
    Ok(Some(lines.join("\n")))
}

pub(crate) fn save_ai_memory_suggestions(
    c: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    user_message_id: Option<&str>,
    items: &[AiMemorySuggestion],
) -> AppResult<usize> {
    let tx = c.unchecked_transaction().map_err(error)?;
    let persona = read_persona(&tx)?;
    let mut saved = 0;
    for suggestion in items.iter().take(2).filter(|item| item.valid()) {
        let mut item = suggestion.clone();
        item.title = item.title.trim().into();
        item.content = item.content.trim().into();
        if persona
            .rules
            .iter()
            .chain(&persona.preferences)
            .any(|old| old == &item.content)
            || [
                &persona.role_background,
                &persona.communication_style,
                &persona.display_name,
            ]
            .contains(&&item.content)
        {
            continue;
        }
        if item.scope == "workspace_fact" {
            item.replaces_value = None;
            if let Some(id) = item.replaces_node_id.as_deref() {
                item.replaces_value = tx.query_row(
                    "SELECT substr(trim(v.content),1,?3) FROM knowledge_nodes n
                     JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
                     LEFT JOIN knowledge_local_only p ON p.workspace_id=n.workspace_id AND p.node_id=n.id
                     WHERE n.workspace_id=?1 AND n.id=?2 AND COALESCE(p.local_only,0)=0",
                    params![workspace_id, id, MEMORY_EXCERPT_LENGTH as i64],
                    |r| r.get(0),
                ).optional().map_err(error)?;
                if item.replaces_value.is_none() {
                    continue;
                }
            }
        } else {
            item.replaces_node_id = None;
            if item
                .replaces_value
                .as_ref()
                .is_some_and(|value| !persona_items(&persona, &item.scope).contains(value))
            {
                item.replaces_value = None;
            }
        }
        if item.replaces_value.is_none() {
            item.conflict_reason = None;
        }
        saved +=
            insert_memory_candidate(&tx, workspace_id, conversation_id, user_message_id, &item)?;
    }
    tx.commit().map_err(error)?;
    Ok(saved)
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
    for mut candidate in extract_from_user_message(user_text) {
        if persona_items(&persona, &candidate.scope).contains(&candidate.content) {
            continue;
        }
        detect_conflict(c, workspace_id, &persona, &mut candidate)?;
        insert_memory_candidate(
            c,
            workspace_id,
            conversation_id,
            Some(user_message_id),
            &candidate,
        )?;
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
        let filter_conv = conversation_id.as_deref().filter(|id| !id.is_empty());
        let limit = if filter_conv.is_some() { 20 } else { 30 };
        let mut stmt = c.prepare(
            "SELECT m.id,m.workspace_id,m.conversation_id,m.source_message_id,m.scope,m.title,
                    m.content,m.status,m.created_at,m.replaces_node_id,v.title,
                    COALESCE(m.replaces_value,substr(trim(v.content),1,?4)),m.conflict_reason
             FROM memory_candidates m
             LEFT JOIN knowledge_nodes n ON n.id=m.replaces_node_id AND n.workspace_id=m.workspace_id
             LEFT JOIN knowledge_versions v ON v.node_id=n.id AND v.version=COALESCE(n.effective_version,n.latest_version)
             WHERE m.workspace_id=?1 AND (?2 IS NULL OR m.conversation_id=?2) AND m.status='pending'
             ORDER BY m.created_at DESC,m.id LIMIT ?3",
        ).map_err(error)?;
        let rows = stmt
            .query_map(params![workspace_id, filter_conv, limit, MEMORY_EXCERPT_LENGTH as i64], |r| {
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
                    replaces_node_id: r.get(9)?,
                    replaces_title: r.get(10)?,
                    replaces_value: r.get(11)?,
                    conflict_reason: r.get(12)?,
                })
            })
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?;
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
    blocking(state.database.clone()?, move |db| {
        resolve_candidate(
            &mut *db.connection()?,
            &workspace_id,
            &candidate_id,
            &action,
            edited_content.as_deref(),
        )
    })
    .await
}

fn resolve_candidate(
    conn: &mut Connection,
    workspace_id: &str,
    candidate_id: &str,
    action: &str,
    edited_content: Option<&str>,
) -> AppResult<()> {
    if !matches!(action, "accept" | "dismiss") {
        return Err("不支持的记忆操作。".into());
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(error)?;
    require_workspace(&tx, workspace_id)?;
    let row = tx
            .query_row(
                "SELECT m.scope,m.title,m.content,m.source_message_id,m.replaces_node_id,m.replaces_value,
                        u.conversation_id,u.content
                 FROM memory_candidates m
                 LEFT JOIN messages u ON u.id=m.source_message_id AND u.workspace_id=m.workspace_id
                   AND u.conversation_id=m.conversation_id AND u.role='user'
                 WHERE m.id=?1 AND m.workspace_id=?2 AND m.status='pending'",
                params![candidate_id, workspace_id],
                |r| Ok((
                    r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                )),
            )
            .optional()
            .map_err(error)?;
    let Some((
        scope,
        title,
        original_content,
        source_message_id,
        replaces_node_id,
        replaces_value,
        source_conversation_id,
        source_text,
    )) = row
    else {
        return Err("该候选记忆已处理或不存在。".into());
    };
    let final_content = edited_content
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&original_content)
        .to_owned();
    let timestamp = now();
    let next_status = if action == "accept" {
        "accepted"
    } else {
        "dismissed"
    };
    tx.execute(
        "UPDATE memory_candidates SET status=?2,content=?3,resolved_at=?4 WHERE id=?1",
        params![candidate_id, next_status, final_content, timestamp],
    )
    .map_err(error)?;

    if action == "accept" {
        match scope.as_str() {
            "global_rule" | "global_preference" => {
                let mut persona = read_persona(&tx)?;
                let items = if scope == "global_rule" {
                    &mut persona.rules
                } else {
                    &mut persona.preferences
                };
                if let Some(old) = replaces_value
                    .as_ref()
                    .filter(|value| items.contains(value))
                {
                    items.retain(|item| item == old || item != &final_content);
                    if let Some(item) = items.iter_mut().find(|item| *item == old) {
                        *item = final_content;
                    }
                } else if !items.contains(&final_content) {
                    items.push(final_content);
                }
                write_persona(&tx, &mut persona)?;
            }
            _ => {
                let target = tx.query_row(
                        "SELECT n.id,n.latest_version,v.title,v.kind FROM knowledge_nodes n
                         JOIN knowledge_versions v ON v.node_id=n.id AND v.version=COALESCE(n.effective_version,n.latest_version)
                         WHERE n.id=?1 AND n.workspace_id=?2",
                        params![replaces_node_id, workspace_id],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
                    ).optional().map_err(error)?;
                let (node_id, version, title, kind, reason) = if let Some((
                    id,
                    latest,
                    title,
                    kind,
                )) = target
                {
                    let next = latest + 1;
                    tx.execute(
                            "UPDATE knowledge_nodes SET latest_version=?2,effective_version=?2,revision=revision+1,updated_at=?3 WHERE id=?1",
                            params![id, next, timestamp],
                        ).map_err(error)?;
                    (id, next, title, kind, "对话记忆演进更新")
                } else {
                    let id = new_id();
                    let source_excerpt = source_text
                        .as_ref()
                        .map(|text| text.chars().take(MEMORY_EXCERPT_LENGTH).collect::<String>());
                    let source_hash = source_text.as_ref().map(|text| hash(text.as_bytes()));
                    let source_message_id = source_conversation_id.as_ref().and(source_message_id);
                    tx.execute(
                            "INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at,source_conversation_id,source_message_id,source_excerpt,source_hash)
                             VALUES(?1,?2,1,1,2,?3,?3,?4,?5,?6,?7)",
                            params![id, workspace_id, timestamp, source_conversation_id, source_message_id, source_excerpt, source_hash],
                        ).map_err(error)?;
                    (
                        id,
                        1,
                        title.chars().take(80).collect(),
                        "knowledge".into(),
                        "对话中确认的记忆",
                    )
                };
                tx.execute(
                        "INSERT INTO knowledge_versions(node_id,version,title,kind,content,author,change_reason,created_at)
                         VALUES(?1,?2,?3,?4,?5,'user',?6,?7)",
                        params![node_id, version, title, kind, final_content, reason, timestamp],
                    ).map_err(error)?;
                // The lineage view derives predecessor and successor versions from approval events.
                tx.execute(
                    "INSERT INTO knowledge_approval_events(id,node_id,version,actor,created_at)
                         VALUES(?1,?2,?3,'user',?4)",
                    params![new_id(), node_id, version, timestamp],
                )
                .map_err(error)?;
                crate::retrieval::index_node(&tx, &node_id)?;
            }
        }
    }
    tx.commit().map_err(error)
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusEpisode {
    pub conversation_id: String,
    pub title: String,
    pub summary: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusSourceUpdate {
    pub source_id: String,
    pub node_id: Option<String>,
    pub name: String,
    pub version: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusBriefItem {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusBrief {
    pub title: String,
    pub query: String,
    pub scheduled_at: String,
    pub headlines: Vec<FocusBriefItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusReminder {
    pub id: String,
    pub title: String,
    pub next_date: String,
    pub time: String,
    pub is_briefing: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyFocus {
    pub workspace_id: String,
    pub workspace_name: String,
    pub display_name: String,
    pub has_activity: bool,
    pub knowledge_count: usize,
    pub pending_memory_count: usize,
    pub recent_episodes: Vec<FocusEpisode>,
    pub recent_sources: Vec<FocusSourceUpdate>,
    pub latest_brief: Option<FocusBrief>,
    pub upcoming_reminders: Vec<FocusReminder>,
    pub suggested_prompts: Vec<String>,
}

pub(crate) fn query_daily_focus(c: &Connection, workspace_id: &str) -> AppResult<DailyFocus> {
    require_workspace(c, workspace_id)?;
    let workspace_name = c
        .query_row(
            "SELECT name FROM workspaces WHERE id=?1",
            [workspace_id],
            |r| r.get(0),
        )
        .map_err(error)?;
    let display_name = read_persona(c)?.display_name;
    let knowledge_count: i64 = c
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM knowledge_nodes n
                 JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
                 WHERE n.workspace_id=?1 AND n.effective_version IS NOT NULL AND {SOURCE_AVAILABLE}"
            ),
            [workspace_id],
            |r| r.get(0),
        )
        .map_err(error)?;
    let pending_memory_count: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM memory_candidates WHERE workspace_id=?1 AND status='pending'",
            [workspace_id],
            |r| r.get(0),
        )
        .map_err(error)?;

    let mut episodes = c
        .prepare(
            "SELECT c.id, c.title, s.summary, COALESCE(s.updated_at, c.updated_at)
             FROM conversations c
             LEFT JOIN conversation_summaries s ON s.conversation_id=c.id AND s.workspace_id=c.workspace_id
             WHERE c.workspace_id=?1 AND c.purpose='chat' AND c.deleted_at IS NULL AND c.archived_at IS NULL
               AND EXISTS(SELECT 1 FROM messages m WHERE m.workspace_id=c.workspace_id
                          AND m.conversation_id=c.id AND m.status='completed')
             ORDER BY COALESCE(s.updated_at, c.updated_at) DESC, c.id LIMIT 3",
        )
        .map_err(error)?;
    let recent_episodes = episodes
        .query_map([workspace_id], |r| {
            let conversation_id: String = r.get(0)?;
            let summary: Option<String> = r.get(2)?;
            let summary = match summary.filter(|text| !text.trim().is_empty()) {
                Some(summary) => summary,
                None => {
                    let content: String = c.query_row(
                        "SELECT content FROM messages WHERE workspace_id=?1 AND conversation_id=?2
                         AND status='completed' ORDER BY seq DESC LIMIT 1",
                        params![workspace_id, conversation_id],
                        |message| message.get(0),
                    )?;
                    let excerpt: String = content
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(80)
                        .collect();
                    excerpt.trim_end().to_owned()
                }
            };
            Ok(FocusEpisode {
                conversation_id,
                title: r.get(1)?,
                summary,
                updated_at: r.get(3)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;

    let cutoff_48h = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
    let mut sources = c
        .prepare(
            "SELECT d.id, smk.node_id, d.name, d.latest_version, sv.created_at
             FROM source_documents d
             JOIN source_versions sv ON sv.source_id=d.id AND sv.version=d.latest_version
             LEFT JOIN source_managed_knowledge smk ON smk.source_id=d.id
             WHERE d.workspace_id=?1 AND d.enabled=1 AND sv.created_at>=?2
             ORDER BY sv.created_at DESC, d.id LIMIT 4",
        )
        .map_err(error)?;
    let recent_sources = sources
        .query_map(params![workspace_id, cutoff_48h], |r| {
            Ok(FocusSourceUpdate {
                source_id: r.get(0)?,
                node_id: r.get(1)?,
                name: r.get(2)?,
                version: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;

    // The reminder writer stores successful briefings as 'ready'.
    let brief_row: Option<(String, i64, String, Option<String>)> = c
        .query_row(
            "SELECT title, scheduled_at, brief_query, brief_results
             FROM reminder_runs
             WHERE workspace_id=?1 AND brief_query IS NOT NULL AND brief_status IN ('completed', 'ready')
             ORDER BY scheduled_at DESC, id DESC LIMIT 1",
            [workspace_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(error)?;
    let latest_brief = if let Some((title, timestamp, query, results)) = brief_row {
        let scheduled_at = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
            .ok_or("本地提醒记录时间无效。")?
            .to_rfc3339();
        let mut headlines: Vec<FocusBriefItem> =
            serde_json::from_str(results.as_deref().unwrap_or("[]")).unwrap_or_default();
        headlines.truncate(3);
        Some(FocusBrief {
            title,
            query,
            scheduled_at,
            headlines,
        })
    } else {
        None
    };

    let mut reminders = c
        .prepare(
            "SELECT id, title, next_date, time, news_query IS NOT NULL
             FROM reminders
             WHERE workspace_id=?1 AND state='active' AND next_date IS NOT NULL
             ORDER BY next_date ASC, time ASC, id LIMIT 3",
        )
        .map_err(error)?;
    let upcoming_reminders = reminders
        .query_map([workspace_id], |r| {
            Ok(FocusReminder {
                id: r.get(0)?,
                title: r.get(1)?,
                next_date: r.get(2)?,
                time: r.get(3)?,
                is_briefing: r.get(4)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;

    let has_activity = knowledge_count > 0
        || pending_memory_count > 0
        || !recent_episodes.is_empty()
        || !recent_sources.is_empty()
        || latest_brief.is_some()
        || !upcoming_reminders.is_empty();
    let mut suggested_prompts = Vec::new();
    if let Some(source) = recent_sources.first() {
        suggested_prompts.push(format!(
            "梳理近期更新的《{}》核心要点，并指出与现有资料的关联",
            source.name
        ));
    }
    if let Some(episode) = recent_episodes.first() {
        suggested_prompts.push(format!(
            "接着上次「{}」的讨论，帮我梳理下一步行动项",
            episode.title
        ));
    }
    if let Some(brief) = &latest_brief {
        suggested_prompts.push(format!(
            "结合最新一期「{}」简报，分析值得跟进的重点动态",
            brief.title
        ));
    }
    if knowledge_count >= 2 {
        suggested_prompts.push("串联当前工作区的知识卡片，总结关键决策与潜在盲点".into());
    }
    suggested_prompts.truncate(3);
    suggested_prompts.extend(
        [
            "记住我的工作偏好：回答尽量简洁，先给结论再展开细节",
            "总结当前工作区里已有的项目资料和关键决策",
            "每个工作日早上 9 点给我整理一份 AI 行业简报",
        ]
        .into_iter()
        .take(3 - suggested_prompts.len())
        .map(str::to_owned),
    );

    Ok(DailyFocus {
        workspace_id: workspace_id.to_owned(),
        workspace_name,
        display_name,
        has_activity,
        knowledge_count: usize::try_from(knowledge_count).map_err(|err| err.to_string())?,
        pending_memory_count: usize::try_from(pending_memory_count)
            .map_err(|err| err.to_string())?,
        recent_episodes,
        recent_sources,
        latest_brief,
        upcoming_reminders,
        suggested_prompts,
    })
}

#[tauri::command]
pub(crate) async fn read_daily_focus(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<DailyFocus> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        query_daily_focus(&c, &workspace_id)
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
    fn daily_focus_aggregates_episodes_sources_briefings_and_respects_empty_workspace() {
        let db = Database::rag_evaluation_database().unwrap();
        let ws = db.create_workspace("今日聚焦").unwrap().id;
        let other_ws = db.create_workspace("其他工作区").unwrap().id;
        let mut c = db.connection().unwrap();
        let empty = query_daily_focus(&c, &ws).unwrap();
        assert!(!empty.has_activity);
        assert!(empty.recent_episodes.is_empty());
        assert!(empty.recent_sources.is_empty());
        assert!(empty.latest_brief.is_none());
        assert_eq!(empty.suggested_prompts.len(), 3);
        assert_eq!(
            empty
                .suggested_prompts
                .iter()
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );

        let tx = c.transaction().unwrap();
        write_persona(
            &tx,
            &mut GlobalPersona {
                display_name: "阿明".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let timestamp = now();
        for (workspace, id, title, updated_at, archived) in [
            (&ws, "chat", "发布计划", "2026-11-23", false),
            (&ws, "fallback", "日常讨论", "2026-11-22", false),
            (&ws, "archived", "归档讨论", "2026-11-25", true),
            (&other_ws, "other-chat", "其他项目", "2026-11-25", false),
        ] {
            tx.execute(
                "INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,archived_at)
                 VALUES(?1,?2,?3,?4,?4,?5)",
                params![
                    id,
                    workspace,
                    title,
                    updated_at,
                    archived.then_some(updated_at)
                ],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO execution_runs(id,workspace_id,conversation_id,assistant_message_id,
                    model_id,model_config_json,context_manifest_json,status,started_at,finished_at)
                 VALUES(?1,?2,?1,?1,'fixture','{}','{}','completed',?3,?3)",
                params![id, workspace, updated_at],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO messages VALUES
                    (?1 || '-question',?2,?1,1,'user','下一步是什么？','completed',?1,?3,?3),
                    (?1,?2,?1,2,'assistant',?4,'completed',?1,?3,?3)",
                params![
                    id,
                    workspace,
                    updated_at,
                    format!("  下一步\n  {}  ", "测".repeat(85))
                ],
            )
            .unwrap();
        }
        tx.execute(
            "INSERT INTO conversation_summaries(conversation_id,workspace_id,summary,message_count,updated_at)
             VALUES('chat',?1,'已确定发布顺序',2,'2026-11-24')",
            [&ws],
        ).unwrap();

        for (
            workspace,
            chat,
            source,
            source_name,
            reminder,
            brief_title,
            scheduled_at,
            next_date,
        ) in [
            (
                &ws,
                "chat",
                "source",
                "项目说明.md",
                "reminder",
                "AI 行业",
                1_700_000_000_i64,
                "2030-01-02",
            ),
            (
                &other_ws,
                "other-chat",
                "other-source",
                "其他资料.md",
                "other-reminder",
                "其他简报",
                1_700_086_400,
                "2030-01-01",
            ),
        ] {
            tx.execute(
                "INSERT INTO source_documents VALUES(?1,?2,?3,?3,1,1,1,?4,?4)",
                params![source, workspace, source_name, timestamp],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO source_versions VALUES(?1,1,'资料正文','content-hash','raw-hash',?2)",
                params![source, timestamp],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO memory_candidates(id,workspace_id,conversation_id,scope,title,content,created_at)
                 VALUES(?1,?2,?1,'workspace_fact','候选记忆','待审核内容',?3)",
                params![chat, workspace, timestamp],
            ).unwrap();
            tx.execute(
                "INSERT INTO reminders(id,workspace_id,title,kind,date,time,weekday,state,next_date,news_query)
                 VALUES(?1,?2,?3,'daily','2026-11-01','09:00',1,'active',?4,'AI 新闻')",
                params![reminder, workspace, brief_title, next_date],
            ).unwrap();
            let results = serde_json::json!(
                (1..=4)
                    .map(|i| serde_json::json!({
                        "title": format!("新闻 {i}"),
                        "url": format!("https://example.com/{i}"),
                        "snippet": "摘要",
                    }))
                    .collect::<Vec<_>>()
            );
            tx.execute(
                "INSERT INTO reminder_runs(id,workspace_id,reminder_id,title,scheduled_at,status,brief_query,brief_status,brief_results)
                 VALUES(?1,?2,?1,?3,?4,'delivered','AI 新闻','ready',?5)",
                params![reminder, workspace, brief_title, scheduled_at, results.to_string()],
            ).unwrap();
        }
        tx.execute(
            "INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at)
             VALUES('node',?1,1,1,1,?3,?3),('other-node',?2,1,1,1,?3,?3),('draft',?1,1,NULL,1,?3,?3)",
            params![ws, other_ws, timestamp],
        ).unwrap();
        tx.execute(
            "INSERT INTO knowledge_versions(node_id,version,title,kind,content,author,change_reason,created_at)
             SELECT id,1,'知识','knowledge','正文','user','测试',?1 FROM knowledge_nodes",
            [&timestamp],
        ).unwrap();
        tx.execute(
            "INSERT INTO source_managed_knowledge VALUES('source','node',1)",
            [],
        )
        .unwrap();
        tx.commit().unwrap();

        let focus = query_daily_focus(&c, &ws).unwrap();
        assert_eq!(focus.workspace_id, ws);
        assert_eq!(focus.workspace_name, "今日聚焦");
        assert_eq!(focus.display_name, "阿明");
        assert!(focus.has_activity);
        assert_eq!(focus.knowledge_count, 1);
        assert_eq!(focus.pending_memory_count, 1);
        let [episode, fallback] = focus.recent_episodes.as_slice() else {
            panic!(
                "expected two active episodes, got {:?}",
                focus.recent_episodes
            );
        };
        assert_eq!(episode.conversation_id, "chat");
        assert_eq!(episode.summary, "已确定发布顺序");
        assert_eq!(episode.updated_at, "2026-11-24");
        assert_eq!(fallback.conversation_id, "fallback");
        assert_eq!(fallback.summary, format!("下一步 {}", "测".repeat(76)));
        let [source] = focus.recent_sources.as_slice() else {
            panic!("expected one source, got {:?}", focus.recent_sources);
        };
        assert_eq!(source.source_id, "source");
        assert_eq!(source.node_id.as_deref(), Some("node"));
        assert_eq!(source.name, "项目说明.md");
        assert_eq!(source.version, 1);
        assert_eq!(source.updated_at, timestamp);
        let brief = focus.latest_brief.unwrap();
        assert_eq!(brief.title, "AI 行业");
        assert_eq!(brief.query, "AI 新闻");
        assert_eq!(brief.scheduled_at, "2023-11-14T22:13:20+00:00");
        assert_eq!(brief.headlines.len(), 3);
        assert_eq!(brief.headlines[0].title, "新闻 1");
        assert_eq!(brief.headlines[2].url, "https://example.com/3");
        let [reminder] = focus.upcoming_reminders.as_slice() else {
            panic!("expected one reminder, got {:?}", focus.upcoming_reminders);
        };
        assert_eq!(reminder.id, "reminder");
        assert_eq!(reminder.title, "AI 行业");
        assert_eq!(reminder.next_date, "2030-01-02");
        assert_eq!(reminder.time, "09:00");
        assert!(reminder.is_briefing);
        assert_eq!(
            focus.suggested_prompts,
            [
                "梳理近期更新的《项目说明.md》核心要点，并指出与现有资料的关联",
                "接着上次「发布计划」的讨论，帮我梳理下一步行动项",
                "结合最新一期「AI 行业」简报，分析值得跟进的重点动态",
            ]
        );
    }

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

    #[test]
    fn conflict_candidate_evolves_existing_knowledge_version_and_persona_item() {
        let db = Database::rag_evaluation_database().unwrap();
        let ws = db.create_workspace("记忆演进").unwrap().id;
        let conv = db.create_conversation(&ws).unwrap().id;
        let other_ws = db.create_workspace("其他工作区").unwrap().id;
        let other_conv = db.create_conversation(&other_ws).unwrap().id;
        let mut c = db.connection().unwrap();
        let original_fact = "项目数据库统一使用 SQLite";
        let original_preference = "我喜欢用 Rust 写桌面工具";
        let mut persona = GlobalPersona {
            preferences: vec![
                "使用中文".into(),
                original_preference.into(),
                "结论先行".into(),
            ],
            ..Default::default()
        };
        write_persona(&c, &mut persona).unwrap();

        let original = AiMemorySuggestion {
            scope: "workspace_fact".into(),
            title: "项目数据库".into(),
            content: original_fact.into(),
            ..Default::default()
        };
        assert_eq!(
            save_ai_memory_suggestions(&c, &ws, &conv, None, &[original]).unwrap(),
            1
        );
        let candidate_id = |c: &Connection, content: &str| -> String {
            c.query_row(
                "SELECT id FROM memory_candidates WHERE workspace_id=?1 AND content=?2 AND status='pending'",
                params![ws, content], |r| r.get(0),
            ).unwrap()
        };
        let initial_id = candidate_id(&c, original_fact);
        resolve_candidate(&mut c, &ws, &initial_id, "accept", None).unwrap();
        let node_id: String = c
            .query_row(
                "SELECT id FROM knowledge_nodes WHERE workspace_id=?1",
                [&ws],
                |r| r.get(0),
            )
            .unwrap();

        let new_fact = "目前项目数据库从 SQLite 改用 PostgreSQL";
        let new_preference = "我喜欢用 Go 写桌面工具";
        record_turn_insights(
            &c,
            &ws,
            &conv,
            None,
            "deterministic-message",
            &format!("{new_fact}。{new_preference}。"),
            "",
        )
        .unwrap();
        let conflict = |c: &Connection, content: &str| {
            c.query_row(
                "SELECT replaces_node_id,replaces_value,conflict_reason FROM memory_candidates WHERE id=?1",
                [candidate_id(c, content)],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            ).unwrap()
        };
        let (target, old, reason) = conflict(&c, new_fact);
        assert_eq!(target.as_deref(), Some(node_id.as_str()));
        assert_eq!(old, original_fact);
        assert_eq!(reason, "与既有记忆《项目数据库》存在更新或冲突");
        assert_eq!(conflict(&c, new_preference).1, original_preference);
        let personal_goal = extract_from_user_message("我目前大一，目标是准备考研");
        assert_eq!(personal_goal[0].scope, "global_preference");
        let mut explicit = extract_from_user_message("改用 PostgreSQL 替代 SQLite").remove(0);
        detect_conflict(&c, &ws, &persona, &mut explicit).unwrap();
        assert_eq!(explicit.replaces_node_id.as_deref(), Some(node_id.as_str()));

        let cloud_fact = "项目持久化使用 PostgreSQL";
        let cloud_preference = "桌面工具示例优先采用 Go";
        let suggestions = [
            AiMemorySuggestion {
                scope: "workspace_fact".into(),
                title: "云端候选标题".into(),
                content: cloud_fact.into(),
                replaces_node_id: Some(node_id.clone()),
                conflict_reason: Some("数据库迁移".into()),
                ..Default::default()
            },
            AiMemorySuggestion {
                scope: "global_preference".into(),
                title: "桌面工具语言".into(),
                content: cloud_preference.into(),
                replaces_value: Some(original_preference.into()),
                conflict_reason: Some("偏好改用 Go".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            save_ai_memory_suggestions(&c, &other_ws, &other_conv, None, &suggestions[..1])
                .unwrap(),
            0
        );
        assert_eq!(
            save_ai_memory_suggestions(&c, &ws, &conv, None, &suggestions).unwrap(),
            2
        );
        assert_eq!(
            save_ai_memory_suggestions(&c, &ws, &conv, None, &suggestions).unwrap(),
            0
        );
        assert_eq!(
            conflict(&c, cloud_fact),
            (
                Some(node_id.clone()),
                original_fact.into(),
                "数据库迁移".into()
            )
        );
        assert_eq!(conflict(&c, cloud_preference).1, original_preference);

        let fact_id = candidate_id(&c, cloud_fact);
        let preference_id = candidate_id(&c, cloud_preference);
        resolve_candidate(&mut c, &ws, &fact_id, "accept", None).unwrap();
        resolve_candidate(&mut c, &ws, &preference_id, "accept", None).unwrap();
        assert_eq!(
            read_persona(&c).unwrap().preferences,
            ["使用中文", cloud_preference, "结论先行"]
        );
        let current = c.query_row(
            "SELECT n.latest_version,n.effective_version,v.title,v.change_reason
             FROM knowledge_nodes n JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
             WHERE n.id=?1",
            [&node_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        ).unwrap();
        assert_eq!(
            current,
            (2, 2, "项目数据库".into(), "对话记忆演进更新".into())
        );
        let replaced: Option<i64> = c.query_row(
            "SELECT replaces_version FROM knowledge_version_lineage WHERE node_id=?1 AND version=2",
            [&node_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(replaced, Some(1));
        let report = crate::retrieval::search(&c, &ws, "PostgreSQL").unwrap();
        assert_eq!(report["candidates"][0]["nodeId"], node_id);
        assert_eq!(report["candidates"][0]["content"], cloud_fact);
        assert_eq!(
            crate::retrieval::search(&c, &ws, "SQLite").unwrap()["status"],
            "no_match"
        );

        let context = cloud_memory_context(&c, &ws).unwrap().unwrap();
        assert!(context.contains(&format!("[nodeId={node_id}] 项目数据库: {cloud_fact}")));
        c.execute(
            "INSERT INTO knowledge_local_only VALUES(?1,?2,1,?3)",
            params![ws, node_id, now()],
        )
        .unwrap();
        assert!(
            !cloud_memory_context(&c, &ws)
                .unwrap()
                .unwrap()
                .contains(&node_id)
        );
        c.execute(
            "INSERT INTO model_cloud_privacy VALUES(?1, '{\"syncMemoryAndHistory\":false}')",
            [&ws],
        )
        .unwrap();
        assert!(cloud_memory_context(&c, &ws).unwrap().is_none());
    }
}
