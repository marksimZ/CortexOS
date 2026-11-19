//! Bounded report reads. No model requests, rescoring, snapshots or message bodies in overview/page queries.
use super::{repository::Detail, telemetry};
use crate::{
    cloud::CloudState,
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::AppResult,
};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::State;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportScope {
    Standard,
    Personalized,
}
impl ReportScope {
    fn standard(self) -> bool {
        matches!(self, Self::Standard)
    }
}
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultFilter {
    pub state: Option<String>,
    pub evaluator: Option<String>,
    pub difficulty: Option<String>,
    pub repetition: Option<u32>,
}
impl ResultFilter {
    fn validate(&self, repetitions: u32) -> AppResult<()> {
        let allowed = |value: &Option<String>, options: &[&str]| {
            value.as_ref().is_none_or(|v| options.contains(&v.as_str()))
        };
        if !allowed(
            &self.state,
            &[
                "Passed",
                "Failed",
                "Unsupported",
                "Not Run",
                "Execution Error",
                "Scoring Error",
                "Needs Review",
                "In Progress",
                "Cancelled",
            ],
        ) || !allowed(
            &self.evaluator,
            &[
                "exact_alias",
                "integer_equal",
                "schema_semantic",
                "evidence_match",
                "python_assertions",
                "native_tool_equal",
            ],
        ) || !allowed(&self.difficulty, &["easy", "medium", "hard"])
            || self.repetition.is_some_and(|n| n == 0 || n > repetitions)
        {
            return Err("筛选条件无效，请重新选择。".into());
        }
        Ok(())
    }
}
pub(super) struct ReportRead {
    pub status: String,
    pub evaluation: Option<Value>,
    pub attempted: bool,
    pub metrics: Value,
    pub logical: Option<Value>,
    pub resources: Value,
}
impl ReportRead {
    pub fn from_detail(detail: &Detail) -> Self {
        Self {
            status: detail.execution.run.status.clone(),
            evaluation: detail
                .evaluations
                .iter()
                .rev()
                .find(|e| e["details"].get("regrade").is_none())
                .cloned(),
            attempted: detail
                .execution
                .events
                .iter()
                .any(|e| e.kind == "model_request_started"),
            metrics: detail
                .execution
                .events
                .iter()
                .rev()
                .find(|e| {
                    matches!(
                        e.kind.as_str(),
                        "response_completed" | "run_cancelled" | "run_failed" | "run_interrupted"
                    )
                })
                .map(|e| e.payload["metrics"].clone())
                .unwrap_or(Value::Null),
            logical: detail
                .events
                .iter()
                .rev()
                .find(|e| e["kind"] == "case_timing_recorded")
                .map(|e| e["payload"].clone()),
            resources: telemetry::resource_summary(&detail.resources),
        }
    }
}
pub(super) fn read_item(c: &Connection, workspace: &str, id: &str) -> AppResult<ReportRead> {
    let (attempt,status,evaluation,attempted,metrics,logical):(String,String,Option<String>,bool,Option<String>,Option<String>)=c.query_row(
        "SELECT a.id,r.status,
          (SELECT json_object('metrics',json_object('performance',json_extract(e.metrics_json,'$.performance')),'details',json_object('state',json_extract(e.details_json,'$.state'),'attempted',json(CASE WHEN json_extract(e.details_json,'$.attempted')=1 THEN 'true' ELSE 'false' END),'executionSucceeded',json(CASE WHEN json_extract(e.details_json,'$.executionSucceeded')=1 THEN 'true' ELSE 'false' END))) FROM benchmark_evaluations e WHERE e.attempt_id=a.id AND json_type(e.details_json,'$.regrade') IS NULL ORDER BY e.created_at DESC,e.id DESC LIMIT 1),
          EXISTS(SELECT 1 FROM execution_events x WHERE x.run_id=r.id AND x.kind='model_request_started'),
          (SELECT json_extract(x.payload_json,'$.metrics') FROM execution_events x WHERE x.run_id=r.id AND x.kind IN ('response_completed','run_cancelled','run_failed','run_interrupted') ORDER BY x.seq DESC LIMIT 1),
          (SELECT x.payload_json FROM benchmark_events x WHERE x.run_id=b.id AND x.kind='case_timing_recorded' ORDER BY x.seq DESC LIMIT 1)
         FROM benchmark_runs b JOIN benchmark_case_runs cr ON cr.run_id=b.id JOIN benchmark_attempts a ON a.case_run_id=cr.id AND a.attempt_no=1 JOIN execution_runs r ON r.id=a.execution_run_id JOIN conversations v ON v.id=r.conversation_id AND v.purpose='benchmark'
         WHERE b.id=?1 AND b.workspace_id=?2 AND a.workspace_id=?2 AND r.workspace_id=?2",
        params![id,workspace],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(error)?;
    let parse = |s: Option<String>| {
        s.map(|s| serde_json::from_str(&s).map_err(json_error))
            .transpose()
    };
    Ok(ReportRead {
        status,
        evaluation: parse(evaluation)?,
        attempted,
        metrics: parse(metrics)?.unwrap_or(Value::Null),
        logical: parse(logical)?,
        resources: resource_totals(c, workspace, &attempt)?,
    })
}
fn resource_totals(c: &Connection, workspace: &str, attempt: &str) -> AppResult<Value> {
    let mut result=c.query_row("SELECT json_object('sampleCount',count(*),'systemCpuPercentPeak',max(json_extract(values_json,'$.systemCpuPercent')),'systemMemoryBytesPeak',max(json_extract(values_json,'$.systemMemory.usedBytes'))) FROM benchmark_resource_samples WHERE attempt_id=?1 AND workspace_id=?2",params![attempt,workspace],|r|r.get::<_,String>(0)).map_err(error).and_then(|s|serde_json::from_str::<Value>(&s).map_err(json_error))?;
    let mut gpus = vec![];
    if result["sampleCount"].as_u64().unwrap_or(0) > 0 {
        let mut q=c.prepare("SELECT json_object('uuid',json_extract(g.value,'$.uuid'),'usedBytesPeak',max(json_extract(g.value,'$.usedBytes')),'memorySamples',count(json_extract(g.value,'$.usedBytes'))) FROM benchmark_resource_samples s,json_each(s.values_json,'$.gpus') g WHERE s.attempt_id=?1 AND s.workspace_id=?2 AND json_type(s.values_json,'$.gpus')='array' AND json_extract(g.value,'$.uuid') IS NOT NULL GROUP BY json_extract(g.value,'$.uuid') ORDER BY json_extract(g.value,'$.uuid')").map_err(error)?;
        for row in q
            .query_map(params![attempt, workspace], |r| r.get::<_, String>(0))
            .map_err(error)?
        {
            gpus.push(serde_json::from_str::<Value>(&row.map_err(error)?).map_err(json_error)?);
        }
    }
    result["gpus"] = json!(gpus);
    Ok(result)
}

const RESULT_QUERY: &str = "WITH results AS (
          SELECT i.ordinal,i.case_id,i.status,i.run_id,substr(i.error,1,1000) AS error,
            substr(json_extract(bc.input_json,'$.prompt'),1,240)||CASE WHEN length(json_extract(bc.input_json,'$.prompt'))>240 THEN '…' ELSE '' END AS question,
            bc.difficulty,json_extract(bc.scoring_json,'$.id') AS evaluator,
            i.ordinal % max(1,json_array_length(b.plan_json,'$.cases')) / max(1,json_array_length(b.plan_json,'$.cases') / coalesce(json_extract(b.plan_json,'$.input.repetitions'),1)) + 1 AS repetition,
            coalesce((SELECT json_extract(e.details_json,'$.state') FROM benchmark_evaluations e WHERE e.attempt_id=a.id AND json_type(e.details_json,'$.regrade') IS NULL ORDER BY e.created_at DESC,e.id DESC LIMIT 1),
              CASE WHEN r.status='completed' THEN 'Needs Review' WHEN r.status='running' THEN 'In Progress' WHEN r.status IN ('cancelled','interrupted') THEN 'Cancelled' WHEN r.status IS NOT NULL THEN 'Execution Error' WHEN i.status='running' THEN 'In Progress' WHEN i.status='error' THEN 'Execution Error' WHEN i.status='cancelled' THEN 'Cancelled' ELSE 'Not Run' END) AS state
          FROM benchmark_batch_items i JOIN benchmark_batches b ON b.id=i.batch_id
          JOIN benchmark_cases bc ON bc.dataset_id=json_extract(b.plan_json,'$.input.datasetId') AND bc.dataset_version=json_extract(b.plan_json,'$.input.datasetVersion') AND bc.case_id=i.case_id
          LEFT JOIN benchmark_case_runs cr ON cr.run_id=i.run_id AND cr.workspace_id=b.workspace_id
          LEFT JOIN benchmark_attempts a ON a.case_run_id=cr.id AND a.attempt_no=1
          LEFT JOIN execution_runs r ON r.id=a.execution_run_id AND r.workspace_id=b.workspace_id
          WHERE b.workspace_id=?1 AND b.id=?2 AND i.model_index=?3
        ), matched AS (SELECT * FROM results WHERE (?4 IS NULL OR state=?4) AND (?5 IS NULL OR evaluator=?5) AND (?6 IS NULL OR difficulty=?6) AND (?7 IS NULL OR repetition=?7)) ";

