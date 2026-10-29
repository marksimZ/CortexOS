//! Regrade immutable saved answers into separate append-only sessions; original reports retain their original grade.
use super::{
    dataset,
    evaluation::{self, State as GradeState},
    python,
    report_read::ReportScope,
    report_transfer::{access, rows},
};
use crate::{
    cloud::CloudState,
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
    tasks::TaskKind,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tauri::State;
use tokio::sync::watch;
fn check(cancel: &Option<watch::Receiver<bool>>) -> AppResult<()> {
    if cancel.as_ref().is_some_and(|c| *c.borrow()) {
        Err("重新评分已取消，本轮没有保存新记录。".into())
    } else {
        Ok(())
    }
}
fn sources(c: &Connection, workspace: &str, batch: &str) -> AppResult<Vec<Value>> {
    let mut found = rows(
        c,
        "SELECT i.ordinal,i.model_index,i.case_id,(i.ordinal % max(1,json_array_length(bb.plan_json,'$.cases')) / max(1,json_array_length(bb.plan_json,'$.cases') / coalesce(json_extract(bb.plan_json,'$.input.repetitions'),1)) + 1) AS repetition,cr.run_id,a.id AS attempt_id,a.attempt_no,a.execution_run_id,r.status AS execution_status,m.content AS output,b.config_snapshot_json,b.dataset_version,bc.content_hash AS case_hash,e.id AS source_evaluation_id,e.output_hash AS source_output_hash FROM benchmark_batch_items i JOIN benchmark_batches bb ON bb.id=i.batch_id JOIN benchmark_case_runs cr ON cr.run_id=i.run_id AND cr.workspace_id=bb.workspace_id JOIN benchmark_runs b ON b.id=cr.run_id JOIN benchmark_cases bc ON bc.dataset_id=cr.dataset_id AND bc.dataset_version=cr.dataset_version AND bc.case_id=cr.case_id JOIN benchmark_attempts a ON a.case_run_id=cr.id LEFT JOIN execution_runs r ON r.id=a.execution_run_id AND r.workspace_id=a.workspace_id AND EXISTS(SELECT 1 FROM conversations cv WHERE cv.id=r.conversation_id AND cv.workspace_id=r.workspace_id AND cv.purpose='benchmark') LEFT JOIN messages m ON m.id=r.assistant_message_id LEFT JOIN benchmark_evaluations e ON e.id=(SELECT ee.id FROM benchmark_evaluations ee WHERE ee.attempt_id=a.id AND json_type(ee.details_json,'$.regrade') IS NULL ORDER BY ee.created_at DESC,ee.id DESC LIMIT 1) WHERE bb.workspace_id=?1 AND bb.id=?2 ORDER BY i.ordinal,a.attempt_no,a.id",
        [workspace, batch],
    )?;
    for source in &mut found {
        if source["config_snapshot_json"]["input"]["caseId"] != source["case_id"] {
            return Err("已保存运行与批次题目不一致。".into());
        }
        let modern = source["dataset_version"] == 2
            || source["config_snapshot_json"]["mode"] == "personalized_single_case_v1";
        source["eligible"] = json!(
            source["execution_status"] == "completed"
                && source["source_evaluation_id"].is_string()
                && modern
        );
        source["nativeFrames"]=json!(rows(c,"SELECT payload_json FROM execution_events WHERE run_id=?1 AND kind='native_tool_calls_received' ORDER BY seq",[source["execution_run_id"].as_str()])?.into_iter().map(|r|r["payload_json"].clone()).collect::<Vec<_>>());
        if source["eligible"] == true {
            let hash = dataset::hash(
                &serde_json::to_vec(
                    &json!({"text":source["output"],"nativeFrames":source["nativeFrames"]}),
                )
                .map_err(json_error)?,
            );
            if source["source_output_hash"] != hash {
                return Err("已保存回答与原评分的内容标识不一致，已停止重新评分。".into());
            }
        }
    }
    Ok(found)
}
fn implementation_hash() -> String {
    dataset::hash(
        concat!(
            include_str!("evaluation.rs"),
            include_str!("scoring_repository.rs"),
            include_str!("evaluation_json.rs"),
            include_str!("python.rs"),
            include_str!("python_runner.py"),
            include_str!("../../Cargo.lock")
        )
        .as_bytes(),
    )
}
fn fingerprint(batch: &Value, sources: &[Value]) -> AppResult<String> {
    if sources.iter().any(|s| {
        s["config_snapshot_json"]["datasetId"] != batch["plan"]["input"]["datasetId"]
            || s["dataset_version"] != batch["plan"]["input"]["datasetVersion"]
    }) {
        return Err("评分来源与冻结题库不一致，已停止。".into());
    }
    Ok(dataset::hash(&serde_json::to_vec(&json!({"batchPlanHash":batch["planHash"],"batchStatus":batch["status"],"engine":evaluation::ENGINE_VERSION,"implementationHash":implementation_hash(),"sandboxPolicy":python::policy(),"sources":sources})).map_err(json_error)?))
}
impl Database {
    pub fn preview_benchmark_regrade(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
    ) -> AppResult<Value> {
        let batch =
            self.require_batch_scope(workspace, id, matches!(scope, ReportScope::Standard))?;
        if matches!(batch["status"].as_str(), Some("running" | "stopping")) {
            return Err("请先完成或停止评测，再重新评分。".into());
        }
        let found = sources(&*self.connection()?, workspace, id)?;
        let eligible = found.iter().filter(|r| r["eligible"] == true).count();
        let python_count = found
            .iter()
            .filter(|r| {
                r["eligible"] == true
                    && r["config_snapshot_json"]["caseScoring"]["id"] == "python_assertions"
            })
            .count();
        Ok(
            json!({"sessionId":format!("regrade-{}",new_id()),"planHash":fingerprint(&batch,&found)?,"engineVersion":evaluation::ENGINE_VERSION,"implementationHash":implementation_hash(),"eligible":eligible,"skippedAttempts":found.len()-eligible,"plannedItems":batch["items"].as_array().map_or(0,|a|a.len()),"pythonCases":python_count}),
        )
    }
    pub fn regrade_benchmark(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        session: &str,
        approved: &str,
        cancel: Option<watch::Receiver<bool>>,
    ) -> AppResult<Value> {
        if !session
            .strip_prefix("regrade-")
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
        {
            return Err("重新评分编号无效。".into());
        }
        let batch =
            self.require_batch_scope(workspace, id, matches!(scope, ReportScope::Standard))?;
        if matches!(batch["status"].as_str(), Some("running" | "stopping")) {
            return Err("评测仍在运行，不能重新评分。".into());
        }
        let source = sources(&*self.connection()?, workspace, id)?;
        if fingerprint(&batch, &source)? != approved {
            return Err("评分输入发生变化，请重新预览并确认。".into());
        }
        let mut scores = Vec::new();
        for item in source.iter().filter(|i| i["eligible"] == true) {
            check(&cancel)?;
            let score = self.calculate_saved_benchmark_score(
                workspace,
                item["run_id"].as_str().ok_or("运行编号缺失。")?,
                item["attempt_id"].as_str(),
                cancel.clone(),
            )?;
            scores.push((item, score));
        }
        check(&cancel)?;
        if scores.is_empty() {
            return Err("没有可重新评分的已完成回答；未运行、失败或旧协议记录保持原样。".into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let current_status: String = tx
            .query_row(
                "SELECT status FROM benchmark_batches WHERE workspace_id=?1 AND id=?2",
                [workspace, id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if current_status != batch["status"].as_str().unwrap_or("")
            || fingerprint(&batch, &sources(&tx, workspace, id)?)? != approved
        {
            return Err("评分输入已变化，本轮没有保存，请重新预览。".into());
        }
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_evaluations WHERE json_extract(details_json,'$.regrade.sessionId')=?1 AND workspace_id=?2 AND json_extract(details_json,'$.regrade.batchId')=?3)",[session,workspace,id],|r|r.get(0)).map_err(error)?;
        if exists {
            return Err("这轮重新评分已经保存，请查看历史记录。".into());
        }
        let created = now();
        let mut tuples = Vec::new();
        for (item, mut score) in scores {
            check(&cancel)?;
            if matches!(score.state, GradeState::NotRun | GradeState::InProgress) {
                return Err("评分器返回了非终态，本轮没有保存。".into());
            }
            let regrade = json!({"sessionId":session,"batchId":id,"scope":if matches!(scope,ReportScope::Standard){"standard"}else{"personalized"},"createdAt":created,"inputHash":approved,"modelRequests":0,"statisticsBasis":"saved_completed_answers","sourcePlanItems":batch["items"].as_array().map_or(0,|v|v.len()),"ordinal":item["ordinal"],"modelIndex":item["model_index"],"caseId":item["case_id"],"repetition":item["repetition"],"attemptNo":item["attempt_no"],"sourceEvaluationId":item["source_evaluation_id"],"sourceOutputHash":item["source_output_hash"],"caseHash":item["case_hash"],"engineVersion":evaluation::ENGINE_VERSION,"implementationHash":implementation_hash(),"sandboxPolicy":python::policy()});
            score.details["state"] = serde_json::to_value(score.state).map_err(json_error)?;
            score.details["attempted"] = json!(true);
            score.details["executionSucceeded"] = json!(true);
            score.details["regrade"] = regrade;
            let scoring_hash=dataset::hash(&serde_json::to_vec(&json!({"caseScoring":item["config_snapshot_json"]["caseScoring"],"rule":item["config_snapshot_json"]["scoringRule"],"engine":evaluation::ENGINE_VERSION,"implementationHash":implementation_hash(),"sandboxPolicy":python::policy(),"image":item["config_snapshot_json"]["input"]["pythonImage"]})).map_err(json_error)?);
            let status = if matches!(score.state, GradeState::Passed | GradeState::Failed) {
                "completed"
            } else if score.state == GradeState::NeedsReview {
                "needs_review"
            } else {
                "failed"
            };
            tx.execute(
                "INSERT INTO benchmark_evaluations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    new_id(),
                    workspace,
                    item["attempt_id"].as_str(),
                    item["config_snapshot_json"]["scoringRule"]["id"].as_str(),
                    evaluation::ENGINE_VERSION,
                    scoring_hash,
                    item["source_output_hash"].as_str(),
                    score.metrics.to_string(),
                    score.details.to_string(),
                    status,
                    created
                ],
            )
            .map_err(error)?;
            tuples.push((score, true, true));
        }
        check(&cancel)?;
        tx.commit().map_err(error)?;
        Ok(
            json!({"sessionId":session,"createdAt":created,"engineVersion":evaluation::ENGINE_VERSION,"implementationHash":implementation_hash(),"statistics":evaluation::statistics(&tuples)}),
        )
    }
    pub fn benchmark_regrades(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        session: Option<&str>,
        page: u32,
    ) -> AppResult<Value> {
        self.require_batch_scope(workspace, id, matches!(scope, ReportScope::Standard))?;
        let c = self.connection()?;
        let total: u32 = if let Some(session) = session {
            c.query_row("SELECT count(*) FROM benchmark_evaluations e JOIN benchmark_attempts a ON a.id=e.attempt_id JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN benchmark_batch_items i ON i.run_id=cr.run_id WHERE e.workspace_id=?1 AND i.batch_id=?2 AND json_extract(e.details_json,'$.regrade.sessionId')=?3",[workspace,id,session],|r|r.get(0)).map_err(error)?
        } else {
            c.query_row("SELECT count(DISTINCT json_extract(e.details_json,'$.regrade.sessionId')) FROM benchmark_evaluations e JOIN benchmark_attempts a ON a.id=e.attempt_id JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN benchmark_batch_items i ON i.run_id=cr.run_id WHERE e.workspace_id=?1 AND i.batch_id=?2",[workspace,id],|r|r.get(0)).map_err(error)?
        };
        if session.is_some() && total == 0 {
            return Err("本轮评分不存在或不属于当前评测。".into());
        }
        if page > 0 && u64::from(page) * 20 >= u64::from(total) {
            return Err("评分记录页码不存在，请返回第一页。".into());
        }
        if let Some(session) = session {
            Ok(
                json!({"results":rows(&c,"SELECT e.id,e.evaluator_version,e.scoring_config_hash,e.output_hash,e.metrics_json,e.details_json,e.created_at FROM benchmark_evaluations e JOIN benchmark_attempts a ON a.id=e.attempt_id JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN benchmark_batch_items i ON i.run_id=cr.run_id WHERE e.workspace_id=?1 AND i.batch_id=?2 AND json_extract(e.details_json,'$.regrade.sessionId')=?3 AND json_extract(e.details_json,'$.regrade.batchId')=?2 ORDER BY i.ordinal,a.attempt_no,e.id LIMIT 20 OFFSET ?4",params![workspace,id,session,i64::from(page)*20])?,"total":total,"page":page,"pageSize":20}),
            )
        } else {
            Ok(
                json!({"sessions":rows(&c,"SELECT json_extract(e.details_json,'$.regrade.sessionId') AS session_id,min(e.created_at) AS created_at,e.evaluator_version AS engine_version,count(*) AS result_count,sum(json_extract(e.details_json,'$.state')='Passed') AS passed,sum(json_extract(e.details_json,'$.state')='Failed') AS failed FROM benchmark_evaluations e JOIN benchmark_attempts a ON a.id=e.attempt_id JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN benchmark_batch_items i ON i.run_id=cr.run_id WHERE e.workspace_id=?1 AND i.batch_id=?2 AND json_type(e.details_json,'$.regrade') IS NOT NULL AND json_extract(e.details_json,'$.regrade.batchId')=?2 GROUP BY session_id,e.evaluator_version ORDER BY created_at DESC,session_id DESC LIMIT 20 OFFSET ?3",params![workspace,id,i64::from(page)*20])?,"total":total,"page":page,"pageSize":20}),
            )
        }
    }
}
#[tauri::command]
pub async fn preview_benchmark_regrade(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
) -> AppResult<Value> {
    let cloud = cloud.inner().clone();
    blocking(state.database.clone()?, move |db| {
        access(&cloud, scope, || {
            db.preview_benchmark_regrade(&workspace_id, &batch_id, scope)
        })
    })
    .await
}
#[tauri::command]
pub async fn run_benchmark_regrade(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    session_id: String,
    approved_plan_hash: String,
) -> AppResult<Value> {
    if !session_id.starts_with("regrade-") {
        return Err("重新评分编号无效。".into());
    }
    let lease = state.claim_task(&session_id, &workspace_id, TaskKind::Benchmark)?;
    let cancel = lease.receiver();
    let cloud = cloud.inner().clone();
    let result = blocking(state.database.clone()?, move |db| {
        access(&cloud, scope, || {
            db.regrade_benchmark(
                &workspace_id,
                &batch_id,
                scope,
                &session_id,
                &approved_plan_hash,
                Some(cancel),
            )
        })
    })
    .await;
    drop(lease);
    result
}
#[tauri::command]
pub fn cancel_benchmark_regrade(
    state: State<'_, AppState>,
    workspace_id: String,
    session_id: String,
) -> AppResult<()> {
    if !session_id.starts_with("regrade-") {
        return Err("重新评分编号无效。".into());
    }
    state.cancel_personalized_task(&workspace_id, &session_id)
}
#[tauri::command]
pub async fn get_benchmark_regrades(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    session_id: Option<String>,
    page: Option<u32>,
) -> AppResult<Value> {
    let cloud = cloud.inner().clone();
    blocking(state.database.clone()?, move |db| {
        access(&cloud, scope, || {
            db.benchmark_regrades(
                &workspace_id,
                &batch_id,
                scope,
                session_id.as_deref(),
                page.unwrap_or(0),
            )
        })
    })
    .await
}
