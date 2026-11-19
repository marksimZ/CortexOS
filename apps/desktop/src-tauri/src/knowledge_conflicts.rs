use crate::{
    knowledge_graph_retrieval::{self as graph, Relation as Conflict, citation},
    knowledge_scope::Resolved,
    models::AppResult,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MAX_CONFLICTS: usize = 10;

pub(crate) fn find(
    c: &Connection,
    workspace: &str,
    scope: Option<&Resolved>,
    seeds: &BTreeSet<String>,
) -> AppResult<(Vec<Conflict>, bool)> {
    let seeds: Vec<_> = seeds.iter().cloned().collect();
    graph::relations(
        c,
        workspace,
        scope,
        Some(&seeds),
        Some("contradicts"),
        MAX_CONFLICTS,
    )
}

pub(crate) fn merge(conflicts: &mut Vec<Conflict>, additional: &[Conflict]) -> bool {
    let mut seen: BTreeSet<_> = conflicts.iter().map(|edge| edge.edge_id.clone()).collect();
    conflicts.extend(
        additional
            .iter()
            .filter(|edge| seen.insert(edge.edge_id.clone()))
            .cloned(),
    );
    let capped = conflicts.len() > MAX_CONFLICTS;
    conflicts.truncate(MAX_CONFLICTS);
    capped
}

pub(crate) fn include_counterparts(
    c: &Connection,
    workspace: &str,
    conflicts: &[Conflict],
    seeds: &BTreeSet<String>,
    report: &mut Value,
) -> AppResult<()> {
    let keywords = graph::keywords(report);
    let candidates = report["candidates"].as_array_mut().unwrap();
    let original_count = candidates.len();
    let mut added = BTreeSet::new();
    for relation in conflicts {
        let (anchor, other) = if seeds.contains(&relation.from.node_id) {
            (&relation.from, &relation.to)
        } else {
            (&relation.to, &relation.from)
        };
        if seeds.contains(&other.node_id) || !added.insert(&other.node_id) {
            continue;
        }
        let value = graph::candidate(c, workspace, other, &keywords, "explicit_conflict")?;
        let index = candidates
            .iter()
            .position(|item| item["nodeId"] == anchor.node_id)
            .map_or(0, |i| i + 1);
        candidates.insert(index, value);
    }
    if candidates.len() > original_count {
        let took = candidates.len() > 20;
        candidates.truncate(20);
        report["candidatesTruncated"] = json!(took || report["candidatesTruncated"] == true);
        report["status"] = json!("matched");
    }
    Ok(())
}

pub(crate) fn prompt_notice(conflicts: &[Conflict], items: &[Value]) -> String {
    let mut pairs = BTreeSet::new();
    for edge in conflicts {
        let from = citation(items, &edge.from);
        let to = citation(items, &edge.to);
        if let (Some(left), Some(right)) = (from, to) {
            pairs.insert(format!("{left} 与 {right}"));
        } else if let Some(only) = from.or(to) {
            pairs.insert(format!("{only} 有已记录的反驳资料，本次上下文只发送了一方"));
        }
    }
    if pairs.is_empty() {
        return String::new();
    }
    format!(
        " 已记录的反驳关系：{}。请区分双方实际提供的依据，引用能查看的资料；另一方未提供时说明依据不完整。",
        pairs.into_iter().collect::<Vec<_>>().join("；")
    )
}

pub(crate) fn audit(conflicts: &[Conflict], items: &[Value]) -> Vec<Value> {
    conflicts
        .iter()
        .map(|edge| {
            let mut value = json!(edge);
            value["from"]["citationId"] = json!(citation(items, &edge.from));
            value["to"]["citationId"] = json!(citation(items, &edge.to));
            value
        })
        .collect()
}
