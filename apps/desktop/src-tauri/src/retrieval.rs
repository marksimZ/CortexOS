use crate::{db::Database, models::*};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const ALGORITHM: &str = "fts5-cjk-bigram-latin-bm25-v2";

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SearchMethod {
    None,
    Fts5,
    Fts5Rerank,
    Hybrid,
    HybridRerank,
    VersionAware,
}

impl SearchMethod {
    fn for_profile(profile: &crate::rag::Profile) -> Self {
        if profile.rerank {
            Self::HybridRerank
        } else {
            Self::Hybrid
        }
    }
}

pub(crate) struct Route {
    method: SearchMethod,
    reason: &'static str,
    pub profile: Option<crate::rag::Profile>,
    pub graph_reason: Option<&'static str>,
    runtime_profile: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum GraphMode {
    Auto,
    Always,
    Disabled,
}

impl From<bool> for GraphMode {
    fn from(force: bool) -> Self {
        if force { Self::Always } else { Self::Auto }
    }
}

impl Route {
    pub fn with_runtime(mut self, c: &Connection, workspace: &str, profile: Option<&crate::runtime_profiles::Profile>) -> AppResult<Self> {
        use crate::runtime_profiles::Target;
        let Some(profile) = profile else { return Ok(self) };
        self.runtime_profile = true;
        let reranks = !matches!(profile.input.reranking, Target::Off);
        if matches!(self.reason, "embedding_index" | "no_embedding_index") {
            let remote = match &profile.input.embedding {
                Target::Cloud { connection_id, model } => Some(crate::rag::cloud::usable(c,workspace,connection_id,model)?),
                _ => None,
            };
            self.profile = match remote { Some(index) => index, None => profile.select_local_embedding(self.profile.take()) };
            self.method = match (self.profile.is_some(),reranks) {
                (true, true) => SearchMethod::HybridRerank,
                (true, false) => SearchMethod::Hybrid,
                (false, true) => SearchMethod::Fts5Rerank,
                (false, false) => SearchMethod::Fts5,
            };
            self.reason = if self.profile.is_some() { "embedding_index" } else { "runtime_no_matching_index" };
        } else if self.method == SearchMethod::Fts5 && reranks {
            self.method = SearchMethod::Fts5Rerank;
        }
        Ok(self)
    }

    pub fn searches(&self) -> bool {
        self.method != SearchMethod::None
    }

    pub fn semantic<'a>(&self, query: Option<&'a crate::rag::Query>) -> Option<&'a crate::rag::Query> {
        match self.reason {
            "embedding_index" | "no_embedding_index" => query,
            _ => None,
        }
    }

    pub fn audit(&self, report: &Value, query: Option<&crate::rag::Query>, preview: bool) -> Value {
        let planned = if self.runtime_profile { self.method } else { query.map_or(self.method, |q| SearchMethod::for_profile(&q.profile)) };
        let used = match self.method {
            SearchMethod::None | SearchMethod::VersionAware => self.method,
            _ if report["reranker"].is_string() && report["semanticStatus"] == "completed" => SearchMethod::HybridRerank,
            _ if report["semanticStatus"] == "completed" => SearchMethod::Hybrid,
            _ if report["reranker"].is_string() => SearchMethod::Fts5Rerank,
            _ => SearchMethod::Fts5,
        };
        let fallback = (planned != used).then(|| {
            report["semanticStatus"].as_str().unwrap_or(if preview {
                "preview"
            } else {
                "no_query_vector"
            })
        });
        json!({"policyVersion":1,"plannedMethod":planned,"usedMethod":used,
            "reason":if query.is_some() { "embedding_index" } else { self.reason },
            "graphReason":self.graph_reason,"fallbackReason":fallback})
    }
}

