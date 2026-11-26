use crate::{
    commands::{AppState, blocking},
    db::{Database, error},
    models::{AppResult, ContextSelection, new_id, now, validate_name},
    retrieval, source_chunks,
};
use chrono::{Datelike, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Instant};
use tauri::State;

pub(crate) const SUPPORTED: &str = "v.kind IN ('knowledge','context','decision','evidence')";
pub(crate) const SOURCE_AVAILABLE: &str = "NOT EXISTS(SELECT 1 FROM knowledge_source_links l
    JOIN source_documents s ON s.id=l.source_id
    WHERE l.node_id=v.node_id AND l.knowledge_version=v.version AND s.enabled=0)";

#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Validity {
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

pub(crate) fn check_date(value: &str) -> AppResult<()> {
    if value.len() == 10
        && let Ok(parsed) = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        && parsed.year() > 0
        && parsed.to_string() == value
    {
        return Ok(());
    }
    Err("日期须使用有效的年-月-日。".into())
}

impl Validity {
    pub(crate) fn validate(&self) -> AppResult<()> {
        for date in [&self.valid_from, &self.valid_to].into_iter().flatten() {
            check_date(date)?;
        }
        if let (Some(from), Some(to)) = (&self.valid_from, &self.valid_to)
            && from >= to
        {
            return Err("失效日期须晚于生效日期。".into());
        }
        Ok(())
    }

    pub(crate) fn is_known(&self) -> bool {
        self.valid_from.is_some() || self.valid_to.is_some()
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    Snapshot { snapshot_id: String },
    Branch { snapshot_id: String },
    FactDate { date: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    id: String,
    name: String,
    created_at: String,
    fact_date: String,
    item_count: i64,
    base_snapshot_id: Option<String>,
    overrides: Option<Value>,
}

fn snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<Snapshot> {
    Ok(Snapshot {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        fact_date: row.get(3)?,
        item_count: row.get(4)?,
        base_snapshot_id: row.get(5)?,
        overrides: row
            .get::<_, Option<String>>(6)?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|cause| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(cause),
                )
            })?,
    })
}

const SNAPSHOTS: &str = "SELECT s.id,s.name,s.created_at,s.fact_date,s.item_count,b.base_snapshot_id,b.overrides_json
    FROM knowledge_snapshots s LEFT JOIN knowledge_experimental_branches b ON b.workspace_id=s.workspace_id AND b.snapshot_id=s.id";

