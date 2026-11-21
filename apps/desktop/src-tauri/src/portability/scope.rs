use crate::{db::error, models::AppResult, retrieval};
use rusqlite::{Connection, config::DbConfig};

const DATASETS: &str = "(json_extract(provenance_json,'$.workspaceId') IS ?1 OR (id='cortex-core-60' AND json_type(provenance_json,'$.workspaceId') IS NULL))";

fn tables(c: &Connection) -> AppResult<Vec<(String, bool)>> {
    c.prepare("SELECT name, EXISTS(SELECT 1 FROM pragma_table_info(t.name) WHERE name='workspace_id') FROM pragma_table_list t WHERE schema='main' AND type='table' AND name NOT GLOB 'sqlite_*' ORDER BY name")
        .map_err(error)?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(error)
}

fn predicate(table: &str, direct: bool, schema: &str) -> AppResult<String> {
    let rule = if direct {
        "workspace_id IS ?1"
    } else {
        match table {
            "schema_migrations" => "?1 IS ?1",
            "app_settings" => "?1 IS NOT ?1",
            "workspaces" => "id IS ?1",
            "benchmark_datasets" => DATASETS,
            "benchmark_cases" => {
                "EXISTS(SELECT 1 FROM @.benchmark_datasets d WHERE d.id=dataset_id AND d.version=dataset_version AND (json_extract(d.provenance_json,'$.workspaceId') IS ?1 OR (d.id='cortex-core-60' AND json_type(d.provenance_json,'$.workspaceId') IS NULL)))"
            }
            "benchmark_batch_items" => {
                "batch_id IN (SELECT id FROM @.benchmark_batches WHERE workspace_id=?1)"
            }
            "benchmark_generated_cases" => {
                "job_id IN (SELECT id FROM @.benchmark_generation_jobs WHERE workspace_id=?1)"
            }
            "context_knowledge_items"
            | "context_snapshots"
            | "execution_events"
            | "model_backup_requests"
            | "retrieval_results" => {
                "run_id IN (SELECT id FROM @.execution_runs WHERE workspace_id=?1)"
            }
            "knowledge_approval_events"
            | "knowledge_validity"
            | "knowledge_versions"
            | "knowledge_source_links" => {
                "node_id IN (SELECT id FROM @.knowledge_nodes WHERE workspace_id=?1)"
            }
            "source_events" | "source_versions" => {
                "source_id IN (SELECT id FROM @.source_documents WHERE workspace_id=?1)"
            }
            _ => return Err("该数据库包含尚未支持的工作区内容，请使用整应用备份。".into()),
        }
    };
    Ok(rule.replace('@', schema))
}

pub(super) fn verify(c: &Connection, id: &str) -> AppResult<()> {
    for (table, direct) in tables(c)? {
        let rule = predicate(&table, direct, "main")?;
        if c.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM \"{table}\" WHERE ({rule}) IS NOT TRUE)"),
            [id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(error)?
        {
            return Err("工作区备份包含范围之外的数据。".into());
        }
    }
    let has_fts: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='knowledge_fts')",
            [],
            |r| r.get(0),
        )
        .map_err(error)?;
    if has_fts
        && c.query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_fts WHERE workspace_id IS NOT ?1)",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(error)?
    {
        return Err("工作区备份的全文索引包含范围之外的数据。".into());
    }
    Ok(())
}

pub(super) fn copy_workspace(
    c: &mut Connection,
    source: &std::path::Path,
    workspace: &str,
    fresh: bool,
) -> AppResult<()> {
    if c.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
        [workspace],
        |r| r.get::<_, bool>(0),
    )
    .map_err(error)?
    {
        return Err("此工作区已存在。请恢复到另一个实例，或选择完整备份替换整应用。".into());
    }
    let tables = tables(c)?;
    c.execute("ATTACH DATABASE ?1 AS incoming", [source.to_string_lossy()])
        .map_err(error)?;
    c.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
        .map_err(error)?;
    c.execute_batch("PRAGMA foreign_keys=OFF").map_err(error)?;
    let tx = c.transaction().map_err(error)?;
    // Only the built-in dataset is shared. Check the immutable dataset and all its case
    // rows before reusing a matching version; installation timestamps may differ.
    let versions = "SELECT i.version FROM incoming.benchmark_datasets i JOIN main.benchmark_datasets m USING (id,version) WHERE i.id='cortex-core-60'";
    let conflict = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM incoming.benchmark_datasets i JOIN main.benchmark_datasets m USING(id,version) WHERE i.id='cortex-core-60' AND (i.name<>m.name OR i.schema_version<>m.schema_version OR i.content_hash<>m.content_hash OR i.manifest_json<>m.manifest_json OR i.provenance_json<>m.provenance_json))",
        [], |r| r.get::<_, bool>(0),
    ).map_err(error)?;
    if conflict {
        return Err("内置题库版本发生冲突，工作区未合并。".into());
    }
    for (left, right) in [("main", "incoming"), ("incoming", "main")] {
        let select = |schema| {
            format!(
                "SELECT * FROM {schema}.benchmark_cases WHERE dataset_id='cortex-core-60' AND dataset_version IN ({versions})"
            )
        };
        if tx
            .query_row(
                &format!("SELECT EXISTS({} EXCEPT {})", select(left), select(right)),
                [],
                |r| r.get::<_, bool>(0),
            )
            .map_err(error)?
        {
            return Err("内置题库题目发生冲突，工作区未合并。".into());
        }
    }
    for (table, direct) in tables {
        let rule = predicate(&table, direct, "incoming")?;
        let additional = match table.as_str() {
            "app_settings" => continue,
            "schema_migrations" if !fresh => continue,
            "schema_migrations" => {
                tx.execute("DELETE FROM main.schema_migrations", [])
                    .map_err(error)?;
                String::new()
            }
            "benchmark_cases" => format!(
                " AND NOT (dataset_id='cortex-core-60' AND dataset_version IN ({versions}))"
            ),
            "benchmark_datasets" => {
                format!(" AND NOT (id='cortex-core-60' AND version IN ({versions}))")
            }
            _ => String::new(),
        };
        // Harness timelines and some dataset views use insertion order for persisted records.
        tx.execute(
            &format!("INSERT INTO main.\"{table}\" SELECT * FROM incoming.\"{table}\" WHERE ({rule}){additional} ORDER BY rowid"),
            [workspace],
        )
        .map_err(|_| "数据编号冲突或写入失败，工作区未合并。".to_string())?;
    }
    retrieval::rebuild(&tx)?;
    super::check_integrity(&tx)?;
    tx.commit().map_err(error)?;
    c.execute_batch("DETACH DATABASE incoming; PRAGMA foreign_keys=ON")
        .map_err(error)?;
    c.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, true)
        .map_err(error)?;
    Ok(())
}