pub(crate) fn route(
    c: &Connection,
    workspace: &str,
    text: &str,
    enabled: bool,
    scoped: bool,
    graph_mode: impl Into<GraphMode>,
    has_manual_selection: bool,
) -> AppResult<Route> {
    let mut plan = Route {
        method: SearchMethod::None,
        reason: "conversation_disabled",
        profile: None,
        graph_reason: None,
        runtime_profile: false,
    };
    if !enabled {
        return Ok(plan);
    }
    let text = text.trim().to_ascii_lowercase();
    // ponytail: expand the phrase list only against fixed evaluation cases.
    plan.graph_reason = match graph_mode.into() {
        GraphMode::Always => Some("user_enabled"),
        GraphMode::Auto if [
        "依赖链", "依赖路径", "关系路径", "关联路径", "冲突来源", "证据冲突",
        "dependency chain", "relationship path", "conflicting evidence",
    ]
        .into_iter()
        .any(|phrase| text.contains(phrase))
        => Some("relationship_question"),
        GraphMode::Auto | GraphMode::Disabled => None,
    };
    if scoped {
        plan.method = SearchMethod::VersionAware;
        plan.reason = "version_scope";
        return Ok(plan);
    }
    let standalone = text.trim_end_matches(['.', '!', '?', '。', '！', '？']).trim_end();
    if !has_manual_selection && plan.graph_reason.is_none() {
        if !text.chars().any(char::is_alphanumeric) {
            plan.reason = "no_searchable_text";
            return Ok(plan);
        }
        if matches!(
            standalone,
            "你好" | "您好" | "早上好" | "晚上好" | "谢谢" | "谢谢你" | "hi" | "hello" | "thanks" | "thank you"
        ) {
            plan.reason = "standalone_greeting";
            return Ok(plan);
        }
    }
    plan.method = SearchMethod::Fts5;
    let identifier = standalone.trim_matches(['`', '"', '\'']);
    let separators = b"-_/\\.:";
    if identifier.bytes().any(|b| b.is_ascii_alphabetic())
        && identifier.bytes().all(|b| b.is_ascii_alphanumeric() || separators.contains(&b))
        && identifier.bytes().any(|b| b.is_ascii_digit() || separators.contains(&b))
    {
        plan.reason = "identifier";
    } else if let Some(profile) = crate::rag::usable_profile(c, workspace)? {
        plan.method = SearchMethod::for_profile(&profile);
        plan.reason = "embedding_index";
        plan.profile = Some(profile);
    } else {
        plan.reason = "no_embedding_index";
    }
    Ok(plan)
}