fn get_snapshot(c: &Connection, workspace: &str, id: &str) -> AppResult<Snapshot> {
    c.query_row(
        &format!("{SNAPSHOTS} WHERE s.workspace_id=?1 AND s.id=?2"),
        params![workspace, id],
        snapshot,
    )
    .optional()
    .map_err(error)?
    .ok_or_else(|| "知识快照不存在或不属于当前工作区。".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMember {
    node_id: String,
    title: String,
    version: i64,
}

fn store_snapshot(
    tx: &Connection,
    workspace: &str,
    value: &Snapshot,
    selection: &[(String, i64)],
) -> AppResult<()> {
    tx.execute(
        "INSERT INTO knowledge_snapshots VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            workspace,
            value.id,
            value.name,
            value.created_at,
            value.fact_date,
            value.item_count
        ],
    )
    .map_err(error)?;
    let mut insert = tx
        .prepare("INSERT INTO knowledge_snapshot_items VALUES(?1,?2,?3,?4,?5)")
        .map_err(error)?;
    for (i, (node, version)) in selection.iter().enumerate() {
        insert
            .execute(params![workspace, value.id, node, version, i as i64 + 1])
            .map_err(error)?;
    }
    if let Some(base) = &value.base_snapshot_id {
        tx.execute(
            "INSERT INTO knowledge_experimental_branches VALUES(?1,?2,?3,?4)",
            params![
                workspace,
                value.id,
                base,
                value.overrides.as_ref().unwrap().to_string()
            ],
        )
        .map_err(error)?;
        tx.execute("INSERT INTO knowledge_snapshot_relations
            SELECT workspace_id,?2,edge_id,edge_version FROM knowledge_snapshot_relations WHERE workspace_id=?1 AND snapshot_id=?3",
            params![workspace,value.id,base]).map_err(error)?;
    } else {
        tx.execute("INSERT INTO knowledge_snapshot_relations
            SELECT e.workspace_id,?2,e.id,v.version FROM knowledge_edges e
            JOIN knowledge_edge_versions v ON v.edge_id=e.id AND v.workspace_id=e.workspace_id AND v.version=e.latest_version
            JOIN knowledge_snapshot_items a ON a.workspace_id=e.workspace_id AND a.snapshot_id=?2 AND a.node_id=v.from_node_id
            JOIN knowledge_snapshot_items b ON b.workspace_id=e.workspace_id AND b.snapshot_id=?2 AND b.node_id=v.to_node_id
            WHERE e.workspace_id=?1 AND v.active=1",
            params![workspace,value.id]).map_err(error)?;
    }
    Ok(())
}

impl Database {
    fn knowledge_snapshots(&self, workspace: &str) -> AppResult<Vec<Snapshot>> {
        let c = self.connection()?;
        crate::benchmark::dataset::require_workspace(&c, workspace)?;
        c.prepare(&format!(
            "{SNAPSHOTS} WHERE s.workspace_id=?1 ORDER BY s.created_at DESC,s.id"
        ))
        .map_err(error)?
        .query_map([workspace], snapshot)
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)
    }

    pub(crate) fn capture_knowledge_snapshot(
        &self,
        workspace: &str,
        name: &str,
    ) -> AppResult<Snapshot> {
        let name = validate_name(name)?;
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        crate::benchmark::dataset::require_workspace(&tx, workspace)?;
        let fact_date: String = tx
            .query_row("SELECT date('now','localtime')", [], |row| row.get(0))
            .map_err(error)?;
        let selection = versions(
            &tx,
            &format!(
                "SELECT v.node_id,v.version FROM rag_current_knowledge v WHERE v.workspace_id=?1 AND {SOURCE_AVAILABLE} ORDER BY v.node_id"
            ),
            workspace,
            "",
        )?;
        if selection.is_empty() {
            return Err("当前没有可用于回答的知识。请先确认知识并检查生效日期。".into());
        }
        let value = Snapshot {
            id: new_id(),
            name,
            created_at: now(),
            fact_date,
            item_count: selection.len() as i64,
            base_snapshot_id: None,
            overrides: None,
        };
        store_snapshot(&tx, workspace, &value, &selection)?;
        tx.commit().map_err(error)?;
        Ok(value)
    }

    fn knowledge_snapshot_members(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Vec<SnapshotMember>> {
        let c = self.connection()?;
        get_snapshot(&c, workspace, id)?;
        c.prepare(
            "SELECT m.node_id,v.title,v.version FROM knowledge_snapshot_items m
            JOIN knowledge_versions v ON v.node_id=m.node_id AND v.version=m.version
            WHERE m.workspace_id=?1 AND m.snapshot_id=?2 ORDER BY v.title,m.node_id",
        )
        .map_err(error)?
        .query_map(params![workspace, id], |r| {
            Ok(SnapshotMember {
                node_id: r.get(0)?,
                title: r.get(1)?,
                version: r.get(2)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)
    }

    pub(crate) fn create_knowledge_branch(
        &self,
        workspace: &str,
        base_id: &str,
        name: &str,
        overrides: &[ContextSelection],
    ) -> AppResult<Snapshot> {
        let name = validate_name(name)?;
        if overrides.is_empty() || overrides.len() > 20 {
            return Err("每次可为 1–20 项知识指定实验版本。".into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let base = get_snapshot(&tx, workspace, base_id)?;
        let mut members = versions(
            &tx,
            "SELECT node_id,version FROM knowledge_snapshot_items WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY node_id",
            workspace,
            base_id,
        )?;
        let mut seen = BTreeSet::new();
        let mut changes = Vec::new();
        for selected in overrides {
            if !seen.insert(&selected.node_id) {
                return Err("同一项知识只能选择一个实验版本。".into());
            }
            let index = members
                .binary_search_by(|(id, _)| id.cmp(&selected.node_id))
                .map_err(|_| "实验版本须属于基准快照中的知识。")?;
            let old_version = members[index].1;
            if old_version == selected.version {
                return Err("请选择与基准不同的知识版本。".into());
            }
            let approved_at: Option<String> = tx.query_row(
                &format!("SELECT a.created_at FROM knowledge_nodes n
                 JOIN knowledge_versions v ON v.node_id=n.id AND v.version=?3
                 LEFT JOIN knowledge_approval_events a ON a.node_id=v.node_id AND a.version=v.version
                 WHERE n.workspace_id=?1 AND n.id=?2 AND {SUPPORTED} AND {SOURCE_AVAILABLE}"),
                params![workspace,selected.node_id,selected.version],|row|row.get(0)
            ).optional().map_err(error)?.ok_or("实验版本不存在、类型不适用，或来源已停用。")?;
            members[index].1 = selected.version;
            changes.push(json!({"nodeId":selected.node_id,"baseVersion":old_version,"version":selected.version,"approvedAtCapture":approved_at}));
        }
        let value = Snapshot {
            id: new_id(),
            name,
            created_at: now(),
            fact_date: base.fact_date,
            item_count: members.len() as i64,
            base_snapshot_id: Some(base.id),
            overrides: Some(json!(changes)),
        };
        store_snapshot(&tx, workspace, &value, &members)?;
        tx.commit().map_err(error)?;
        Ok(value)
    }
}

#[tauri::command]
pub async fn list_knowledge_snapshots(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<Snapshot>> {
    blocking(state.database.clone()?, move |db| {
        db.knowledge_snapshots(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn capture_knowledge_snapshot(
    state: State<'_, AppState>,
    workspace_id: String,
    name: String,
) -> AppResult<Snapshot> {
    blocking(state.database.clone()?, move |db| {
        db.capture_knowledge_snapshot(&workspace_id, &name)
    })
    .await
}

#[tauri::command]
pub async fn list_knowledge_snapshot_members(
    state: State<'_, AppState>,
    workspace_id: String,
    snapshot_id: String,
) -> AppResult<Vec<SnapshotMember>> {
    blocking(state.database.clone()?, move |db| {
        db.knowledge_snapshot_members(&workspace_id, &snapshot_id)
    })
    .await
}

#[tauri::command]
pub async fn create_knowledge_branch(
    state: State<'_, AppState>,
    workspace_id: String,
    base_snapshot_id: String,
    name: String,
    overrides: Vec<ContextSelection>,
) -> AppResult<Snapshot> {
    blocking(state.database.clone()?, move |db| {
        db.create_knowledge_branch(&workspace_id, &base_snapshot_id, &name, &overrides)
    })
    .await
}

fn versions(
    c: &Connection,
    sql: &str,
    workspace: &str,
    scope: &str,
) -> AppResult<Vec<(String, i64)>> {
    let mut statement = c.prepare(sql).map_err(error)?;
    let args = [workspace, scope];
    let count = statement.parameter_count();
    statement
        .query_map(rusqlite::params_from_iter(&args[..count]), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)
}

pub(crate) struct Resolved {
    key: String,
    pub audit: Value,
    versions: Vec<(String, i64)>,
}

impl Resolved {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn contains(&self, node: &str, version: i64) -> bool {
        self.versions
            .binary_search_by(|(id, v)| id.as_str().cmp(node).then(v.cmp(&version)))
            .is_ok()
    }

    pub(crate) fn versions(&self) -> &[(String, i64)] {
        &self.versions
    }
}

pub(crate) fn resolve(c: &Connection, workspace: &str, request: &Request) -> AppResult<Resolved> {
    crate::benchmark::dataset::require_workspace(c, workspace)?;
    let (mut audit, selection) = match request {
        Request::Snapshot { snapshot_id } | Request::Branch { snapshot_id } => {
            let snap = get_snapshot(c, workspace, snapshot_id)?;
            if matches!(request, Request::Branch { .. }) != snap.base_snapshot_id.is_some() {
                return Err("所选资料范围已变化，请重新选择知识快照或实验分支。".into());
            }
            let selected = versions(
                c,
                &format!(
                    "SELECT v.node_id,v.version FROM knowledge_snapshot_items v WHERE v.workspace_id=?1 AND v.snapshot_id=?2 AND {SOURCE_AVAILABLE} ORDER BY v.node_id"
                ),
                workspace,
                snapshot_id,
            )?;
            let blocked = snap.item_count - selected.len() as i64;
            let mut audit = json!({"kind":"snapshot","snapshotId":snap.id,"name":snap.name,"savedAt":snap.created_at,"factDate":snap.fact_date,"unavailableCount":blocked});
            if let Some(base) = snap.base_snapshot_id {
                audit["kind"] = json!("branch");
                audit["baseSnapshotId"] = json!(base);
                audit["overrides"] = snap.overrides.unwrap_or_default();
            }
            (audit, selected)
        }
        Request::FactDate { date } => {
            check_date(date)?;
            let selected = versions(c,&format!(
                "WITH valid_versions AS (
                    SELECT v.node_id,v.version,d.valid_to,ROW_NUMBER() OVER (PARTITION BY v.node_id ORDER BY v.version DESC) AS rank
                    FROM knowledge_nodes n JOIN knowledge_versions v ON v.node_id=n.id
                    JOIN knowledge_approval_events a ON a.node_id=v.node_id AND a.version=v.version
                    JOIN knowledge_validity d ON d.node_id=v.node_id AND d.version=v.version
                    WHERE n.workspace_id=?1 AND {SUPPORTED}
                    AND (d.valid_from IS NULL OR d.valid_from<=?2))
                 SELECT v.node_id,v.version FROM valid_versions v
                 WHERE v.rank=1 AND (v.valid_to IS NULL OR v.valid_to>?2) AND {SOURCE_AVAILABLE} ORDER BY v.node_id"
            ),workspace,date)?;
            let unknown: i64 = c.query_row(&format!(
                "SELECT count(*) FROM knowledge_nodes n JOIN knowledge_versions v ON v.node_id=n.id
                 JOIN knowledge_approval_events a ON a.node_id=v.node_id AND a.version=v.version
                 WHERE n.workspace_id=?1 AND {SUPPORTED} AND {SOURCE_AVAILABLE}
                 AND NOT EXISTS(SELECT 1 FROM knowledge_validity d WHERE d.node_id=v.node_id AND d.version=v.version)"
            ),[workspace],|row| row.get(0)).map_err(error)?;
            (
                json!({"kind":"factDate","factDate":date,"unknownDateCount":unknown}),
                selected,
            )
        }
    };
    let key = crate::benchmark::dataset::hash(json!([audit, selection]).to_string().as_bytes());
    audit["key"] = json!(key);
    audit["eligibleCount"] = json!(selection.len());
    Ok(Resolved {
        key,
        audit,
        versions: selection,
    })
}

pub(crate) fn search(
    c: &Connection,
    workspace: &str,
    query: &str,
    scope: &Resolved,
) -> AppResult<Value> {
    let started = now();
    let clock = Instant::now();
    let (terms, truncated) = retrieval::query_terms(query);
    let keywords: BTreeSet<&str> = terms.iter().map(String::as_str).collect();
    let score = |text: &str| -> usize {
        retrieval::terms(text)
            .into_iter()
            .filter(|term| keywords.contains(term.as_str()))
            .collect::<BTreeSet<_>>()
            .len()
    };
    let mut hits = Vec::<(usize, String, Value)>::new();
    // ponytail: explicit scopes rank their frozen/dated set in memory and keep only 21 hits.
    // A persistent historical index would avoid the scan for measured large-workspace use.
    if !terms.is_empty() {
        let refs = serde_json::to_string(&scope.versions).map_err(|_| "无法读取知识版本范围。")?;
        let mut statement = c.prepare(
            "SELECT v.node_id,v.version,v.title,v.kind,v.content,l.source_id
             FROM json_each(?2) item
             JOIN knowledge_nodes n ON n.workspace_id=?1 AND n.id=json_extract(item.value,'$[0]')
             JOIN knowledge_versions v ON v.node_id=n.id AND v.version=json_extract(item.value,'$[1]')
             LEFT JOIN knowledge_source_links l ON l.node_id=n.id AND l.knowledge_version=v.version
             ORDER BY n.id"
        ).map_err(error)?;
        let mut rows = statement.query(params![workspace, refs]).map_err(error)?;
        while let Some(row) = rows.next().map_err(error)? {
            let id: String = row.get(0).map_err(error)?;
            let version: i64 = row.get(1).map_err(error)?;
            let title: String = row.get(2).map_err(error)?;
            let body: String = row.get(4).map_err(error)?;
            let title_score = 5 * score(&title);
            let full_score = score(&body);
            if title_score + full_score == 0 {
                continue;
            }
            let kind: String = row.get(3).map_err(error)?;
            let source: Option<String> = row.get(5).map_err(error)?;
            let mut candidate = json!({"nodeId":id,"version":version,"title":title,"kind":kind,"content":body,"included":false,"reason":"pending_budget","costUnits":0});
            let mut body_score = full_score;
            let evidence = if source.is_some() {
                crate::sources::evidence(c, &id, version)?
            } else {
                Vec::new()
            };
            if let Some((matched, segment)) =
                source_chunks::knowledge_segments(workspace, &id, version, &body, evidence.first())
                    .into_iter()
                    .map(|segment| (score(segment.body), segment))
                    .max_by_key(|(score, _)| *score)
                && title_score + matched > 0
            {
                body_score = matched;
                candidate["chunkId"] = json!(segment.id);
                candidate["chunkLocation"] = json!(segment.location);
            }
            let total = title_score + body_score;
            candidate["score"] = json!(total);
            hits.push((total, id, candidate));
            hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            hits.truncate(21);
        }
    }
    Ok(retrieval::report(
        "scoped-term-coverage-v1",
        terms,
        truncated,
        hits.into_iter().map(|(_, _, item)| item).collect(),
        started,
        clock,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        knowledge::SaveKnowledge,
        models::{ContextSelection, ModelSettings},
        sources::{PendingSource, SourcePreview, SourceRef},
    };

    #[test]
    fn knowledge_scope_branch_and_conflicts_remain_isolated() {
        let path = std::env::temp_dir().join(format!("cortex-knowledge-branch-{}", new_id()));
        let db = Database::open(&path).unwrap();
        let workspace = db.create_workspace("实验").unwrap().id;
        let other = db.create_workspace("隔离").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let draft = |title: &str, content: &str| SaveKnowledge {
            workspace_id: workspace.clone(),
            node_id: None,
            expected_revision: None,
            title: title.into(),
            kind: "knowledge".into(),
            content: content.into(),
            change_reason: "验收".into(),
            source_message_id: None,
            source_excerpt: None,
        };
        let approved = |title, content| {
            let id = db.save_knowledge(draft(title, content)).unwrap();
            db.approve_knowledge(&workspace, &id, 1).unwrap();
            id
        };
        let primary = approved("发布地点", "releasecode 东楼正式地点");
        let counter = approved("例外通知", "南楼反驳资料");
        let unrelated = approved("二跳通知", "二跳无关材料");
        let edge = crate::graph::SaveEdge {
            workspace_id: workspace.clone(),
            edge_id: None,
            expected_version: None,
            from_node_id: primary.clone(),
            to_node_id: counter.clone(),
            relation: "contradicts".into(),
            active: true,
            change_reason: "登记反驳".into(),
        };
        let edge_id = db.save_knowledge_edge(edge.clone()).unwrap();
        db.save_knowledge_edge(crate::graph::SaveEdge {
            from_node_id: counter.clone(),
            to_node_id: unrelated.clone(),
            ..edge.clone()
        })
        .unwrap();
        let base = db
            .capture_knowledge_snapshot(&workspace, "正式基准")
            .unwrap();
        db.save_knowledge(SaveKnowledge {
            node_id: Some(primary.clone()),
            expected_revision: Some(2),
            ..draft("发布地点", "releasecode 西楼实验地点")
        })
        .unwrap();
        let change = ContextSelection {
            node_id: primary.clone(),
            version: 2,
        };
        let current = db.knowledge_detail(&workspace, &primary).unwrap();
        assert_eq!(current.node.effective_version, Some(1));
        assert!(
            db.create_knowledge_branch(&other, &base.id, "跨区", &[change.clone()])
                .is_err()
        );
        assert!(
            db.create_knowledge_branch(
                &workspace,
                &base.id,
                "重复",
                &[change.clone(), change.clone()]
            )
            .is_err()
        );
        assert!(
            db.create_knowledge_branch(
                &workspace,
                &base.id,
                "缺失版本",
                &[ContextSelection {
                    version: 45,
                    ..change.clone()
                }]
            )
            .is_err()
        );
        let branch = db
            .create_knowledge_branch(&workspace, &base.id, "西楼实验", &[change])
            .unwrap();
        assert_eq!(branch.base_snapshot_id.as_deref(), Some(base.id.as_str()));
        assert!(branch.overrides.as_ref().unwrap()[0]["approvedAtCapture"].is_null());
        let historic = Request::Snapshot {
            snapshot_id: base.id.clone(),
        };
        let experiment = Request::Branch {
            snapshot_id: branch.id.clone(),
        };
        let wrong_kind = Request::Snapshot {
            snapshot_id: branch.id.clone(),
        };
        assert!(resolve(&db.connection().unwrap(), &workspace, &wrong_kind).is_err());
        let settings = ModelSettings::default();
        let ask = |scope: Option<&Request>, manual: &[ContextSelection]| {
            db.prepare_context_run_with_retrieval(
                &new_id(),
                &workspace,
                &chat,
                "releasecode",
                &settings,
                manual,
                true,
                None,
                scope,
                false,
            )
            .unwrap()
        };
        for (scope, content, kind) in [
            (None, "东楼正式地点", "current_graph"),
            (Some(&historic), "东楼正式地点", "saved_snapshot"),
            (Some(&experiment), "西楼实验地点", "saved_snapshot"),
        ] {
            let run = ask(scope, &[]);
            let prompt = run.request.to_string();
            assert!(
                prompt.contains(content)
                    && prompt.contains("南楼反驳资料")
                    && !prompt.contains("二跳无关材料")
            );
            assert!(prompt.contains("资料1 与 资料2") || prompt.contains("资料2 与 资料1"));
            assert_eq!(
                run.manifest["retrieval"]["conflicts"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            let conflict = &run.manifest["retrieval"]["conflicts"][0];
            assert_eq!(conflict["recordedIn"], kind);
            assert_eq!(conflict["edgeId"], edge_id);
            assert_eq!(conflict["edgeVersion"], 1);
            assert!(
                conflict["from"]["citationId"].is_string()
                    && conflict["to"]["citationId"].is_string()
            );
            if matches!(scope, Some(Request::Branch { .. })) {
                assert_eq!(run.manifest["knowledgeScope"]["kind"], "branch");
                assert!(prompt.contains("待确认的草稿"));
                assert!(!prompt.contains("东楼正式地点"));
            }
        }
        db.set_conversation_retrieval(&workspace, &chat, false)
            .unwrap();
        let manual = ask(
            None,
            &[ContextSelection {
                node_id: primary.clone(),
                version: 1,
            }],
        );
        assert!(!manual.request.to_string().contains("南楼反驳资料"));
        assert!(
            manual
                .request
                .to_string()
                .contains("本次上下文只发送了一方")
        );
        assert!(manual.manifest["retrieval"]["conflicts"][0]["to"]["citationId"].is_null());
        db.set_conversation_retrieval(&workspace, &chat, true)
            .unwrap();
        db.save_knowledge(SaveKnowledge {
            node_id: Some(counter.clone()),
            expected_revision: Some(2),
            ..draft("例外通知", &"过长反驳材料。".repeat(500))
        })
        .unwrap();
        db.approve_knowledge(&workspace, &counter, 3).unwrap();
        let limited = ask(None, &[]);
        assert!(
            limited
                .request
                .to_string()
                .contains("本次上下文只发送了一方")
        );
        assert!(!limited.request.to_string().contains("过长反驳材料"));
        assert_eq!(
            limited.manifest["retrieval"]["candidates"][1]["reason"],
            "auto_budget"
        );
        db.save_knowledge_edge(crate::graph::SaveEdge {
            edge_id: Some(edge_id.clone()),
            expected_version: Some(1),
            active: false,
            change_reason: "停用".into(),
            ..edge
        })
        .unwrap();
        assert!(ask(None, &[]).manifest["retrieval"]["conflicts"].is_null());
        assert_eq!(
            ask(Some(&historic), &[]).manifest["retrieval"]["conflicts"][0]["edgeVersion"],
            1
        );
        assert_eq!(
            db.knowledge_detail(&workspace, &primary)
                .unwrap()
                .node
                .effective_version,
            Some(1)
        );
        db.approve_knowledge(&workspace, &primary, 3).unwrap();
        let current = ask(None, &[]);
        assert_eq!(
            current.manifest["knowledgeRefs"][0]["versionLineage"]["replacesVersion"],
            1
        );
        assert_eq!(
            ask(Some(&historic), &[]).manifest["knowledgeRefs"][0]["versionLineage"]["replacedByVersion"],
            2
        );
        assert!(!current.request.to_string().contains("东楼正式地点"));
        assert!(
            get_snapshot(&db.connection().unwrap(), &workspace, &branch.id)
                .unwrap()
                .overrides
                .unwrap()[0]["approvedAtCapture"]
                .is_null()
        );
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn knowledge_scope_preserves_versions_date_boundaries_and_history() {
        let path = std::env::temp_dir().join(format!("cortex-knowledge-scope-{}", new_id()));
        let db = Database::open(&path).unwrap();
        let workspace = db.create_workspace("历史资料").unwrap().id;
        let other = db.create_workspace("另一工作区").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let source_text = "# 交付记录\n交付地点为东楼。";
        let hash = crate::benchmark::dataset::hash(source_text.as_bytes());
        let source = db
            .import_source(PendingSource {
                workspace: workspace.clone(),
                path: "delivery.md".into(),
                raw_hash: hash.clone(),
                created: Instant::now(),
                preview: SourcePreview {
                    ticket: new_id(),
                    name: "delivery.md".into(),
                    content: source_text.into(),
                    content_hash: hash,
                    previous_revision: None,
                    previous_version: None,
                },
            })
            .unwrap();
        let draft = SaveKnowledge {
            workspace_id: workspace.clone(),
            node_id: None,
            expected_revision: None,
            title: "交付地点".into(),
            kind: "knowledge".into(),
            content: source_text.into(),
            change_reason: "验收".into(),
            source_message_id: None,
            source_excerpt: None,
        };
        let dates = |from: Option<&str>, to: Option<&str>| {
            Some(Validity {
                valid_from: from.map(str::to_owned),
                valid_to: to.map(str::to_owned),
            })
        };
        let node = db
            .save_version(
                draft.clone(),
                Some(SourceRef {
                    source_id: source.id.clone(),
                    version: 1,
                    start_line: 1,
                    end_line: 2,
                }),
                dates(Some("2001-01-01"), None),
            )
            .unwrap();
        db.approve_knowledge(&workspace, &node, 1).unwrap();
        let saved = db
            .capture_knowledge_snapshot(&workspace, "第一次交付记录")
            .unwrap();
        assert_eq!(saved.item_count, 1);
        let mut changed = SaveKnowledge {
            node_id: Some(node.clone()),
            expected_revision: Some(2),
            content: "交付地点改为西楼。".into(),
            ..draft.clone()
        };
        assert!(
            db.save_version(changed.clone(), None, dates(Some("2020-02-30"), None))
                .is_err()
        );
        assert!(
            db.save_version(
                changed.clone(),
                None,
                dates(Some("2020-01-01"), Some("2020-01-01"))
            )
            .is_err()
        );
        db.save_version(changed.clone(), None, dates(Some("2020-01-01"), None))
            .unwrap();
        db.approve_knowledge(&workspace, &node, 3).unwrap();
        // A date-only edit remains a draft and leaves the approved version intact.
        changed.expected_revision = Some(4);
        db.save_version(changed, None, dates(Some("2100-01-01"), None))
            .unwrap();
        let now = db.knowledge_detail(&workspace, &node).unwrap();
        assert_eq!(
            now.versions[0].validity.valid_from.as_deref(),
            Some("2100-01-01")
        );
        assert_eq!(now.node.effective_version, Some(2));
        let expired = db
            .save_version(
                SaveKnowledge {
                    title: "交付旧时段".into(),
                    content: "旧时段在上午。".into(),
                    ..draft.clone()
                },
                None,
                dates(None, Some("2020-01-01")),
            )
            .unwrap();
        db.approve_knowledge(&workspace, &expired, 1).unwrap();
        let temporary = db
            .save_version(
                SaveKnowledge {
                    title: "临时安排".into(),
                    content: "早期安排。".into(),
                    ..draft.clone()
                },
                None,
                dates(Some("2001-01-01"), None),
            )
            .unwrap();
        db.approve_knowledge(&workspace, &temporary, 1).unwrap();
        db.save_version(
            SaveKnowledge {
                node_id: Some(temporary.clone()),
                expected_revision: Some(2),
                title: "临时安排".into(),
                content: "更新的临时安排。".into(),
                ..draft.clone()
            },
            None,
            dates(Some("2030-01-01"), Some("2040-01-01")),
        )
        .unwrap();
        db.approve_knowledge(&workspace, &temporary, 3).unwrap();
        let undated = db
            .save_knowledge(SaveKnowledge {
                title: "交付备注".into(),
                content: "日期未记录。".into(),
                ..draft
            })
            .unwrap();
        db.approve_knowledge(&workspace, &undated, 1).unwrap();

        let settings = ModelSettings::default();
        let query = |scope: Option<&Request>, save: bool, manual: &[ContextSelection]| {
            db.prepare_context_run_with_retrieval(
                &new_id(),
                &workspace,
                &chat,
                "交付地点在哪里？",
                &settings,
                manual,
                !save,
                None,
                scope,
                false,
            )
        };
        let current = query(None, true, &[]).unwrap();
        assert!(current.request.to_string().contains("西楼"));
        assert!(!current.request.to_string().contains("上午"));
        db.finish(
            &current.run,
            "当前范围西楼答复",
            "completed",
            None,
            json!({}),
        )
        .unwrap();

        let scope = Request::Snapshot {
            snapshot_id: saved.id.clone(),
        };
        let historical = query(Some(&scope), true, &[]).unwrap();
        assert_eq!(
            historical.manifest["knowledgeScope"]["snapshotId"],
            saved.id
        );
        assert_eq!(historical.manifest["knowledgeRefs"][0]["version"], 1);
        assert_eq!(
            historical.manifest["knowledgeRefs"][0]["sourceEvidence"][0]["version"],
            1
        );
        assert!(historical.request.to_string().contains("东楼"));
        assert!(!historical.request.to_string().contains("当前范围西楼答复"));
        assert!(
            query(
                Some(&scope),
                false,
                &[ContextSelection {
                    node_id: node.clone(),
                    version: 2
                }]
            )
            .is_err()
        );
        db.finish(
            &historical.run,
            "历史范围东楼答复",
            "completed",
            None,
            json!({}),
        )
        .unwrap();
        assert!(
            query(Some(&scope), false, &[])
                .unwrap()
                .request
                .to_string()
                .contains("历史范围东楼答复")
        );
        assert!(
            !query(None, false, &[])
                .unwrap()
                .request
                .to_string()
                .contains("历史范围东楼答复")
        );

        for (date, version, includes_expired) in [("2019-12-31", 1, true), ("2020-01-01", 2, false)]
        {
            let request = Request::FactDate { date: date.into() };
            let c = db.connection().unwrap();
            let subset = resolve(&c, &workspace, &request).unwrap();
            assert!(subset.contains(&node, version));
            assert_eq!(subset.contains(&expired, 1), includes_expired);
            assert!(!subset.contains(&undated, 1));
            assert_eq!(subset.audit["unknownDateCount"], 1);
        }
        assert!(resolve(&db.connection().unwrap(), &other, &scope).is_err());
        for (date, expected) in [
            ("2029-12-31", Some(1)),
            ("2030-01-01", Some(2)),
            ("2040-01-01", None),
        ] {
            let c = db.connection().unwrap();
            let dated = resolve(&c, &workspace, &Request::FactDate { date: date.into() }).unwrap();
            let actual = dated
                .versions()
                .iter()
                .find(|(node, _)| node == &temporary)
                .map(|(_, v)| *v);
            assert_eq!(actual, expected);
        }
        let experiment = db
            .create_knowledge_branch(
                &workspace,
                &saved.id,
                "保留旧文件的实验",
                &[ContextSelection {
                    node_id: node,
                    version: 2,
                }],
            )
            .unwrap();
        db.revoke_source(&workspace, &source.id, source.revision)
            .unwrap();
        let revoked = query(Some(&scope), false, &[]).unwrap();
        assert_eq!(revoked.manifest["knowledgeScope"]["unavailableCount"], 1);
        assert_eq!(revoked.manifest["knowledgeRefs"], json!([]));
        assert!(!revoked.request.to_string().contains("东楼"));
        assert!(!query(None, false, &[]).unwrap().request.to_string().contains("当前范围西楼答复"));
        let blocked_branch = query(
            Some(&Request::Branch {
                snapshot_id: experiment.id,
            }),
            false,
            &[],
        )
        .unwrap();
        assert_eq!(blocked_branch.manifest["knowledgeRefs"], json!([]));
        assert_eq!(
            db.run_detail(&workspace, &historical.run.id)
                .unwrap()
                .snapshot
                .unwrap()
                .request,
            historical.request
        );
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
}
