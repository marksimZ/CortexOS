use crate::{
    commands::{AppState, blocking},
    db::error,
    knowledge_scope::{self, Resolved},
    models::AppResult,
    retrieval, source_chunks, sources,
};
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tauri::State;

const SEED_LIMIT: usize = 6;
const PATH_LIMIT: usize = 8;
const HOP_LIMIT: usize = 2;
const EDGE_LIMIT_PER_HOP: usize = 64;
const CLUSTER_EDGE_LIMIT: usize = 2000;
const CLUSTER_LIMIT: usize = 12;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Node {
    pub node_id: String,
    pub version: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FileRef>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileRef {
    source_id: String,
    version: i64,
    name: String,
    start_line: i64,
    end_line: i64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Relation {
    pub edge_id: String,
    pub edge_version: i64,
    pub recorded_in: &'static str,
    pub relation: String,
    pub from: Node,
    pub to: Node,
}

pub(crate) fn relations(
    c: &Connection,
    workspace: &str,
    scope: Option<&Resolved>,
    roots: Option<&[String]>,
    relation: Option<&str>,
    limit: usize,
) -> AppResult<(Vec<Relation>, bool)> {
    if roots.is_some_and(<[String]>::is_empty) {
        return Ok((Vec::new(), false));
    }
    let vertices = scope.map(|scope| json!(scope.versions()).to_string());
    let snapshot = scope.and_then(|scope| scope.audit["snapshotId"].as_str());
    let basis = if snapshot.is_some() {
        "saved_snapshot"
    } else {
        "current_graph"
    };
    let seeds = roots.map(|roots| json!(roots).to_string());
    let mut statement=c.prepare(
        "WITH eligible AS (
            SELECT v.node_id,v.version FROM rag_current_knowledge v
            WHERE ?3 IS NULL AND v.workspace_id=?1 AND NOT EXISTS(
                SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id
                WHERE l.node_id=v.node_id AND l.knowledge_version=v.version AND s.enabled=0)
            UNION ALL SELECT json_extract(value,'$[0]'),json_extract(value,'$[1]') FROM json_each(?3)
         ), revisions AS (
            SELECT id AS edge_id,latest_version AS version FROM knowledge_edges WHERE ?4 IS NULL AND workspace_id=?1
            UNION ALL SELECT edge_id,edge_version FROM knowledge_snapshot_relations WHERE workspace_id=?1 AND snapshot_id=?4
         ), seeds AS MATERIALIZED (SELECT CAST(key AS INTEGER) AS ordinal,value AS node FROM json_each(?2))
         SELECT e.edge_id,e.version,e.relation,
            a.node_id,a.version,va.title,sa.id,la.source_version,sa.name,la.start_line,la.end_line,
            b.node_id,b.version,vb.title,sb.id,lb.source_version,sb.name,lb.start_line,lb.end_line
         FROM revisions r
         JOIN knowledge_edge_versions e ON e.edge_id=r.edge_id AND e.version=r.version AND e.workspace_id=?1
         JOIN eligible a ON a.node_id=e.from_node_id JOIN eligible b ON b.node_id=e.to_node_id
         JOIN knowledge_versions va ON va.node_id=a.node_id AND va.version=a.version
         JOIN knowledge_versions vb ON vb.node_id=b.node_id AND vb.version=b.version
         LEFT JOIN knowledge_source_links la ON la.node_id=a.node_id AND la.knowledge_version=a.version
         LEFT JOIN source_documents sa ON sa.workspace_id=?1 AND sa.id=la.source_id
         LEFT JOIN knowledge_source_links lb ON lb.node_id=b.node_id AND lb.knowledge_version=b.version
         LEFT JOIN source_documents sb ON sb.workspace_id=?1 AND sb.id=lb.source_id
         LEFT JOIN seeds fa ON fa.node=e.from_node_id LEFT JOIN seeds fb ON fb.node=e.to_node_id
         WHERE e.active=1 AND (?5 IS NULL OR e.relation=?5)
            AND (?2 IS NULL OR fa.ordinal IS NOT NULL OR fb.ordinal IS NOT NULL)
         ORDER BY MIN(COALESCE(fa.ordinal,9999),COALESCE(fb.ordinal,9999)),e.edge_id LIMIT ?6"
    ).map_err(error)?;
    let mut rows = statement
        .query_map(
            params![
                workspace,
                seeds,
                vertices,
                snapshot,
                relation,
                (limit + 1) as i64
            ],
            |row| {
                let endpoint = |col: usize| -> rusqlite::Result<Node> {
                    let source = row
                        .get::<_, Option<String>>(col + 3)?
                        .map(|source_id| -> rusqlite::Result<_> {
                            Ok(FileRef {
                                source_id,
                                version: row.get(col + 4)?,
                                name: row.get(col + 5)?,
                                start_line: row.get(col + 6)?,
                                end_line: row.get(col + 7)?,
                            })
                        })
                        .transpose()?;
                    Ok(Node {
                        node_id: row.get(col)?,
                        version: row.get(col + 1)?,
                        title: row.get(col + 2)?,
                        source,
                    })
                };
                Ok(Relation {
                    edge_id: row.get(0)?,
                    edge_version: row.get(1)?,
                    relation: row.get(2)?,
                    recorded_in: basis,
                    from: endpoint(3)?,
                    to: endpoint(11)?,
                })
            },
        )
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    Ok((rows, truncated))
}

pub(crate) fn keywords(report: &Value) -> BTreeSet<String> {
    report["terms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub(crate) fn candidate(
    c: &Connection,
    workspace: &str,
    node: &Node,
    terms: &BTreeSet<String>,
    recall: &str,
) -> AppResult<Value> {
    let (kind,body):(String,String)=c.query_row(
        "SELECT v.kind,v.content FROM knowledge_versions v JOIN knowledge_nodes n ON n.id=v.node_id AND n.workspace_id=?1
         WHERE v.node_id=?2 AND v.version=?3",
        params![workspace,node.node_id,node.version],|row|Ok((row.get(0)?,row.get(1)?))
    ).map_err(error)?;
    let mut value = json!({"nodeId":node.node_id,"version":node.version,"title":node.title,"kind":kind,
        "content":body,"score":0,"included":false,"reason":"pending_budget","costUnits":0,"recallSource":recall});
    if let Some(source) = sources::evidence(c, &node.node_id, node.version)?.first()
        && let Some(chunk) = source_chunks::segments(workspace, &body, source)
            .into_iter()
            .max_by_key(|chunk| {
                retrieval::terms(chunk.body)
                    .into_iter()
                    .filter(|term| terms.contains(term))
                    .collect::<BTreeSet<_>>()
                    .len()
            })
    {
        value["chunkId"] = json!(chunk.id);
        value["chunkLocation"] = json!(chunk.location);
    }
    Ok(value)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    #[serde(flatten)]
    relation: Relation,
    reversed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Path {
    nodes: Vec<Node>,
    steps: Vec<Step>,
}

#[derive(Default)]
pub(crate) struct Trail {
    seeds: Vec<String>,
    paths: Vec<Path>,
    pub conflicts: Vec<Relation>,
    truncated: bool,
}

pub(crate) fn expand(
    c: &Connection,
    workspace: &str,
    scope: Option<&Resolved>,
    manual: &[Value],
    report: &mut Value,
) -> AppResult<Trail> {
    let terms = keywords(report);
    let direct = report["candidates"].as_array_mut().unwrap();
    let mut available = BTreeSet::new();
    let mut seeds = Vec::new();
    for node in manual
        .iter()
        .chain(direct.iter())
        .filter_map(|item| item["nodeId"].as_str())
    {
        if available.insert(node.to_owned()) {
            seeds.push(node.to_owned());
        }
    }
    let mut trail = Trail {
        truncated: seeds.len() > SEED_LIMIT,
        ..Trail::default()
    };
    seeds.truncate(SEED_LIMIT);
    trail.seeds = seeds.clone();
    let root_ids: BTreeSet<_> = seeds.iter().cloned().collect();
    let mut seen = root_ids.clone();
    let mut frontier: Vec<_> = seeds
        .into_iter()
        .map(|id| {
            (
                id,
                Path {
                    nodes: Vec::new(),
                    steps: Vec::new(),
                },
            )
        })
        .collect();
    let mut conflicts_seen = BTreeSet::new();
    let mut additions = Vec::new();
    let mut direct_links = BTreeSet::new();
    for hop in 0..HOP_LIMIT {
        let ids: Vec<_> = frontier.iter().map(|(id, _)| id.clone()).collect();
        if ids.is_empty() {
            break;
        }
        let (mut edges, capped) =
            relations(c, workspace, scope, Some(&ids), None, EDGE_LIMIT_PER_HOP)?;
        trail.truncated |= capped;
        // Prefer routes that can continue when a node also has a terminal conflict edge.
        edges.sort_by_key(|edge| edge.relation == "contradicts");
        let mut next = Vec::new();
        for edge in &edges {
            for (id, path) in &frontier {
                let (from, target, reversed) = if edge.from.node_id == *id {
                    (&edge.from, &edge.to, false)
                } else if edge.to.node_id == *id {
                    (&edge.to, &edge.from, true)
                } else {
                    continue;
                };
                if edge.relation == "contradicts" && conflicts_seen.insert(edge.edge_id.clone()) {
                    trail.conflicts.push(edge.clone());
                }
                let direct_link = hop == 0
                    && root_ids.contains(&target.node_id)
                    && direct_links.insert(edge.edge_id.clone());
                if !direct_link && seen.contains(&target.node_id) {
                    continue;
                }
                if trail.paths.len() == PATH_LIMIT {
                    trail.truncated = true;
                    continue;
                }
                seen.insert(target.node_id.clone());
                let mut extended = path.clone();
                if extended.nodes.is_empty() {
                    extended.nodes.push(from.clone());
                }
                extended.nodes.push(target.clone());
                extended.steps.push(Step {
                    relation: edge.clone(),
                    reversed,
                });
                if !direct_link && edge.relation != "contradicts" {
                    next.push((target.node_id.clone(), extended.clone()));
                }
                if available.insert(target.node_id.clone()) {
                    additions.push(candidate(c, workspace, target, &terms, "graph_relation")?);
                }
                trail.paths.push(extended);
            }
        }
        frontier = next;
    }
    if !additions.is_empty() {
        let at = direct.len().min(3);
        direct.splice(at..at, additions);
        let capped = direct.len() > 20;
        direct.truncate(20);
        report["candidatesTruncated"] = json!(capped || report["candidatesTruncated"] == true);
        report["status"] = json!("matched");
    }
    Ok(trail)
}

pub(crate) fn citation<'a>(items: &'a [Value], node: &Node) -> Option<&'a str> {
    items
        .iter()
        .find(|item| item["nodeId"] == node.node_id && item["version"] == node.version)
        .and_then(|item| item["citationId"].as_str())
}

fn relation_label(value: &str) -> &str {
    match value {
        "supports" => "支持",
        "depends_on" => "依赖",
        "derived_from" => "源自",
        "revises" => "修订",
        _ => "反驳",
    }
}

impl Trail {
    pub(crate) fn audit(&self, items: &[Value]) -> Value {
        let paths: Vec<_> = self
            .paths
            .iter()
            .map(|path| {
                let target = path.nodes.last().unwrap();
                let source_ids: BTreeSet<_> = path
                    .nodes
                    .iter()
                    .filter_map(|node| node.source.as_ref().map(|file| &file.source_id))
                    .collect();
                let mut record = json!(path);
                record["citationId"] = json!(citation(items, target));
                record["crossDocument"] = json!(source_ids.len() > 1);
                record
            })
            .collect();
        json!({"status":if self.seeds.is_empty(){"no_seeds"}else if paths.is_empty(){"no_paths"}else{"matched"},
               "maxHops":HOP_LIMIT,"seedCount":self.seeds.len(),"truncated":self.truncated,"paths":paths})
    }

    pub(crate) fn prompt_notice(&self, items: &[Value]) -> String {
        let pairs: BTreeSet<_> = self
            .paths
            .iter()
            .flat_map(|path| &path.steps)
            .filter(|step| step.relation.relation != "contradicts")
            .filter_map(|step| {
                let edge = &step.relation;
                let from = citation(items, &edge.from)?;
                let to = citation(items, &edge.to)?;
                Some(format!("{from}{} {to}", relation_label(&edge.relation)))
            })
            .collect();
        if pairs.is_empty() {
            return String::new();
        }
        format!(
            " 已登记的资料关系为 {}。请根据实际资料内容判断关系是否适用于本题。",
            pairs.into_iter().collect::<Vec<_>>().join("，")
        )
    }
}

pub(crate) fn cluster_overview(
    c: &Connection,
    workspace: &str,
    scope: Option<&Resolved>,
) -> AppResult<Value> {
    let (edges, edges_capped) = relations(c, workspace, scope, None, None, CLUSTER_EDGE_LIMIT)?;
    let mut nodes = BTreeMap::<&str, (&Node, Vec<usize>)>::new();
    for (i, edge) in edges.iter().enumerate() {
        for point in [&edge.from, &edge.to] {
            nodes
                .entry(&point.node_id)
                .or_insert((point, Vec::new()))
                .1
                .push(i);
        }
    }
    let mut seen = BTreeSet::new();
    let mut groups = Vec::new();
    for root in nodes.keys() {
        if !seen.insert(*root) {
            continue;
        }
        let mut queue = VecDeque::from([*root]);
        let mut members = Vec::new();
        let mut edges_in_group = BTreeSet::new();
        let mut files = BTreeSet::new();
        while let Some(id) = queue.pop_front() {
            let (node, adjacent) = &nodes[id];
            members.push(*node);
            if let Some(file) = &node.source {
                files.insert(&file.source_id);
            }
            for i in adjacent {
                if !edges_in_group.insert(*i) {
                    continue;
                }
                for next in [&edges[*i].from.node_id, &edges[*i].to.node_id] {
                    if seen.insert(next.as_str()) {
                        queue.push_back(next.as_str());
                    }
                }
            }
        }
        members.sort_by(|a, b| {
            nodes[b.node_id.as_str()]
                .1
                .len()
                .cmp(&nodes[a.node_id.as_str()].1.len())
                .then(a.node_id.cmp(&b.node_id))
        });
        groups.push((members, edges_in_group, files.len()));
    }
    groups.sort_by(|a, b| {
        b.0.len()
            .cmp(&a.0.len())
            .then(a.0[0].node_id.cmp(&b.0[0].node_id))
    });
    let truncated = groups.len() > CLUSTER_LIMIT || edges_capped;
    let mut summaries = Vec::new();
    for (members, links, file_count) in groups.into_iter().take(CLUSTER_LIMIT) {
        let mut counts = BTreeMap::new();
        for index in &links {
            *counts.entry(edges[*index].relation.as_str()).or_insert(0) += 1;
        }
        let mut examples = Vec::new();
        for node in members.iter().take(3) {
            let body: String = c
                .query_row(
                    "SELECT content FROM knowledge_versions WHERE node_id=?1 AND version=?2",
                    params![node.node_id, node.version],
                    |row| row.get(0),
                )
                .map_err(error)?;
            let excerpt: String = body.trim().chars().take(120).collect();
            examples.push(json!({"node":node,"excerpt":excerpt,"excerptTruncated":excerpt.len()<body.trim().len()}));
        }
        summaries.push(json!({"id":members[0].node_id,"name":members[0].title,"nodeCount":members.len(),
            "edgeCount":links.len(),"fileCount":file_count,"relationCounts":counts,"examples":examples}));
    }
    Ok(
        json!({"groups":summaries,"truncated":truncated,"edgeLimit":CLUSTER_EDGE_LIMIT,"groupLimit":CLUSTER_LIMIT}),
    )
}

#[tauri::command]
pub async fn get_knowledge_clusters(
    state: State<'_, AppState>,
    workspace_id: String,
    knowledge_scope: Option<knowledge_scope::Request>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        crate::benchmark::dataset::require_workspace(&c, &workspace_id)?;
        let scope = knowledge_scope
            .as_ref()
            .map(|request| knowledge_scope::resolve(&c, &workspace_id, request))
            .transpose()?;
        cluster_overview(&c, &workspace_id, scope.as_ref())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        graph::SaveEdge,
        knowledge::SaveKnowledge,
        models::{ModelSettings, new_id},
        sources::{PendingSource, SourcePreview, SourceRef},
    };
    use std::time::Instant;

    #[test]
    fn knowledge_graph_retrieval_preserves_direction_sources_and_scope() {
        let dir = std::env::temp_dir().join(format!("cortex-graph-rag-{}", new_id()));
        let db = Database::open(&dir).unwrap();
        let workspace = db.create_workspace("路径验收").unwrap().id;
        let other = db.create_workspace("另一工作区").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let node = |workspace: &str, title: &str, body: &str, file: Option<&str>| {
            let source = file.map(|name| {
                let hash = crate::benchmark::dataset::hash(body.as_bytes());
                db.import_source(PendingSource {
                    workspace: workspace.into(),
                    path: name.into(),
                    raw_hash: hash.clone(),
                    created: Instant::now(),
                    preview: SourcePreview {
                        ticket: new_id(),
                        name: name.into(),
                        content: body.into(),
                        content_hash: hash,
                        previous_revision: None,
                        previous_version: None,
                    },
                })
                .unwrap()
            });
            let id = db
                .save_knowledge_with_evidence(
                    SaveKnowledge {
                        workspace_id: workspace.into(),
                        node_id: None,
                        expected_revision: None,
                        title: title.into(),
                        kind: "knowledge".into(),
                        content: body.into(),
                        change_reason: "验收".into(),
                        source_message_id: None,
                        source_excerpt: None,
                    },
                    source.as_ref().map(|file| SourceRef {
                        source_id: file.id.clone(),
                        version: 1,
                        start_line: 1,
                        end_line: 1,
                    }),
                )
                .unwrap();
            db.approve_knowledge(workspace, &id, 1).unwrap();
            (id, source)
        };
        let (root, _) = node(
            &workspace,
            "发布入口",
            "release_key 发布规则甲",
            Some("release.md"),
        );
        let (one, one_source) = node(&workspace, "准备工作", "准备工作乙", Some("steps.md"));
        let (two, _) = node(&workspace, "检查项", "检查内容丙", None);
        let (three, _) = node(&workspace, "后续项", "禁止穿透三跳", None);
        let (conflict, _) = node(&workspace, "例外规则", "例外登载丁", None);
        let (terminal, _) = node(&workspace, "例外的旁支", "禁止穿透反驳", None);
        let (stranger, _) = node(&other, "别处", "release_key FOREIGN_CONTENT", None);
        let relation = |from: &str, to: &str, relation: &str| SaveEdge {
            workspace_id: workspace.clone(),
            edge_id: None,
            expected_version: None,
            from_node_id: from.into(),
            to_node_id: to.into(),
            relation: relation.into(),
            active: true,
            change_reason: "测试证据".into(),
        };
        let incoming = relation(&one, &root, "depends_on");
        let edge_id = db.save_knowledge_edge(incoming.clone()).unwrap();
        db.save_knowledge_edge(relation(&one, &two, "supports"))
            .unwrap();
        db.save_knowledge_edge(relation(&two, &three, "revises"))
            .unwrap();
        db.save_knowledge_edge(relation(&root, &conflict, "contradicts"))
            .unwrap();
        db.save_knowledge_edge(relation(&conflict, &terminal, "derived_from"))
            .unwrap();
        assert!(
            db.save_knowledge_edge(relation(&root, &stranger, "supports"))
                .is_err()
        );
        let snapshot = db
            .capture_knowledge_snapshot(&workspace, "冻结路径")
            .unwrap();
        let snapshot_json = json!(snapshot);
        let frozen = knowledge_scope::Request::Snapshot {
            snapshot_id: snapshot_json["id"].as_str().unwrap().into(),
        };
        let settings = ModelSettings {
            num_ctx: 8192,
            ..ModelSettings::default()
        };
        let ask_text = |text: &str, scope: Option<&knowledge_scope::Request>, graph, preview| {
            db.prepare_context_run_with_retrieval(
                &new_id(),
                &workspace,
                &chat,
                text,
                &settings,
                &[],
                preview,
                None,
                scope,
                graph,
            )
            .unwrap()
        };
        let ask = |scope, graph, preview| ask_text("release_key", scope, graph, preview);
        let enabled = ask(None, true, false);
        let graph = &enabled.manifest["retrieval"]["graph"];
        let paths = graph["paths"].as_array().unwrap();
        let path_to = |target: &str| {
            paths
                .iter()
                .find(|path| path["nodes"].as_array().unwrap().last().unwrap()["nodeId"] == target)
                .unwrap()
        };
        assert_eq!(graph["status"], "matched");
        assert_eq!(paths.len(), 3);
        assert_eq!(path_to(&one)["steps"][0]["reversed"], true);
        assert_eq!(path_to(&two)["steps"].as_array().unwrap().len(), 2);
        assert_eq!(path_to(&two)["crossDocument"], true);
        assert_eq!(path_to(&two)["nodes"][0]["source"]["version"], 1);
        assert_eq!(path_to(&two)["nodes"][1]["source"]["name"], "steps.md");
        assert_eq!(path_to(&conflict)["steps"][0]["relation"], "contradicts");
        let request = enabled.request.to_string();
        assert!(request.contains("检查内容丙") && request.contains("已登记的资料关系"));
        assert!(!request.contains("禁止穿透") && !request.contains("FOREIGN_CONTENT"));
        assert_eq!(
            db.run_detail(&workspace, &enabled.run.id)
                .unwrap()
                .context_manifest["retrieval"]["graph"],
            *graph
        );
        assert!(ask(None, false, true).manifest["retrieval"]["graph"].is_null());
        let routed = ask_text("请梳理 release_key 的依赖链。", None, false, true);
        assert_eq!(routed.manifest["retrieval"]["route"]["graphReason"], "relationship_question");
        assert_eq!(routed.manifest["retrieval"]["graph"]["status"], "matched");
        db.set_conversation_retrieval(&workspace, &chat, false)
            .unwrap();
        assert!(ask(None, true, true).manifest["retrieval"]["graph"].is_null());
        assert!(ask_text("请梳理 release_key 的依赖链。", None, false, true).manifest["retrieval"]["graph"].is_null());
        db.set_conversation_retrieval(&workspace, &chat, true)
            .unwrap();
        {
            let c = db.connection().unwrap();
            let overview = cluster_overview(&c, &workspace, None).unwrap();
            assert_eq!(overview["groups"][0]["nodeCount"], 6);
            assert_eq!(overview["groups"][0]["fileCount"], 2);
            assert_eq!(overview["groups"][0]["relationCounts"]["contradicts"], 1);
            assert_eq!(
                cluster_overview(&c, &other, None).unwrap()["groups"],
                json!([])
            );
        }
        db.save_knowledge_edge(SaveEdge {
            edge_id: Some(edge_id),
            expected_version: Some(1),
            active: false,
            ..incoming
        })
        .unwrap();
        assert!(
            !ask(None, true, true)
                .request
                .to_string()
                .contains("检查内容丙")
        );
        let old = ask(Some(&frozen), true, true);
        assert!(old.request.to_string().contains("检查内容丙"));
        assert_eq!(
            old.manifest["retrieval"]["graph"]["paths"][0]["steps"][0]["recordedIn"],
            "saved_snapshot"
        );
        let file = one_source.unwrap();
        db.revoke_source(&workspace, &file.id, file.revision)
            .unwrap();
        assert!(
            !ask(Some(&frozen), true, true)
                .request
                .to_string()
                .contains("检查内容丙")
        );
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