fn error(_: rusqlite::Error) -> String {
    "知识检索不可用，问题已保留。请重试；也可关闭本会话自动检索。".into()
}
fn cjk(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x3134f | 0x3040..=0x30ff | 0xac00..=0xd7af)
}
// Lexical matching only: overlapping CJK pairs and lowercase alphanumeric words.
pub fn terms(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut previous = None;
    let mut latin = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if cjk(c) {
            if !latin.is_empty() {
                result.push(std::mem::take(&mut latin));
            }
            if let Some(p) = previous {
                result.push(format!("{p}{c}"));
            }
            previous = Some(c);
        } else {
            previous = None;
            if c.is_alphanumeric() {
                latin.extend(c.to_lowercase());
            } else if !latin.is_empty() {
                result.push(std::mem::take(&mut latin));
            }
        }
    }
    result
}
pub fn index_node(connection: &Connection, id: &str) -> AppResult<()> {
    connection
        .execute("DELETE FROM knowledge_fts WHERE node_id=?1", params![id])
        .map_err(error)?;
    let chunk_schema = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='rag_chunks')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(error)?;
    if chunk_schema {
        connection.execute(
            "DELETE FROM rag_chunks WHERE workspace_id=(SELECT workspace_id FROM knowledge_nodes WHERE id=?1) AND node_id=?1",
            [id]
        ).map_err(error)?;
    }
    let row: Option<(String,i64,String,String)>=connection.query_row("SELECT n.workspace_id,v.version,v.title,v.content FROM knowledge_nodes n JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version WHERE n.id=?1 AND v.kind IN ('knowledge','context','decision','evidence')",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(error)?;
    if let Some((workspace, version, title, body)) = row {
        if !crate::sources::available(connection, id, version)? {
            return Ok(());
        }
        let title_terms = terms(&title).join(" ");
        if chunk_schema {
            let evidence = crate::sources::evidence(connection, id, version)?;
            let segments = crate::source_chunks::knowledge_segments(
                &workspace, id, version, &body, evidence.first(),
            );
            if !segments.is_empty() {
                for segment in segments {
                    let location = serde_json::to_string(&segment.location)
                        .map_err(|_| "无法记录原文片段坐标。")?;
                    connection
                        .execute(
                            "INSERT INTO rag_chunks VALUES(?1,?2,?3,?4,?5,?6)",
                            params![workspace, id, version, segment.id, segment.body, location],
                        )
                        .map_err(error)?;
                    connection
                        .execute(
                            "INSERT INTO knowledge_fts VALUES(?1,?2,?3,?4,?5,?6)",
                            params![
                                id,
                                workspace,
                                version,
                                title_terms,
                                terms(segment.body).join(" "),
                                segment.id
                            ],
                        )
                        .map_err(error)?;
                }
                return Ok(());
            }
        }
        connection
            .execute(
                "INSERT INTO knowledge_fts(node_id,workspace_id,version,title_terms,body_terms) VALUES(?1,?2,?3,?4,?5)",
                params![
                    id,
                    workspace,
                    version,
                    title_terms,
                    terms(&body).join(" ")
                ],
            )
            .map_err(error)?;
    }
    Ok(())
}
pub fn rebuild(connection: &Connection) -> AppResult<()> {
    connection
        .execute("DELETE FROM knowledge_fts", [])
        .map_err(error)?;
    let ids = connection
        .prepare("SELECT id FROM knowledge_nodes WHERE effective_version IS NOT NULL")
        .map_err(error)?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    for id in ids {
        index_node(connection, &id)?;
    }
    Ok(())
}
pub fn enabled(connection: &Connection, workspace: &str, conversation: &str) -> AppResult<bool> {
    connection.query_row("SELECT COALESCE((SELECT enabled FROM conversation_retrieval_settings WHERE conversation_id=c.id AND workspace_id=c.workspace_id),1) FROM conversations c WHERE c.workspace_id=?1 AND c.id=?2 AND c.purpose='chat'",params![workspace,conversation],|r|r.get(0)).optional().map_err(error)?.ok_or_else(|| "会话不存在或不属于当前对话。".into())
}
impl Database {
    pub fn set_conversation_retrieval(
        &self,
        workspace: &str,
        conversation: &str,
        value: bool,
    ) -> AppResult<()> {
        let c = self.connection()?;
        let exists: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND id=?2 AND purpose='chat')",
                params![workspace, conversation],
                |r| r.get(0),
            )
            .map_err(error)?;
        if !exists {
            return Err("会话不属于当前工作区。".into());
        }
        c.execute("INSERT INTO conversation_retrieval_settings VALUES(?1,?2,?3) ON CONFLICT(conversation_id) DO UPDATE SET enabled=excluded.enabled",params![conversation,workspace,value]).map_err(error)?;
        Ok(())
    }
}
pub(crate) fn query_terms(query: &str) -> (Vec<String>, bool) {
    let mut seen = BTreeSet::new();
    let tokens: Vec<_> = terms(query)
        .into_iter()
        .filter(|x| {
            ![
                "什么", "如何", "我们", "你们", "请问", "一下", "怎么", "哪些", "the", "is", "a",
                "an", "of", "to", "and",
            ]
            .contains(&x.as_str())
        })
        .filter(|x| seen.insert(x.clone()))
        .collect();
    let truncated = tokens.len() > 64;
    (tokens.into_iter().take(64).collect(), truncated)
}

pub(crate) fn match_expression(tokens: &[String], operator: &str) -> String {
    tokens
        .iter()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(operator)
}

