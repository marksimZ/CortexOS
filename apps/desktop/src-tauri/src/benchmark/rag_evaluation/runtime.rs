use super::*;
use crate::{
    db::json_error, ollama::Ollama, runtime_profiles::CloudTargets, runtime_usage::Meter,
    tasks::TaskKind,
};
use std::sync::Arc;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::watch;

mod execute;
mod plan;
#[cfg(test)]
mod tests;
use plan::{Arm, Input, Plan};

fn persist(db: &Database, workspace: &str, report: &Value, status: &str) -> AppResult<()> {
    let raw = report.to_string();
    let fingerprint = hash(raw.as_bytes());
    let id = report["id"].as_str().ok_or("对照记录缺少编号。")?;
    let finished = (status != "running").then(now);
    let changed = db.connection()?.execute("INSERT INTO rag_runtime_experiments
         (workspace_id,id,created_at,finished_at,status,report_json,report_hash) VALUES (?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(workspace_id,id) DO UPDATE SET finished_at=excluded.finished_at,
           status=excluded.status,report_json=excluded.report_json,report_hash=excluded.report_hash
         WHERE rag_runtime_experiments.status='running'",
        params![workspace,id,report["createdAt"].as_str(),finished,status,raw,fingerprint]).map_err(error)?;
    if changed != 1 {
        return Err("此对照已归档。".into());
    }
    Ok(())
}

fn read(db: &Database, workspace: &str, selected: Option<&str>) -> AppResult<Value> {
    let c = db.connection()?;
    require_workspace(&c, workspace)?;
    let history = c
        .prepare(
            "SELECT id,created_at,status,finished_at FROM rag_runtime_experiments
        WHERE workspace_id=?1 ORDER BY created_at DESC,id DESC",
        )
        .map_err(error)?
        .query_map([workspace], |r| {
            Ok(
                json!({"id":r.get::<_,String>(0)?,"createdAt":r.get::<_,String>(1)?,
            "status":r.get::<_,String>(2)?,"finishedAt":r.get::<_,Option<String>>(3)?}),
            )
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    let id = selected.or_else(|| history.first().and_then(|row| row["id"].as_str()));
    let report = id
        .map(|id| -> AppResult<Value> {
            let (raw, digest, status, finished): (String, String, String, Option<String>) = c
                .query_row(
                    "SELECT report_json,report_hash,status,finished_at
            FROM rag_runtime_experiments WHERE workspace_id=?1 AND id=?2",
                    params![workspace, id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()
                .map_err(error)?
                .ok_or("找不到此工作区的模型组合对照。")?;
            if hash(raw.as_bytes()) != digest {
                return Err("模型组合对照校验失败。".into());
            }
            let mut report: Value = serde_json::from_str(&raw).map_err(json_error)?;
            report["status"] = json!(status);
            report["finishedAt"] = json!(finished);
            report["cloudCalls"] = json!(cloud_calls(&c, workspace, id)?);
            report["localEmbeddings"] = json!(local_embeddings(&c, workspace, id)?);
            let embeddings = report["localEmbeddings"].clone();
            for (side, row) in report["results"]
                .as_array_mut()
                .into_iter()
                .flatten()
                .enumerate()
            {
                if status == "interrupted" {
                    for case in row["cases"].as_array_mut().into_iter().flatten() {
                        if case["status"] == "running" {
                            case["status"] = json!("interrupted");
                        }
                    }
                }
                let prefix = format!("{id}/{side}/");
                let mut calls = report_usage(&c, workspace, id, side)?;
                let embedding_usage =
                    embeddings
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|embedding| {
                            embedding["operationId"]
                                .as_str()
                                .is_some_and(|op| op.starts_with(&prefix))
                        });
                let cases = row["cases"].as_array().into_iter().flatten();
                let model_usage = cases
                    .clone()
                    .flat_map(|case| case["execution"]["events"].as_array().into_iter().flatten())
                    .filter(|event| event["kind"] == "local_model_usage")
                    .map(|event| &event["payload"]);
                let sum = |values: Vec<u64>| {
                    (!values.is_empty()).then(|| values.into_iter().sum::<u64>())
                };
                calls["localInputTokens"] = json!(sum(model_usage
                    .clone()
                    .chain(embedding_usage.clone())
                    .filter_map(|value| value["inputTokens"].as_u64())
                    .collect()));
                calls["localOutputTokens"] = json!(sum(model_usage
                    .clone()
                    .filter_map(|value| value["outputTokens"].as_u64())
                    .collect()));
                calls["localSeconds"] = json!(
                    sum(model_usage
                        .chain(embedding_usage.clone())
                        .filter_map(|value| value["elapsedMs"].as_u64())
                        .collect())
                    .map(|ms| ms as f64 / 1000.)
                );
                calls["localEmbeddingCalls"] = json!(embedding_usage.count());
                calls["systemCpuPeak"] = json!(
                    cases
                        .clone()
                        .filter_map(|case| case["resources"]["after"]["systemCpuPercent"].as_f64())
                        .reduce(f64::max)
                );
                calls["gpuUsedPeakBytes"] = json!(
                    cases
                        .flat_map(|case| case["resources"]["gpuAfter"]["devices"]
                            .as_array()
                            .into_iter()
                            .flatten())
                        .filter_map(|gpu| gpu["usedBytes"].as_u64())
                        .max()
                );
                row["usage"] = calls;
                row["operationPrefix"] = json!(prefix);
            }
            Ok(report)
        })
        .transpose()?;
    Ok(json!({"history":history,"report":report,
        "cases":Suite::load()?.cases.into_iter().map(|case|json!({"id":case.id,"question":case.question})).collect::<Vec<_>>()}))
}

fn cloud_calls(c: &rusqlite::Connection, workspace: &str, id: &str) -> AppResult<Vec<Value>> {
    c.prepare("SELECT id,operation_id,provider,connection_id,model,capability,data_types_json,started_unix,state,
          estimate_micros,settled_micros,input_tokens,output_tokens,vector_count,elapsed_ms,failure
          FROM model_cloud_calls WHERE workspace_id=?1 AND purpose='rag_experiment' AND operation_id GLOB ?2
          ORDER BY started_unix,id").map_err(error)?
        .query_map(params![workspace,format!("{id}/*")], |r| Ok(json!({
            "id":r.get::<_,String>(0)?,"operationId":r.get::<_,String>(1)?,"provider":r.get::<_,String>(2)?,
            "connectionId":r.get::<_,String>(3)?,"model":r.get::<_,String>(4)?,"capability":r.get::<_,String>(5)?,
            "dataTypes":serde_json::from_str::<Value>(&r.get::<_,String>(6)?).unwrap_or_default(),
            "startedUnix":r.get::<_,i64>(7)?,"status":r.get::<_,String>(8)?,"reservedMicros":r.get::<_,Option<i64>>(9)?,
            "settledMicros":r.get::<_,Option<i64>>(10)?,"inputTokens":r.get::<_,Option<i64>>(11)?,"outputTokens":r.get::<_,Option<i64>>(12)?,
            "vectors":r.get::<_,Option<i64>>(13)?,"elapsedMs":r.get::<_,Option<i64>>(14)?,"error":r.get::<_,Option<String>>(15)?
        }))).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)
}

fn local_embeddings(c: &rusqlite::Connection, workspace: &str, id: &str) -> AppResult<Vec<Value>> {
    c.prepare("SELECT id,operation_id,model,started_unix,state,input_tokens,vector_count,elapsed_ms FROM model_local_embeddings
        WHERE workspace_id=?1 AND purpose='rag_experiment' AND operation_id GLOB ?2 ORDER BY started_unix,id").map_err(error)?
        .query_map(params![workspace,format!("{id}/*")], |r| Ok(json!({
            "id":r.get::<_,String>(0)?,"operationId":r.get::<_,String>(1)?,"model":r.get::<_,String>(2)?,
            "startedUnix":r.get::<_,i64>(3)?,"status":r.get::<_,String>(4)?,"inputTokens":r.get::<_,Option<i64>>(5)?,
            "vectors":r.get::<_,Option<i64>>(6)?,"elapsedMs":r.get::<_,Option<i64>>(7)?
        }))).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)
}

fn report_usage(
    c: &rusqlite::Connection,
    workspace: &str,
    id: &str,
    side: usize,
) -> AppResult<Value> {
    let pattern = format!("{id}/{side}/*");
    c.query_row("SELECT COUNT(*),SUM(settled_micros),COUNT(*) FILTER (WHERE settled_micros IS NULL),
        SUM(estimate_micros) FILTER (WHERE settled_micros IS NULL),SUM(input_tokens),SUM(output_tokens),
        COUNT(*) FILTER (WHERE capability='embedding'),COUNT(*) FILTER (WHERE capability='reranking')
        FROM model_cloud_calls WHERE workspace_id=?1 AND purpose='rag_experiment' AND operation_id GLOB ?2",
        params![workspace,pattern],|r|Ok(json!({
            "cloudCalls":r.get::<_,i64>(0)?,"settledMicros":r.get::<_,Option<i64>>(1)?,
            "unknownChargeCalls":r.get::<_,i64>(2)?,"unsettledReservedMicros":r.get::<_,Option<i64>>(3)?,
            "knownCloudInputTokens":r.get::<_,Option<i64>>(4)?,"knownCloudOutputTokens":r.get::<_,Option<i64>>(5)?,
            "cloudEmbeddingCalls":r.get::<_,i64>(6)?,"cloudRerankingCalls":r.get::<_,i64>(7)?
        }))).map_err(error)
}

async fn run(
    owner: Arc<Database>,
    workspace: &str,
    id: &str,
    suite: &Suite,
    plan: &Plan,
    provider: Option<&Ollama>,
    cancel: watch::Receiver<bool>,
) -> AppResult<Value> {
    let mut report = json!({"id":id,"createdAt":now(),"formatVersion":1,"plan":plan.audit,"planHash":plan.hash,
        "dataset":suite,"mode":"live_runtime","results":plan.arms.iter().map(|arm|
        json!({"arm":arm.audit,"cases":suite.cases.iter().filter(|c|plan.case_ids.contains(&c.id)).
            map(|c|json!({"id":c.id,"question":c.question,"status":"not_run"})).collect::<Vec<_>>()})
        ).collect::<Vec<_>>()});
    persist(&owner, workspace, &report, "running")?;
    let cap = plan.audit["costCapMicros"].as_u64();
    let mut any_error = false;
    for (side, arm) in plan.arms.iter().enumerate() {
        if *cancel.borrow() {
            break;
        }
        let unit = format!("{side}/index");
        let setup = async {
            let targets = CloudTargets::resolve(&owner, workspace, Some(&arm.profile))?;
            let corpus = execute::Corpus::open(
                suite,
                &suite.snapshot_date,
                &targets,
                plan.local_preferences,
            )?;
            let meter = Meter::fixed_rag_experiment(
                owner.clone(),
                workspace,
                corpus.database.clone(),
                WORKSPACE,
                id,
                &unit,
                "",
                cap,
            );
            let started = std::time::Instant::now();
            let index =
                execute::index(&corpus, arm, &targets, provider, &meter, cancel.clone()).await?;
            Ok::<_, String>((targets, corpus, index, started.elapsed().as_millis() as u64))
        }
        .await;
        let (targets, corpus, index, index_ms) = match setup {
            Ok(data) => data,
            Err(reason) => {
                any_error = true;
                report["results"][side]["setupError"] = json!(reason);
                persist(&owner, workspace, &report, "running")?;
                continue;
            }
        };
        report["results"][side]["index"] = index;
        report["results"][side]["indexBuildMs"] = json!(arm.policy.semantic().then_some(index_ms));
        persist(&owner, workspace, &report, "running")?;
        let mut cases = Vec::new();
        for (position, case) in suite
            .cases
            .iter()
            .filter(|case| plan.case_ids.contains(&case.id))
            .enumerate()
        {
            if *cancel.borrow() {
                break;
            }
            let date = if arm.policy == Policy::VersionAware {
                case.fact_date.as_deref().unwrap_or(&suite.snapshot_date)
            } else {
                &suite.snapshot_date
            };
            let historical = (date != suite.snapshot_date)
                .then(|| execute::Corpus::open(suite, date, &targets, plan.local_preferences))
                .transpose();
            let unit = format!("{side}/{}", case.id);
            let run_id = new_id();
            report["results"][side]["cases"][position]["status"] = json!("running");
            report["results"][side]["cases"][position]["operationId"] =
                json!(format!("{id}/{unit}"));
            persist(&owner, workspace, &report, "running")?;
            let execution = match historical {
                Ok(ref historical) => {
                    let corpus = historical.as_ref().unwrap_or(&corpus);
                    let meter = Meter::fixed_rag_experiment(
                        owner.clone(),
                        workspace,
                        corpus.database.clone(),
                        WORKSPACE,
                        id,
                        &unit,
                        &run_id,
                        cap,
                    );
                    execute::case(
                        corpus,
                        arm,
                        &targets,
                        provider,
                        case,
                        date,
                        &run_id,
                        &meter,
                        cancel.clone(),
                    )
                    .await
                }
                Err(error) => Err(error),
            };
            let mut row = execution.unwrap_or_else(|error| {
                json!({"id":case.id,"question":case.question,
                "status":if *cancel.borrow() {"cancelled"} else {"execution_error"},"error":error})
            });
            any_error |= row["status"] == "execution_error" || row["status"] == "scoring_error";
            row["operationId"] = json!(format!("{id}/{unit}"));
            row["effectiveDate"] = json!(date);
            cases.push(row.clone());
            report["results"][side]["cases"][position] = row;
            report["results"][side]["summary"] = score::summarize(&cases);
            persist(&owner, workspace, &report, "running")?;
        }
    }
    let status = if *cancel.borrow() {
        "cancelled"
    } else if any_error {
        "failed"
    } else {
        "completed"
    };
    persist(&owner, workspace, &report, status)?;
    Ok(read(&owner, workspace, Some(id))?["report"].clone())
}

#[tauri::command]
pub async fn preview_rag_runtime_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
) -> AppResult<Value> {
    let database = state.database.clone()?;
    let provider = state.provider.as_ref().ok();
    let plan = plan::prepare(&database, &workspace_id, &Suite::load()?, &input, provider).await?;
    Ok(json!({"plan":plan.audit,"planHash":plan.hash,"id":new_id()}))
}

#[tauri::command]
pub async fn run_rag_runtime_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
    id: String,
    plan_hash: String,
) -> AppResult<Value> {
    let identifier = uuid::Uuid::parse_str(&id).map_err(|_| "对照编号无效。")?;
    if identifier.to_string() != id {
        return Err("对照编号无效。".into());
    }
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let _guard = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "另一项云端任务正在进行。")?;
    let database = state.database.clone()?;
    let provider = state.provider.as_ref().ok();
    let suite = Suite::load()?;
    let prepared = plan::prepare(&database, &workspace_id, &suite, &input, provider).await?;
    if prepared.hash != plan_hash {
        return Err("模型、配置或费用规则已变化，请重新检查实验计划。".into());
    }
    if database
        .connection()?
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM rag_runtime_experiments WHERE workspace_id=?1 AND id=?2)",
            params![workspace_id, id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(error)?
    {
        return Err("此对照已经发起，请重新预览。".into());
    }
    run(
        database,
        &workspace_id,
        &id,
        &suite,
        &prepared,
        provider,
        lease.receiver(),
    )
    .await
}

#[tauri::command]
pub fn cancel_rag_runtime_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    id: String,
) -> AppResult<()> {
    state.cancel_personalized_task(&workspace_id, &id)
}

#[tauri::command]
pub async fn get_rag_runtime_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        read(db, &workspace_id, id.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn export_rag_runtime_experiment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
    id: String,
) -> AppResult<bool> {
    let export_id = id.clone();
    let bytes = blocking(state.database.clone()?, move |db| {
        let value = read(db, &workspace_id, Some(&export_id))?;
        serde_json::to_vec_pretty(&value["report"]).map_err(json_error)
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_title("保存模型组合对照")
            .add_filter("JSON", &["json"])
            .set_file_name(format!("CortexOS-RAG-{id}.json"))
            .blocking_save_file();
        let Some(path) = path else { return Ok(false) };
        crate::benchmark::report_transfer::write_new(
            &path.into_path().map_err(|_| "仅支持本机保存位置。")?,
            &bytes,
        )?;
        Ok(true)
    })
    .await
    .map_err(|_| "保存窗口异常，请重试。".to_owned())?
}