impl Database {
    fn report_scope(&self, workspace: &str, id: &str, scope: ReportScope) -> AppResult<()> {
        let found:bool=self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_batches WHERE id=?1 AND workspace_id=?2 AND (json_extract(plan_json,'$.input.datasetId')='cortex-core-60')=?3)",params![id,workspace,scope.standard()],|r|r.get(0)).map_err(error)?;
        if found {
            Ok(())
        } else {
            Err("评测不存在或不属于此工作区和评测类型。".into())
        }
    }
    pub fn benchmark_overview(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
    ) -> AppResult<Value> {
        self.report_scope(workspace, id, scope)?;
        self.batch_overview(workspace, id)
    }
    #[cfg(test)]
    pub fn benchmark_result_page(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        model: u32,
        page: u32,
        size: u32,
    ) -> AppResult<Value> {
        self.benchmark_filtered_result_page(
            workspace,
            id,
            scope,
            model,
            page,
            size,
            &ResultFilter::default(),
        )
    }
    pub fn benchmark_filtered_result_page(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        model: u32,
        page: u32,
        size: u32,
        filter: &ResultFilter,
    ) -> AppResult<Value> {
        self.report_scope(workspace, id, scope)?;
        if !(1..=50).contains(&size) {
            return Err("每页数量须为 1–50。".into());
        }
        let c = self.connection()?;
        let (models, repetitions): (u32, u32) = c.query_row("SELECT json_array_length(plan_json,'$.input.models'), coalesce(json_extract(plan_json,'$.input.repetitions'),1) FROM benchmark_batches WHERE id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;
        if model >= models {
            return Err("模型序号不存在。".into());
        }
        filter.validate(repetitions)?;
        let planned: u32 = c
            .query_row(
                "SELECT count(*) FROM benchmark_batch_items WHERE batch_id=?1 AND model_index=?2",
                params![id, model],
                |r| r.get(0),
            )
            .map_err(error)?;
        // Both COUNT and page use the same database projection and bound conditions.
        let query = RESULT_QUERY;
        let total: u32 = c
            .query_row(
                &format!("{query} SELECT count(*) FROM matched"),
                params![
                    workspace,
                    id,
                    model,
                    filter.state,
                    filter.evaluator,
                    filter.difficulty,
                    filter.repetition
                ],
                |r| r.get(0),
            )
            .map_err(error)?;
        if page > 0 && u64::from(page) * u64::from(size) >= u64::from(total) {
            return Err("页码不存在，请返回第一页。".into());
        }
        let mut q=c.prepare(&format!("{query} SELECT ordinal,case_id,status,run_id,error,question,state,repetition,evaluator,difficulty FROM matched ORDER BY ordinal LIMIT ?8 OFFSET ?9")).map_err(error)?;
        let rows=q.query_map(params![workspace,id,model,filter.state,filter.evaluator,filter.difficulty,filter.repetition,size,i64::from(page)*i64::from(size)],|r|Ok(json!({"ordinal":r.get::<_,u32>(0)?,"caseId":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"runId":r.get::<_,Option<String>>(3)?,"error":r.get::<_,Option<String>>(4)?,"question":r.get::<_,String>(5)?,"state":r.get::<_,String>(6)?,"repetition":r.get::<_,u32>(7)?,"evaluator":r.get::<_,String>(8)?,"difficulty":r.get::<_,String>(9)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        Ok(
            json!({"batchId":id,"modelIndex":model,"page":page,"pageSize":size,"total":total,"plannedTotal":planned,"rows":rows}),
        )
    }
    pub fn benchmark_result_location(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        ordinal: Option<u32>,
        execution: Option<&str>,
        size: u32,
        filter: &ResultFilter,
    ) -> AppResult<Value> {
        self.report_scope(workspace, id, scope)?;
        if !(1..=50).contains(&size)
            || (ordinal.is_none() && execution.is_none())
            || execution.is_some_and(|s| s.is_empty() || s.len() > 128)
        {
            return Err("题目定位参数无效。".into());
        }
        let c = self.connection()?;
        let (found,model,repetitions):(u32,u32,u32)=c.query_row("SELECT i.ordinal,i.model_index,coalesce(json_extract(b.plan_json,'$.input.repetitions'),1) FROM benchmark_batch_items i JOIN benchmark_batches b ON b.id=i.batch_id LEFT JOIN benchmark_case_runs cr ON cr.run_id=i.run_id AND cr.workspace_id=b.workspace_id LEFT JOIN benchmark_attempts a ON a.case_run_id=cr.id AND a.attempt_no=1 WHERE b.workspace_id=?1 AND b.id=?2 AND (?3 IS NULL OR i.ordinal=?3) AND (?4 IS NULL OR a.execution_run_id=?4) ORDER BY i.ordinal LIMIT 1",params![workspace,id,ordinal,execution],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"题目或执行记录不属于当前工作区和评测，请检查定位链接。".to_string())?;
        filter.validate(repetitions)?;
        let (matched,before):(bool,u32)=c.query_row(&format!("{RESULT_QUERY} SELECT EXISTS(SELECT 1 FROM matched WHERE ordinal=?8),(SELECT count(*) FROM matched WHERE ordinal<?8)"),params![workspace,id,model,filter.state,filter.evaluator,filter.difficulty,filter.repetition,found],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;
        Ok(
            json!({"ordinal":found,"modelIndex":model,"matchesFilter":matched,"page":if matched{Some(before/size)}else{None}}),
        )
    }
    pub fn benchmark_result_detail(
        &self,
        workspace: &str,
        id: &str,
        scope: ReportScope,
        ordinal: u32,
    ) -> AppResult<Value> {
        self.report_scope(workspace, id, scope)?;
        let c = self.connection()?;
        let (run,case,dataset,version,item_error):(Option<String>,String,String,u32,Option<String>)=c.query_row("SELECT i.run_id,i.case_id,json_extract(b.plan_json,'$.input.datasetId'),json_extract(b.plan_json,'$.input.datasetVersion'),i.error FROM benchmark_batch_items i JOIN benchmark_batches b ON b.id=i.batch_id WHERE b.id=?1 AND b.workspace_id=?2 AND i.ordinal=?3",params![id,workspace,ordinal],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|_|"这道题不属于当前评测。".to_string())?;
        drop(c);
        if let Some(run) = run {
            let detail = self.benchmark_detail(workspace, &run)?;
            if detail.case.case_id != case
                || detail.config["datasetId"] != dataset
                || detail.config["datasetVersion"] != version
            {
                return Err("题目记录与冻结计划不一致。".into());
            }
            let read = ReportRead::from_detail(&detail);
            let mut result = serde_json::to_value(&detail).map_err(json_error)?;
            result["performance"] = telemetry::measure(
                &read.metrics,
                &read.status,
                read.logical.as_ref(),
                read.resources,
            );
            result["itemError"] = json!(item_error);
            Ok(result)
        } else {
            Ok(
                json!({"itemError":item_error,"case":self.benchmark_case(workspace,&dataset,version,&case)?,"execution":null,"output":null,"evaluations":[],"events":[],"resources":[],"hardware":null,"performance":null}),
            )
        }
    }
}
async fn access(
    state: AppState,
    cloud: CloudState,
    scope: ReportScope,
    read: impl FnOnce(&Database) -> AppResult<Value> + Send + 'static,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| match scope {
        ReportScope::Standard => read(db),
        ReportScope::Personalized => cloud.with_access(|| read(db)),
    })
    .await
}
#[tauri::command]
pub async fn get_benchmark_overview(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
) -> AppResult<Value> {
    access(
        state.inner().clone(),
        cloud.inner().clone(),
        scope,
        {
            let tasks = state.tasks.clone();
            move |db| {
                let mut report = db.benchmark_overview(&workspace_id, &batch_id, scope)?;
                let waiting = tasks.is_waiting(&batch_id, &workspace_id, crate::tasks::TaskKind::Benchmark)?;
                let starting = !waiting && matches!(report["status"].as_str(), Some("planned" | "cancelled" | "interrupted" | "failed")) && tasks.summary()?.is_some_and(|a| a.id == batch_id && a.workspace_id == workspace_id && a.kind == crate::tasks::TaskKind::Benchmark);
                report["starting"] = Value::Bool(starting);
                report["waitingForResources"] = Value::Bool(waiting);
                Ok(report)
            }
        },
    )
    .await
}
#[tauri::command]
pub async fn get_benchmark_result_page(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    model_index: u32,
    page: u32,
    page_size: u32,
    filter: Option<ResultFilter>,
) -> AppResult<Value> {
    access(
        state.inner().clone(),
        cloud.inner().clone(),
        scope,
        move |db| {
            db.benchmark_filtered_result_page(
                &workspace_id,
                &batch_id,
                scope,
                model_index,
                page,
                page_size,
                &filter.unwrap_or_default(),
            )
        },
    )
    .await
}
#[tauri::command]
pub async fn get_benchmark_result_detail(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    ordinal: u32,
) -> AppResult<Value> {
    access(
        state.inner().clone(),
        cloud.inner().clone(),
        scope,
        move |db| db.benchmark_result_detail(&workspace_id, &batch_id, scope, ordinal),
    )
    .await
}

#[tauri::command]
pub async fn get_benchmark_result_location(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    scope: ReportScope,
    ordinal: Option<u32>,
    execution_run_id: Option<String>,
    page_size: u32,
    filter: Option<ResultFilter>,
) -> AppResult<Value> {
    access(
        state.inner().clone(),
        cloud.inner().clone(),
        scope,
        move |db| {
            db.benchmark_result_location(
                &workspace_id,
                &batch_id,
                scope,
                ordinal,
                execution_run_id.as_deref(),
                page_size,
                &filter.unwrap_or_default(),
            )
        },
    )
    .await
}