pub(crate) fn report(
    algorithm: &str,
    tokens: Vec<String>,
    truncated: bool,
    mut candidates: Vec<Value>,
    started: String,
    clock: std::time::Instant,
) -> Value {
    let capped = candidates.len() > 20;
    candidates.truncate(20);
    for (index, item) in candidates.iter_mut().enumerate() {
        item["lexicalRank"] = json!(index + 1);
    }
    let elapsed = clock.elapsed().as_micros() as u64;
    let mut report = json!({"enabled":true,"algorithm":algorithm,"terms":tokens,"queryTruncated":truncated,
        "candidateLimit":20,"candidatesTruncated":capped,"autoLimit":5,"startedAt":started,
        "finishedAt":now(),"elapsedMicros":elapsed,"stages":[],
        "status":if tokens.is_empty(){"no_search_terms"}else if candidates.is_empty(){"no_match"}else{"matched"},
        "candidates":candidates});
    let step = if tokens.is_empty() { "prepare_terms" }
        else if algorithm == ALGORITHM { "lexical_lookup" }
        else { "version_lookup" };
    stage(&mut report, step, elapsed);
    report
}

pub(crate) fn stage(report: &mut Value, kind: &str, elapsed_micros: u64) {
    report["stages"].as_array_mut().unwrap().push(json!({"kind":kind,"elapsedMicros":elapsed_micros}));
}

pub fn search(connection: &Connection, workspace: &str, query: &str) -> AppResult<Value> {
    search_with_privacy(connection, workspace, query, false)
}

