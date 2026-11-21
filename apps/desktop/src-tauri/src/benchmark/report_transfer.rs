//! Versioned local reports. Snapshot validation and file publication never mutate benchmark data.
use super::{dataset, report_read::ReportScope};
use crate::{
    cloud::CloudState,
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
    tasks::TaskKind,
};
use rusqlite::{Connection, Params, types::ValueRef};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub(super) fn access<T>(
    cloud: &CloudState,
    scope: ReportScope,
    read: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    if matches!(scope, ReportScope::Standard) {
        read()
    } else {
        cloud.with_access(read)
    }
}
pub(super) fn rows(c: &Connection, sql: &str, args: impl Params) -> AppResult<Vec<Value>> {
    let mut q = c.prepare(sql).map_err(error)?;
    let names = q
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mapped = q
        .query_map(args, |row| {
            let mut object = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(v) => json!(v),
                    ValueRef::Real(v) => json!(v),
                    ValueRef::Text(v) => {
                        let text =
                            std::str::from_utf8(v).map_err(|_| rusqlite::Error::InvalidQuery)?;
                        if name.ends_with("_json") {
                            serde_json::from_str(text).map_err(|_| rusqlite::Error::InvalidQuery)?
                        } else {
                            json!(text)
                        }
                    }
                    ValueRef::Blob(_) => return Err(rusqlite::Error::InvalidQuery),
                };
                object.insert(name.clone(), value);
            }
            Ok(Value::Object(object))
        })
        .map_err(error)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(error)
}
fn revision(c: &Connection) -> AppResult<(u64, i64)> {
    Ok((
        c.total_changes(),
        c.query_row("PRAGMA data_version", [], |r| r.get(0))
            .map_err(error)?,
    ))
}
// Only connection-owned metadata is sanitized; questions, answers and tool schemas retain their original data.
pub(super) fn strip_transport(value: &mut Value) {
    if let Some(map) = value.as_object_mut() {
        map.retain(|key, _| {
            !matches!(
                key.to_ascii_lowercase().replace(['_', '-'], "").as_str(),
                "apikey"
                    | "authorization"
                    | "proxyauthorization"
                    | "xapikey"
                    | "accesstoken"
                    | "refreshtoken"
                    | "clientsecret"
                    | "password"
            )
        });
        for key in [
            "headers",
            "provider",
            "connection",
            "credentials",
            "transport",
            "wire",
        ] {
            if let Some(child) = map.get_mut(key) {
                strip_transport(child);
            }
        }
    }
}
fn strip_preflight_events(events: &mut Value) {
    if let Some(events) = events.as_array_mut() {
        for event in events {
            if matches!(
                event["kind"].as_str(),
                Some("model_preflight_completed" | "model_inspection_completed")
            ) {
                for key in ["payload", "payload_json"] {
                    if let Some(value) = event.get_mut(key) {
                        strip_transport(value);
                    }
                }
            }
        }
    }
}
impl Database {
    pub fn benchmark_export(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
    ) -> AppResult<Value> {
        let before = revision(&*self.connection()?)?;
        let batch =
            self.require_batch_scope(workspace, id, matches!(scope, ReportScope::Standard))?;
        let mut report = if matches!(scope, ReportScope::Standard) {
            self.standard_report(workspace, id)?
        } else {
            self.personalized_report(workspace, id)?
        };
        if let Some(reports) = report["reports"].as_array_mut() {
            for report in reports {
                if let Some(results) = report["results"].as_array_mut() {
                    for result in results {
                        strip_preflight_events(&mut result["events"]);
                    }
                }
            }
        }
        let c = self.connection()?;
        let mut items = Vec::new();
        for item in batch["items"].as_array().ok_or("批次项目缺失。")? {
            let case=rows(&c,"SELECT * FROM benchmark_cases WHERE dataset_id=?1 AND dataset_version=?2 AND case_id=?3",rusqlite::params![batch["plan"]["input"]["datasetId"].as_str(),batch["plan"]["input"]["datasetVersion"].as_i64(),item["caseId"].as_str()])?.pop().ok_or("冻结题目缺失。")?;
            let mut entry =
                json!({"item":item,"case":case,"run":null,"models":[],"attempts":[],"events":[]});
            if let Some(run) = item["runId"].as_str() {
                let mut run_record = rows(
                    &c,
                    "SELECT * FROM benchmark_runs WHERE workspace_id=?1 AND id=?2",
                    [workspace, run],
                )?
                .pop()
                .ok_or("运行不属于当前工作区。")?;
                if run_record["dataset_id"] != batch["plan"]["input"]["datasetId"]
                    || run_record["dataset_version"] != batch["plan"]["input"]["datasetVersion"]
                    || run_record["config_snapshot_json"]["input"]["caseId"] != item["caseId"]
                {
                    return Err("运行与冻结题目或题库版本不一致，未导出。".into());
                }
                strip_transport(&mut run_record["config_snapshot_json"]);
                let mut models = rows(
                    &c,
                    "SELECT * FROM benchmark_run_models WHERE workspace_id=?1 AND run_id=?2 ORDER BY model_slot",
                    [workspace, run],
                )?;
                for model in &mut models {
                    strip_transport(&mut model["profile_snapshot_json"]);
                }
                let mut attempts = Vec::new();
                for attempt in rows(
                    &c,
                    "SELECT a.* FROM benchmark_attempts a JOIN benchmark_case_runs cr ON cr.id=a.case_run_id WHERE a.workspace_id=?1 AND cr.run_id=?2 ORDER BY cr.id,a.attempt_no,a.id",
                    [workspace, run],
                )? {
                    let aid = attempt["id"].as_str().ok_or("尝试编号缺失。")?;
                    let mut saved = json!({"record":attempt,"execution":null,"messages":[],"snapshot":null,"executionEvents":[],"evaluations":rows(&c,"SELECT * FROM benchmark_evaluations WHERE workspace_id=?1 AND attempt_id=?2 ORDER BY created_at,id",[workspace,aid])?,"resources":rows(&c,"SELECT * FROM benchmark_resource_samples WHERE workspace_id=?1 AND attempt_id=?2 ORDER BY seq",[workspace,aid])?});
                    if let Some(execution) = attempt["execution_run_id"].as_str() {
                        let execution_record=rows(&c,"SELECT r.* FROM execution_runs r WHERE r.workspace_id=?1 AND r.id=?2 AND EXISTS(SELECT 1 FROM conversations cv WHERE cv.id=r.conversation_id AND cv.workspace_id=r.workspace_id AND cv.purpose='benchmark')",[workspace,execution])?.pop().ok_or("执行记录不属于当前评测。")?;
                        let mut snapshot = rows(
                            &c,
                            "SELECT * FROM context_snapshots WHERE run_id=?1",
                            [execution],
                        )?
                        .pop()
                        .unwrap_or(Value::Null);
                        if let Some(request) = snapshot.get_mut("request_json") {
                            strip_transport(request);
                        }
                        saved["messages"] = json!(rows(
                            &c,
                            "SELECT m.* FROM messages m JOIN execution_runs r ON r.id=m.model_run_id WHERE r.workspace_id=?1 AND r.id=?2 AND EXISTS(SELECT 1 FROM conversations cv WHERE cv.id=r.conversation_id AND cv.workspace_id=r.workspace_id AND cv.purpose='benchmark') ORDER BY m.created_at,m.id",
                            [workspace, execution]
                        )?);
                        saved["executionEvents"] = json!(rows(
                            &c,
                            "SELECT * FROM execution_events WHERE run_id=?1 ORDER BY seq",
                            [execution]
                        )?);
                        saved["snapshot"] = snapshot;
                        saved["execution"] = execution_record;
                    }
                    strip_preflight_events(&mut saved["executionEvents"]);
                    attempts.push(saved);
                }
                entry["caseRuns"] = json!(rows(
                    &c,
                    "SELECT * FROM benchmark_case_runs WHERE workspace_id=?1 AND run_id=?2 ORDER BY model_slot,case_id,repetition,id",
                    [workspace, run]
                )?);
                entry["run"] = run_record;
                entry["models"] = json!(models);
                entry["attempts"] = json!(attempts);
                entry["events"] = json!(rows(
                    &c,
                    "SELECT * FROM benchmark_events WHERE workspace_id=?1 AND run_id=?2 ORDER BY seq",
                    [workspace, run]
                )?);
            }
            strip_preflight_events(&mut entry["events"]);
            items.push(entry);
        }
        let dataset = rows(
            &c,
            "SELECT * FROM benchmark_datasets WHERE id=?1 AND version=?2",
            rusqlite::params![
                batch["plan"]["input"]["datasetId"].as_str(),
                batch["plan"]["input"]["datasetVersion"].as_i64()
            ],
        )?
        .pop()
        .ok_or("题库版本缺失。")?;
        if revision(&c)? != before {
            return Err("导出期间数据发生变化，请重新导出；尚未写入文件。".into());
        }
        let payload = json!({"workspaceId":workspace,"scope":if matches!(scope,ReportScope::Standard){"standard"}else{"personalized"},"originalReport":report,"dataset":dataset,"items":items,"scoringView":"original","priceSnapshot":batch["plan"]["input"]["price"],"limits":{"hiddenDatasets":"not_supported","csv":"review_table_json_is_lossless","snapshot":"validated_change_counters","transportRedaction":"connection_metadata_only_source_hashes_refer_to_original"}});
        if payload["dataset"]["manifest_json"]["level"] == "capability"
            || payload["dataset"]["manifest_json"]["level"] == "cognitive"
        {
            return Err("此导出协议尚未开放隐藏测试题。".into());
        }
        let hash = dataset::hash(serde_json::to_vec(&payload).map_err(json_error)?.as_slice());
        Ok(
            json!({"reportSchema":"cortexos.benchmark-report","schemaVersion":1,"createdAt":now(),"applicationVersion":crate::app_version(),"contentHash":hash,"payload":payload}),
        )
    }
}
pub fn validate_report(report: &Value) -> AppResult<()> {
    if report["reportSchema"] != "cortexos.benchmark-report"
        || report["schemaVersion"] != 1
        || report["contentHash"]
            != dataset::hash(&serde_json::to_vec(&report["payload"]).map_err(json_error)?)
    {
        return Err("报告版本或内容校验不通过。".into());
    }
    Ok(())
}
pub(crate) fn cell(value: &Value) -> String {
    let text = match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    };
    let start = text.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let unsafe_start =
        start.starts_with(['=', '+', '-', '@']) || text.starts_with(['\t', '\r', '\n']);
    format!(
        "\"{}{}\"",
        if unsafe_start { "'" } else { "" },
        text.replace('"', "\"\"")
    )
}
pub fn csv_report(report: &Value) -> AppResult<Vec<u8>> {
    validate_report(report)?;
    let p = &report["payload"];
    let batch = &p["originalReport"];
    let plan = &batch["plan"]["input"];
    let mut lines=vec!["schema_version,report_hash,workspace_id,scope,batch_id,dataset_id,dataset_version,dataset_hash,plan_hash,model_index,model_name,case_id,repetition,ordinal,run_id,attempt_id,attempt_no,attempt_status,execution_run_id,execution_status,request_disposition,evaluation_id,grading_view,session_id,evaluator_version,scoring_hash,output_hash,state,question,output,native_calls_json,configuration_json,metrics_json,details_json,request_snapshot_json,hardware_json,price_snapshot_json".into()];
    for entry in p["items"].as_array().ok_or("报告项目缺失。")? {
        let i = &entry["item"];
        let empty = json!({});
        let attempts = entry["attempts"].as_array().ok_or("尝试数据缺失。")?;
        let placeholders = vec![empty.clone()];
        for a in if attempts.is_empty() {
            &placeholders
        } else {
            attempts
        } {
            let evaluations = a["evaluations"]
                .as_array()
                .filter(|v| !v.is_empty())
                .unwrap_or(&placeholders);
            for e in evaluations {
                let execution = &a["execution"];
                let text = a["messages"]
                    .as_array()
                    .and_then(|m| {
                        m.iter()
                            .find(|m| m["id"] == execution["assistant_message_id"])
                    })
                    .map(|m| m["content"].clone())
                    .unwrap_or(Value::Null);
                let fields = vec![
                    json!(1),
                    report["contentHash"].clone(),
                    p["workspaceId"].clone(),
                    p["scope"].clone(),
                    batch["id"].clone(),
                    plan["datasetId"].clone(),
                    plan["datasetVersion"].clone(),
                    plan["datasetHash"].clone(),
                    batch["planHash"].clone(),
                    i["modelIndex"].clone(),
                    batch["reports"]
                        .as_array()
                        .and_then(|r| r.iter().find(|r| r["modelIndex"] == i["modelIndex"]))
                        .map(|r| r["name"].clone())
                        .unwrap_or(Value::Null),
                    i["caseId"].clone(),
                    i["repetition"].clone(),
                    i["ordinal"].clone(),
                    i["runId"].clone(),
                    a["record"]["id"].clone(),
                    a["record"]["attempt_no"].clone(),
                    a["record"]["status"].clone(),
                    a["record"]["execution_run_id"].clone(),
                    execution["status"].clone(),
                    a["record"]["request_disposition"].clone(),
                    e["id"].clone(),
                    json!(if e["details_json"].get("regrade").is_some() {
                        "regrade"
                    } else {
                        "original"
                    }),
                    e["details_json"]["regrade"]["sessionId"].clone(),
                    e["evaluator_version"].clone(),
                    e["scoring_config_hash"].clone(),
                    e["output_hash"].clone(),
                    e["details_json"].get("state").cloned().unwrap_or_else(|| {
                        json!(match execution["status"]
                            .as_str()
                            .or(a["record"]["status"].as_str())
                            .or(i["status"].as_str())
                        {
                            Some("completed") => "Needs Review",
                            Some("running") => "In Progress",
                            Some("cancelled" | "interrupted") => "Cancelled",
                            Some("failed" | "error" | "blocked") => "Execution Error",
                            Some("unsupported") => "Unsupported",
                            _ => "Not Run",
                        })
                    }),
                    entry["case"]["input_json"]["prompt"].clone(),
                    text,
                    json!(
                        a["executionEvents"]
                            .as_array()
                            .map(|events| events
                                .iter()
                                .filter(|e| e["kind"] == "native_tool_calls_received")
                                .map(|e| e["payload_json"].clone())
                                .collect::<Vec<_>>())
                            .unwrap_or_default()
                    ),
                    entry["run"]["config_snapshot_json"].clone(),
                    e["metrics_json"].clone(),
                    e["details_json"].clone(),
                    a["snapshot"].clone(),
                    entry["run"]["hardware_snapshot_json"].clone(),
                    p["priceSnapshot"].clone(),
                ];
                lines.push(fields.iter().map(cell).collect::<Vec<_>>().join(","));
            }
        }
    }
    Ok(format!("\u{feff}{}\r\n", lines.join("\r\n")).into_bytes())
}
/// Publish only complete new files. Existing destinations remain untouched, including write errors.
pub fn write_new(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .filter(|p| p.is_dir())
        .ok_or("保存目录不存在，请重新选择。")?;
    if path.exists() {
        return Err("文件已存在，请选择新文件名，原文件未改动。".into());
    }
    let temp = parent.join(format!(".cortexos-report-{}.tmp", new_id()));
    let mut created = false;
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "无法创建报告临时文件。")?;
        created = true;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "报告写入失败，目标文件未改动。")?;
        drop(f);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
            let from = temp
                .as_os_str()
                .encode_wide()
                .chain([0])
                .collect::<Vec<_>>();
            let to = path
                .as_os_str()
                .encode_wide()
                .chain([0])
                .collect::<Vec<_>>();
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
                return Err("保存未完成，请选择新文件名或检查目录权限，已有文件未改动。".into());
            }
        }
        #[cfg(not(windows))]
        {
            fs::hard_link(&temp, path).map_err(|_| "保存未完成，已有文件未改动。")?;
        }
        Ok(())
    })();
    if created {
        let _ = fs::remove_file(temp);
    }
    result
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Csv,
}
#[tauri::command]
pub async fn export_benchmark_report(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    format: Format,
) -> AppResult<bool> {
    let lease = state.claim_task(&new_id(), &workspace_id, TaskKind::Benchmark)?;
    let cancelled = lease.receiver();
    let db = state.database.clone()?;
    let cloud = cloud.inner().clone();
    let bytes = blocking(db, move |db| {
        access(&cloud, scope, || {
            let report = db.benchmark_export(&workspace_id, &batch_id, scope)?;
            match format {
                Format::Json => serde_json::to_vec_pretty(&report).map_err(json_error),
                Format::Csv => csv_report(&report),
            }
        })
    })
    .await?;
    if *cancelled.borrow() {
        return Err("导出已取消，没有写入文件。".into());
    }
    drop(lease);
    tauri::async_runtime::spawn_blocking(move || {
        let ext = if matches!(format, Format::Json) {
            "json"
        } else {
            "csv"
        };
        let path = app
            .dialog()
            .file()
            .set_title("保存评测报告")
            .add_filter("评测报告", &[ext])
            .set_file_name(format!("CortexOS-评测报告-{}.{}", new_id(), ext))
            .blocking_save_file();
        let Some(path) = path else {
            return Ok(false);
        };
        write_new(
            &path.into_path().map_err(|_| "仅支持本机保存位置。")?,
            &bytes,
        )?;
        Ok(true)
    })
    .await
    .map_err(|_| "保存窗口异常，请重试。".to_string())?
}
