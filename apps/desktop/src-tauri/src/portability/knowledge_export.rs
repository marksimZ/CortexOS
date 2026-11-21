use super::*;
use crate::benchmark::report_transfer::write_new;
use rusqlite::types::ValueRef;
use serde_json::{Value, json};
use std::fmt::Write as _;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Format {
    Json,
    Markdown,
}

fn rows(c: &Connection, sql: &str, workspace: &str) -> AppResult<Vec<Value>> {
    let mut statement = c.prepare(sql).map_err(error)?;
    let columns = statement
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    statement
        .query_map([workspace], |row| {
            let mut item = serde_json::Map::new();
            for (index, name) in columns.iter().enumerate() {
                let value = match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => json!(n),
                    ValueRef::Text(_) => json!(row.get::<_, String>(index)?),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                item.insert(name.clone(), value);
            }
            Ok(Value::Object(item))
        })
        .map_err(error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(error)
}

pub(super) fn report(c: &Connection, workspace: &str) -> AppResult<Value> {
    let info = workspaces(c, Some(workspace))?;
    Ok(json!({
        "formatVersion": 1,
        "exportedAt": now(),
        "workspace": info[0],
        "nodes": rows(c,"SELECT * FROM knowledge_nodes WHERE workspace_id=?1 ORDER BY id",workspace)?,
        "versions": rows(c,"SELECT v.* FROM knowledge_versions v JOIN knowledge_nodes n ON n.id=v.node_id WHERE n.workspace_id=?1 ORDER BY v.node_id,v.version",workspace)?,
        "approvals": rows(c,"SELECT a.* FROM knowledge_approval_events a JOIN knowledge_nodes n ON n.id=a.node_id WHERE n.workspace_id=?1 ORDER BY a.node_id,a.version",workspace)?,
        "validity": rows(c,"SELECT v.* FROM knowledge_validity v JOIN knowledge_nodes n ON n.id=v.node_id WHERE n.workspace_id=?1 ORDER BY v.node_id,v.version",workspace)?,
        "sources": rows(c,"SELECT id,name,latest_version,enabled,revision,created_at,updated_at FROM source_documents WHERE workspace_id=?1 ORDER BY id",workspace)?,
        "sourceVersions": rows(c,"SELECT v.* FROM source_versions v JOIN source_documents s ON s.id=v.source_id WHERE s.workspace_id=?1 ORDER BY v.source_id,v.version",workspace)?,
        "sourceLinks": rows(c,"SELECT l.* FROM knowledge_source_links l JOIN knowledge_nodes n ON n.id=l.node_id WHERE n.workspace_id=?1 ORDER BY l.node_id,l.knowledge_version",workspace)?,
        "relations": rows(c,"SELECT * FROM knowledge_edges WHERE workspace_id=?1 ORDER BY id",workspace)?,
        "relationVersions": rows(c,"SELECT * FROM knowledge_edge_versions WHERE workspace_id=?1 ORDER BY edge_id,version",workspace)?,
    }))
}

fn field<'a>(item: &'a Value, key: &str) -> &'a str {
    item[key].as_str().unwrap_or_default()
}

fn array<'a>(report: &'a Value, key: &str) -> &'a [Value] {
    report[key].as_array().map_or(&[], Vec::as_slice)
}

pub(super) fn markdown(report: &Value) -> Vec<u8> {
    let mut text = format!(
        "# {}\n\n导出时间 {}\n\n",
        field(&report["workspace"], "name"),
        field(report, "exportedAt")
    );
    let nodes: BTreeMap<_, _> = array(report, "nodes")
        .iter()
        .map(|v| (field(v, "id"), v))
        .collect();
    let links: BTreeMap<_, _> = array(report, "sourceLinks")
        .iter()
        .map(|v| ((field(v, "node_id"), v["knowledge_version"].as_i64()), v))
        .collect();
    let sources: BTreeMap<_, _> = array(report, "sources")
        .iter()
        .map(|v| (field(v, "id"), field(v, "name")))
        .collect();
    for version in array(report, "versions") {
        let node = field(version, "node_id");
        let number = &version["version"];
        let active = nodes
            .get(node)
            .is_some_and(|n| n["effective_version"] == *number);
        let state = if active {
            "生效版本"
        } else {
            "其他版本"
        };
        let _ = writeln!(
            text,
            "## {} · 版本 {number}\n\n- {state}\n- 条目编号 `{node}`\n- 修改时间 {}\n- 修改原因 {}\n",
            field(version, "title"),
            field(version, "created_at"),
            field(version, "change_reason")
        );
        if let Some(link) = links.get(&(node, number.as_i64())) {
            let source = field(link, "source_id");
            let _ = writeln!(
                text,
                "- 来源 {} · 版本 {} · 第 {}–{} 行\n- 来源编号 `{source}`\n",
                sources.get(source).copied().unwrap_or(source),
                link["source_version"],
                link["start_line"],
                link["end_line"]
            );
        }
        let _ = writeln!(text, "{}\n", field(version, "content"));
    }
    if !sources.is_empty() {
        text.push_str("## 来源文本\n\n");
        for version in array(report, "sourceVersions") {
            let id = field(version, "source_id");
            let _ = writeln!(
                text,
                "### {} · 版本 {}\n\n来源编号 `{id}`\n\n{}\n",
                sources.get(id).copied().unwrap_or(id),
                version["version"],
                field(version, "content")
            );
        }
    }
    if !array(report, "relations").is_empty() {
        text.push_str("## 关系与版本\n\n");
        for version in array(report, "relationVersions") {
            let _ = writeln!(
                text,
                "- `{}` · {} · `{}` · 版本 {} · {}",
                field(version, "from_node_id"),
                field(version, "relation"),
                field(version, "to_node_id"),
                version["version"],
                field(version, "change_reason")
            );
        }
    }
    text.into_bytes()
}

#[tauri::command]
pub(crate) async fn export_knowledge(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
    format: Format,
) -> AppResult<bool> {
    let _lease = state.claim_task(&new_id(), &workspace_id, TaskKind::LocalMaintenance)?;
    blocking(state.database.clone()?, move |db| {
        let ext = if matches!(format, Format::Json) {
            "json"
        } else {
            "md"
        };
        let path = app
            .dialog()
            .file()
            .set_title("导出工作区知识")
            .add_filter("知识导出", &[ext])
            .set_file_name(format!("CortexOS-知识-{}.{ext}", new_id()))
            .blocking_save_file();
        let Some(path) = path else { return Ok(false) };
        let data = report(&*db.connection()?, &workspace_id)?;
        let bytes = match format {
            Format::Json => serde_json::to_vec_pretty(&data).map_err(json_error)?,
            Format::Markdown => markdown(&data),
        };
        write_new(
            &path.into_path().map_err(|_| "请选择本机文件位置。")?,
            &bytes,
        )?;
        Ok(true)
    })
    .await
}