pub(crate) fn search_with_privacy(
    connection: &Connection, workspace: &str, query: &str, exclude_local_only: bool,
) -> AppResult<Value> {
    let started = now();
    let clock = std::time::Instant::now();
    let (tokens, truncated) = query_terms(query);
    let expression = match_expression(&tokens, " OR ");
    let mut candidates = Vec::new();
    if !expression.is_empty() {
        let mut statement = connection.prepare(
            "WITH hits AS MATERIALIZED (
                SELECT f.rowid AS hit_id, n.id AS node_id, v.version, f.chunk_id,
                       bm25(knowledge_fts,0,0,0,5,1,0) AS score
                FROM knowledge_fts f
                JOIN knowledge_nodes n ON n.id=f.node_id AND n.workspace_id=f.workspace_id
                JOIN rag_current_knowledge v ON v.workspace_id=n.workspace_id AND v.node_id=n.id AND v.version=f.version
                WHERE knowledge_fts MATCH ?1 AND n.workspace_id=?2
                  AND (NOT ?3 OR NOT EXISTS(SELECT 1 FROM knowledge_local_only p
                       WHERE p.workspace_id=n.workspace_id AND p.node_id=n.id AND p.local_only=1))
                  AND NOT EXISTS(SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id
                                 WHERE l.node_id=n.id AND l.knowledge_version=v.version AND s.enabled=0)
             ), ranked AS (
                SELECT *, ROW_NUMBER() OVER(PARTITION BY node_id ORDER BY score,hit_id) AS rank
                FROM hits
             )
             SELECT h.node_id,v.version,v.title,v.kind,v.content,h.score,h.chunk_id,ch.body
             FROM ranked h
             JOIN knowledge_versions v ON v.node_id=h.node_id AND v.version=h.version
             LEFT JOIN rag_chunks ch ON ch.workspace_id=?2 AND ch.node_id=h.node_id AND ch.version=h.version AND ch.id=h.chunk_id
             WHERE h.rank=1 AND (h.chunk_id IS NULL OR ch.id IS NOT NULL)
             ORDER BY h.score,h.node_id LIMIT 21"
        ).map_err(error)?;
        candidates = statement.query_map(params![expression,workspace,exclude_local_only], |row| {
            let mut item = json!({"nodeId":row.get::<_,String>(0)?,"version":row.get::<_,i64>(1)?,"title":row.get::<_,String>(2)?,"kind":row.get::<_,String>(3)?,"content":row.get::<_,String>(4)?,"score":row.get::<_,f64>(5)?,"included":false,"reason":"pending_budget","costUnits":0});
            if let Some(chunk_id) = row.get::<_, Option<String>>(6)? {
                item["chunkId"] = json!(chunk_id);
                item["chunkText"] = json!(row.get::<_, String>(7)?);
            }
            Ok(item)
        }).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
    }
    Ok(report(
        ALGORITHM, tokens, truncated, candidates, started, clock,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::SaveKnowledge;
    fn draft(w: &str, title: &str, content: &str) -> SaveKnowledge {
        SaveKnowledge {
            workspace_id: w.into(),
            node_id: None,
            expected_revision: None,
            title: title.into(),
            kind: "knowledge".into(),
            content: content.into(),
            change_reason: "检索测试".into(),
            source_message_id: None,
            source_excerpt: None,
        }
    }
    fn approved(db: &Database, w: &str, title: &str, content: &str) -> String {
        let id = db.save_knowledge(draft(w, title, content)).unwrap();
        db.approve_knowledge(w, &id, 1).unwrap();
        id
    }
    fn setup() -> (std::path::PathBuf, Database, String, String) {
        let path = std::env::temp_dir().join(format!("cortex-retrieval-{}", new_id()));
        let db = Database::open(&path).unwrap();
        let w = db.create_workspace("检索验收").unwrap().id;
        let c = db.create_conversation(&w).unwrap().id;
        (path, db, w, c)
    }
    #[test]
    fn page_section_chunks_indexes_long_standalone_web_knowledge_and_retrieves_matching_section() {
        let db = Database::rag_evaluation_database().unwrap();
        let workspace = db.create_workspace("网页检索").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let title = "网页笔记";
        let overview = "背景资料".repeat(200);
        let filler = "实验说明".repeat(50);
        let section = format!("## 第二节 量子纠错\n{filler}\n稳定子测量\n{filler}");
        let content = format!("## 第一节 概览\n{overview}\n\n{section}");
        assert!(content.len() > 2048);
        let mut input = draft(&workspace, title, &content);
        input.kind = "evidence".into();
        let node = db.save_knowledge(input).unwrap();
        {
            let connection = db.connection().unwrap();
            assert!(crate::sources::evidence(&connection, &node, 1).unwrap().is_empty());
            let indexed: (i64, i64, String) = connection.query_row(
                "SELECT (SELECT count(*) FROM rag_chunks WHERE node_id=?1), count(*),
                        max(CASE WHEN ch.body LIKE '## 第二节 量子纠错%' THEN ch.id END)
                 FROM knowledge_fts f
                 JOIN rag_chunks ch ON ch.workspace_id=f.workspace_id AND ch.node_id=f.node_id
                     AND ch.version=f.version AND ch.id=f.chunk_id
                 WHERE f.node_id=?1",
                [&node],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).unwrap();
            assert!(indexed.0 > 2);
            assert_eq!(indexed.0, indexed.1);
            let result = search(&connection, &workspace, "量子纠错").unwrap();
            let hit = &result["candidates"][0];
            assert_eq!(hit["nodeId"], node);
            assert_eq!(hit["kind"], "evidence");
            assert_eq!(hit["chunkId"], indexed.2);
            assert!(indexed.2.starts_with(&format!("node/{workspace}/{node}/1/child/")));
            assert!(hit["chunkText"].as_str().unwrap().starts_with("## 第二节 量子纠错"));
        }

        let settings = ModelSettings {
            num_ctx: 8192,
            ..Default::default()
        };
        let snapshot = json!(db.capture_knowledge_snapshot(&workspace, "章节快照").unwrap());
        let scope = crate::knowledge_scope::Request::Snapshot {
            snapshot_id: snapshot["id"].as_str().unwrap().into(),
        };
        for scope in [None, Some(&scope)] {
            let prepared = db.prepare_context_run_with_retrieval(
                &new_id(), &workspace, &chat, "量子纠错", &settings, &[], true, None, scope, false,
            ).unwrap();
            let reference = &prepared.manifest["knowledgeRefs"][0];
            assert_eq!(reference["nodeId"], node);
            assert_eq!(reference["content"], section);
            assert_eq!(reference["sourceEvidence"], json!([]));
            let chunk = &reference["sourceChunk"];
            assert_eq!(chunk["sourceId"], "");
            assert_eq!(chunk["documentVersionId"], 1);
            assert_eq!(chunk["sourcePath"], title);
            assert_eq!(chunk["sectionPath"], json!(["第二节 量子纠错"]));
            assert_eq!(chunk["expanded"], true);
            assert_eq!(chunk["matchStartLine"], 4);
            assert_eq!(chunk["matchEndLine"], 6);
            let request = prepared.request.to_string();
            assert!(request.contains("稳定子测量"));
            assert!(!request.contains("第一节 概览"));
        }

        let limited = db.prepare_context_run(
            &new_id(), &workspace, &chat, "量子纠错", &ModelSettings::default(), &[], true,
        ).unwrap();
        let reference = &limited.manifest["knowledgeRefs"][0];
        assert_eq!(
            reference["content"],
            format!("## 第二节 量子纠错\n{filler}\n稳定子测量\n")
        );
        assert_eq!(reference["sourceChunk"]["expanded"], false);
        assert_eq!(reference["sourceChunk"]["sectionPath"], json!(["第二节 量子纠错"]));
    }

    #[test]
    fn tokenizer_comparison_and_fixed_mixed_language_queries() {
        let (path, db, w, _) = setup();
        {
            let conn = db.connection().unwrap();
            conn.execute_batch("CREATE VIRTUAL TABLE default_words USING fts5(body); CREATE VIRTUAL TABLE trigrams USING fts5(body,tokenize='trigram'); INSERT INTO default_words VALUES('项目发布计划'); INSERT INTO trigrams VALUES('项目发布计划');").unwrap();
            for table in ["default_words", "trigrams"] {
                let count: i64 = conn
                    .query_row(
                        &format!("SELECT count(*) FROM {table} WHERE {table} MATCH '发布'"),
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(count, 0);
            }
        }
        let release = approved(&db, &w, "发布计划", "项目发布定于周四。");
        let backup = approved(&db, &w, "数据库备份", "SQLite 数据库备份采用一致性快照。");
        let cancel = approved(&db, &w, "取消生成", "取消生成后保留部分回答。");
        let gpu = approved(&db, &w, "显存预算", "GPU 显存不足时减小上下文。");
        let code = approved(&db, &w, "HTTP超时", "reqwest timeout 参数控制请求超时。");
        let auth = approved(&db, &w, "API鉴权", "API_AUTH_TOKEN 只在本地配置。");
        let queries = [
            ("发布", &release),
            ("项目什么时候发布", &release),
            ("SQLite如何备份", &backup),
            ("备份", &backup),
            ("怎么取消生成", &cancel),
            ("GPU显存不足", &gpu),
            ("reqwest timeout", &code),
            ("API_AUTH_TOKEN", &auth),
        ];
        for (q, id) in queries {
            let report = search(&db.connection().unwrap(), &w, q).unwrap();
            assert_eq!(report["candidates"][0]["nodeId"], *id, "query: {q}");
        }
        let no = search(&db.connection().unwrap(), &w, "火星香蕉").unwrap();
        assert_eq!(no["status"], "no_match");
        let empty = search(&db.connection().unwrap(), &w, "? OR \" ***").unwrap();
        assert!(empty["candidates"].as_array().unwrap().is_empty());
        let single = search(&db.connection().unwrap(), &w, "发").unwrap();
        assert_eq!(single["status"], "no_search_terms");
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn retrieval_snapshot_approval_budget_preferences_and_rollback() {
        let (path, db, w, c) = setup();
        let other = db.create_workspace("隔离").unwrap().id;
        let id = approved(&db, &w, "发布计划", "周一发布。");
        approved(&db, &other, "发布计划", "跨工作区内容。");
        db.save_knowledge(draft(&w, "发布草案", "未确认发布内容。"))
            .unwrap();
        let mut edit = draft(&w, "发布计划", "周二发布。");
        edit.node_id = Some(id.clone());
        edit.expected_revision = Some(2);
        db.save_knowledge(edit).unwrap();
        let settings = ModelSettings::default();
        let old = db
            .prepare_context_run(&new_id(), &w, &c, "发布", &settings, &[], false)
            .unwrap();
        assert_eq!(
            old.manifest["retrieval"]["candidates"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(old.manifest["knowledgeRefs"][0]["version"], 1);
        assert!(old.request.to_string().contains("周一"));
        assert!(!old.request.to_string().contains("周二"));
        let first = db.run_detail(&w, &old.run.id).unwrap();
        assert_eq!(
            first
                .events
                .iter()
                .map(|e| e.kind.as_str())
                .collect::<Vec<_>>(),
            [
                "request_received",
                "retrieval_started",
                "retrieval_completed",
                "context_prepared"
            ]
        );
        assert!(
            first
                .events
                .windows(2)
                .all(|p| p[0].occurred_at <= p[1].occurred_at)
        );
        db.finish(&old.run, "周一", "completed", None, json!({}))
            .unwrap();
        db.approve_knowledge(&w, &id, 3).unwrap();
        let next = db
            .prepare_context_run(&new_id(), &w, &c, "发布", &settings, &[], true)
            .unwrap();
        assert_eq!(next.manifest["knowledgeRefs"][0]["version"], 2);
        let manual = db
            .prepare_context_run(
                &new_id(),
                &w,
                &c,
                "发布",
                &settings,
                &[ContextSelection {
                    node_id: id.clone(),
                    version: 2,
                }],
                true,
            )
            .unwrap();
        assert_eq!(
            manual.manifest["knowledgeRefs"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            manual.manifest["retrieval"]["candidates"][0]["reason"],
            "manual_selected"
        );
        let big = approved(&db, &w, "长文档", &"长文档资料".repeat(1000));
        let budget = db
            .prepare_context_run(&new_id(), &w, &c, "长文档", &settings, &[], true)
            .unwrap();
        assert_eq!(
            budget.manifest["retrieval"]["candidates"][0]["reason"],
            "auto_budget"
        );
        assert!(
            budget.manifest["knowledgeRefs"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            db.prepare_context_run(
                &new_id(),
                &w,
                &c,
                "长文档",
                &settings,
                &[ContextSelection {
                    node_id: big,
                    version: 1
                }],
                false
            )
            .is_err()
        );
        assert!(db.set_conversation_retrieval(&other, &c, false).is_err());
        db.set_conversation_retrieval(&w, &c, false).unwrap();
        let disabled = db
            .prepare_context_run(&new_id(), &w, &c, "发布", &settings, &[], false)
            .unwrap();
        assert_eq!(disabled.manifest["retrievalUsed"], false);
        assert_eq!(db.run_detail(&w, &disabled.run.id).unwrap().events.len(), 2);
        db.finish(&disabled.run, "", "cancelled", None, json!({}))
            .unwrap();
        db.set_conversation_retrieval(&w, &c, true).unwrap();
        let count: i64 = db
            .connection()
            .unwrap()
            .query_row("SELECT count(*) FROM execution_runs", [], |r| r.get(0))
            .unwrap();
        db.connection().unwrap().execute_batch("CREATE TRIGGER fail_retrieval BEFORE INSERT ON retrieval_results BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        assert!(
            db.prepare_context_run(&new_id(), &w, &c, "发布", &settings, &[], false)
                .is_err()
        );
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row("SELECT count(*) FROM execution_runs", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            count
        );
        db.connection()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_retrieval;")
            .unwrap();
        assert!(
            db.connection()
                .unwrap()
                .execute("UPDATE retrieval_results SET reason='fake'", [])
                .is_err()
        );
        assert!(
            db.connection()
                .unwrap()
                .execute("DELETE FROM retrieval_results", [])
                .is_err()
        );
        // Approval and FTS update are one transaction.
        let mut edit = draft(&w, "发布计划", "周三发布。");
        edit.node_id = Some(id.clone());
        edit.expected_revision = Some(4);
        db.save_knowledge(edit).unwrap();
        db.connection()
            .unwrap()
            .execute_batch("DROP TABLE knowledge_fts;")
            .unwrap();
        assert!(db.approve_knowledge(&w, &id, 5).is_err());
        assert_eq!(
            db.knowledge_detail(&w, &id).unwrap().node.effective_version,
            Some(2)
        );
        db.connection().unwrap().execute_batch("CREATE VIRTUAL TABLE knowledge_fts USING fts5(node_id UNINDEXED,workspace_id UNINDEXED,version UNINDEXED,title_terms,body_terms,chunk_id UNINDEXED,tokenize='unicode61');").unwrap();
        rebuild(&db.connection().unwrap()).unwrap();
        db.set_conversation_retrieval(&w, &c, false).unwrap();
        drop(db);
        let reopened = Database::open(&path).unwrap();
        assert!(!reopened.conversations(&w).unwrap()[0].auto_retrieval);
        assert_eq!(
            reopened
                .run_detail(&w, &old.run.id)
                .unwrap()
                .snapshot
                .unwrap()
                .request,
            old.request
        );
        assert!(
            reopened
                .connection()
                .unwrap()
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none()
        );
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn v5_upgrade_backfills_effective_versions_and_limits_candidates() {
        let (path, db, w, c) = setup();
        for i in 0..26 {
            approved(&db, &w, &format!("发布资料{i}"), "发布。");
        }
        let settings = ModelSettings {
            num_ctx: 16384,
            ..Default::default()
        };
        let preview = db
            .prepare_context_run(&new_id(), &w, &c, "发布", &settings, &[], true)
            .unwrap();
        assert_eq!(
            preview.manifest["retrieval"]["candidates"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        assert_eq!(preview.manifest["retrieval"]["candidatesTruncated"], true);
        assert_eq!(preview.manifest["retrieval"]["includedCount"], 5);
        assert_eq!(
            preview.manifest["retrieval"]["candidates"][5]["reason"],
            "auto_limit"
        );
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row("SELECT count(*) FROM retrieval_results", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(db);
        let legacy = path.join("legacy-v5");
        std::fs::create_dir_all(&legacy).unwrap();
        let mut connection = Connection::open(legacy.join("cortexos.sqlite3")).unwrap();
        crate::db::initialize_fixture_schema(&mut connection, &legacy, 5).unwrap();
        connection
            .execute("INSERT INTO workspaces VALUES(?1,'旧工作区','t','t')", [&w])
            .unwrap();
        connection
            .execute(
                "INSERT INTO conversations VALUES(?1,?2,'旧会话','t','t')",
                params![c, w],
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        for i in 0..26 {
            let node = new_id();
            transaction.execute("INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at) VALUES(?1,?2,1,1,2,'t','t')",params![node,w]).unwrap();
            transaction.execute("INSERT INTO knowledge_versions VALUES(?1,1,?2,'knowledge','发布。','user','fixture','t')",params![node,format!("发布资料{i}")]).unwrap();
        }
        transaction.commit().unwrap();
        drop(connection);
        let migrated = Database::open(&legacy).unwrap();
        assert!(migrated.conversations(&w).unwrap()[0].auto_retrieval);
        assert_eq!(
            search(&migrated.connection().unwrap(), &w, "发布").unwrap()["candidates"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        assert!(std::fs::read_dir(&legacy).unwrap().any(|f| {
            f.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("before-migration-6-")
        }));
        assert_eq!(
            migrated
                .connection()
                .unwrap()
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        drop(migrated);
        std::fs::remove_dir_all(path).unwrap();
    }
}
