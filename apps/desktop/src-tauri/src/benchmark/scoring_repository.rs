//! Durable v2 grading and read-only statistics; legacy v1 is handled separately.
use super::{
    dataset,
    evaluation::{self, Score, State},
    evaluation_json, python,
    repository::{benchmark_event, summary},
};
use crate::{
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use tokio::sync::watch;

impl Database {
    pub(crate) fn benchmark_score_pending(&self, workspace: &str, id: &str) -> AppResult<bool> {
        let c = self.connection()?;
        let s = summary(&c, workspace, id)?;
        saved_output_pending(&c, &s)
    }
    pub(crate) fn benchmark_modern(&self, workspace: &str, id: &str) -> AppResult<bool> {
        self.connection()?.query_row("SELECT dataset_version=2 OR json_extract(config_snapshot_json,'$.mode')='personalized_single_case_v1' FROM benchmark_runs WHERE workspace_id=?1 AND id=?2",params![workspace,id],|r|r.get(0)).map_err(error)
    }
    pub(crate) fn finish_benchmark_v2(
        &self,
        workspace: &str,
        id: &str,
        cancel: Option<watch::Receiver<bool>>,
    ) -> AppResult<()> {
        if !self.benchmark_modern(workspace, id)? {
            return Err("评分版本不匹配。".into());
        }
        {
            let c = self.connection()?;
            let s = summary(&c, workspace, id)?;
            let state: String = c
                .query_row(
                    "SELECT status FROM benchmark_attempts WHERE id=?1",
                    [&s.attempt_id],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if state != "running" && !saved_output_pending(&c, &s)? {
                return Ok(());
            }
        }
        let score = self.calculate_saved_benchmark_score(workspace, id, None, cancel)?;
        self.save_benchmark_score(workspace, id, score)
    }

    pub(crate) fn calculate_saved_benchmark_score(
        &self,
        workspace: &str,
        id: &str,
        attempt: Option<&str>,
        cancel: Option<watch::Receiver<bool>>,
    ) -> AppResult<Score> {
        if !self.benchmark_modern(workspace, id)? {
            return Err("仅支持已实现的新版评分协议，历史评分保持原样。".into());
        }
        let (config, status, output, metrics, frames, unsupported) = {
            let c = self.connection()?;
            let mut s = summary(&c, workspace, id)?;
            if let Some(attempt) = attempt {
                let (case_run, execution): (String, String) = c.query_row("SELECT a.case_run_id,a.execution_run_id FROM benchmark_attempts a JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN execution_runs r ON r.id=a.execution_run_id WHERE a.workspace_id=?1 AND cr.run_id=?2 AND a.id=?3 AND r.workspace_id=?1 AND EXISTS(SELECT 1 FROM conversations cv WHERE cv.id=r.conversation_id AND cv.workspace_id=r.workspace_id AND cv.purpose='benchmark')",params![workspace,id,attempt],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"评分尝试不属于此工作区和运行。".to_string())?;
                s.case_run_id = case_run;
                s.attempt_id = attempt.into();
                s.execution_run_id = execution;
            }
            let config: String = c
                .query_row(
                    "SELECT config_snapshot_json FROM benchmark_runs WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .map_err(error)?;
            let (status,output):(String,String)=c.query_row("SELECT r.status,m.content FROM execution_runs r JOIN messages m ON m.id=r.assistant_message_id WHERE r.id=?1",params![s.execution_run_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;
            let mut stmt=c.prepare("SELECT kind,payload_json FROM execution_events WHERE run_id=?1 AND kind IN ('native_tool_calls_received','response_completed','model_inspection_completed') ORDER BY seq").map_err(error)?;
            let rows = stmt
                .query_map(params![s.execution_run_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(error)?;
            let mut metrics = json!({});
            let mut frames = Vec::new();
            let mut unsupported = false;
            for row in rows {
                let (kind, payload) = row.map_err(error)?;
                let payload: Value = serde_json::from_str(&payload).map_err(json_error)?;
                if kind == "response_completed" {
                    metrics = payload["metrics"].clone();
                } else if kind == "model_inspection_completed" {
                    unsupported = payload["issues"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|i| i["code"] == "capability_unsupported"));
                } else {
                    frames.push(
                        payload["calls"]
                            .as_array()
                            .ok_or("工具响应记录无效。")?
                            .clone(),
                    );
                }
            }
            (
                serde_json::from_str::<Value>(&config).map_err(json_error)?,
                status,
                output,
                metrics,
                frames,
                unsupported,
            )
        };
        if status == "running" {
            return Err("模型尚在运行，不能评分。".into());
        }
        let case = self.benchmark_case(
            workspace,
            config["datasetId"].as_str().unwrap_or("cortex-core-60"),
            config["datasetVersion"].as_u64().ok_or("Missing version")? as u32,
            config["input"]["caseId"].as_str().ok_or("缺少题目。")?,
        )?;
        let score = if status == "cancelled" || status == "interrupted" {
            Score::state(State::Cancelled, status)
        } else if status != "completed" && unsupported {
            Score::state(
                State::Unsupported,
                "capability_unsupported_at_final_inspection",
            )
        } else if status != "completed" {
            Score::state(State::ExecutionError, status)
        } else if metrics["finishReason"] == "length" {
            Score::state(State::NeedsReview, "output_truncated")
        } else if case.scoring.id == "python_assertions" {
            python::score(
                &case,
                &output,
                config["input"]["pythonImage"].as_str(),
                cancel.as_ref(),
            )
        } else {
            match evaluation_json::assemble_calls(&frames) {
                Ok(calls) => evaluation::evaluate(&case, &output, &calls),
                Err(reason) => Score::state(State::ExecutionError, reason),
            }
        };
        Ok(score)
    }

    pub(crate) fn save_benchmark_score(
        &self,
        workspace: &str,
        id: &str,
        mut score: Score,
    ) -> AppResult<()> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let s = summary(&tx, workspace, id)?;
        let (version,config):(u32,String)=tx.query_row("SELECT dataset_version,config_snapshot_json FROM benchmark_runs WHERE id=?1 AND workspace_id=?2",params![id,workspace],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;
        if version != 2
            && serde_json::from_str::<Value>(&config).map_err(json_error)?["mode"]
                != "personalized_single_case_v1"
        {
            return Err("不能使用新版评分改写历史运行。".into());
        }
        let (attempt,raw):(String,String)=tx.query_row("SELECT a.status,r.status FROM benchmark_attempts a JOIN execution_runs r ON r.id=a.execution_run_id WHERE a.id=?1",params![s.attempt_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;
        if attempt != "running" && !saved_output_pending(&tx, &s)? {
            return Ok(());
        }
        score.details["recoveredFromSavedOutput"] = json!(attempt != "running");
        if raw == "running" {
            return Err("输出尚未完成保存。".into());
        }
        if matches!(score.state, State::Passed | State::Failed) && raw != "completed" {
            return Err("未完成执行不能记为已评分。".into());
        }
        if matches!(score.state, State::InProgress | State::NotRun) {
            return Err("未运行或进行中不是评分终态。".into());
        }
        let config: Value = serde_json::from_str(&config).map_err(json_error)?;
        let text: String = tx
            .query_row(
                "SELECT content FROM messages WHERE model_run_id=?1 AND role='assistant'",
                params![s.execution_run_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        let mut frames = Vec::new();
        {
            let mut stmt=tx.prepare("SELECT payload_json FROM execution_events WHERE run_id=?1 AND kind='native_tool_calls_received' ORDER BY seq").map_err(error)?;
            let rows = stmt
                .query_map(params![s.execution_run_id], |r| r.get::<_, String>(0))
                .map_err(error)?;
            for row in rows {
                frames
                    .push(serde_json::from_str::<Value>(&row.map_err(error)?).map_err(json_error)?);
            }
        }
        let attempted:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM execution_events WHERE run_id=?1 AND kind='model_request_started')",params![s.execution_run_id],|r|r.get(0)).map_err(error)?;
        score.details["state"] = serde_json::to_value(score.state).map_err(json_error)?;
        score.details["datasetVersion"] = config["datasetVersion"].clone();
        score.details["caseHash"] = config["caseHash"].clone();
        score.details["attempted"] = json!(attempted);
        score.details["executionSucceeded"] = json!(raw == "completed");
        let performance:Option<String>=tx.query_row("SELECT json_extract(payload_json,'$.metrics') FROM execution_events WHERE run_id=?1 AND kind='response_completed' ORDER BY seq DESC LIMIT 1",params![s.execution_run_id],|r|r.get(0)).optional().map_err(error)?;
        score.metrics["performance"] = match performance {
            Some(p) => serde_json::from_str(&p).map_err(json_error)?,
            None => {
                json!({"evalCount":null,"promptEvalCount":null,"clientTiming":null,"availability":"unavailable"})
            }
        };
        let evaluation_status = match score.state {
            State::ScoringError => "failed",
            State::NeedsReview => "needs_review",
            _ => "completed",
        };
        let (attempt_status, run_status) = match score.state {
            State::Passed | State::Failed => ("completed", "completed"),
            State::Unsupported => ("unsupported", "completed"),
            State::Cancelled => ("cancelled", "cancelled"),
            State::NeedsReview => ("completed", "completed"),
            State::ScoringError if raw == "completed" => ("completed", "failed"),
            _ => ("failed", "failed"),
        };
        let scoring_hash=dataset::hash(serde_json::to_string(&json!({"caseScoring":config["caseScoring"],"rule":config["scoringRule"],"engine":evaluation::ENGINE_VERSION,"sandboxPolicy":config["sandboxPolicy"],"image":config["input"]["pythonImage"]})).map_err(json_error)?.as_bytes());
        let output_hash = dataset::hash(
            serde_json::to_string(&json!({"text":text,"nativeFrames":frames}))
                .map_err(json_error)?
                .as_bytes(),
        );
        tx.execute(
            "INSERT INTO benchmark_evaluations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                new_id(),
                workspace,
                s.attempt_id,
                config["scoringRule"]["id"]
                    .as_str()
                    .ok_or("缺少评分规则。")?,
                evaluation::ENGINE_VERSION,
                scoring_hash,
                output_hash,
                score.metrics.to_string(),
                score.details.to_string(),
                evaluation_status,
                now()
            ],
        )
        .map_err(error)?;
        tx.execute("UPDATE benchmark_attempts SET status=?1,finished_at=?2,error_kind=?3,request_disposition=CASE WHEN ?4 THEN 'finished' ELSE request_disposition END WHERE id=?5 AND status='running'",params![attempt_status,now(),if matches!(score.state,State::Passed|State::Failed){None}else{Some(serde_json::to_string(&score.state).map_err(json_error)?)},raw=="completed",s.attempt_id]).map_err(error)?;
        tx.execute(
            "UPDATE benchmark_case_runs SET status=?1 WHERE id=?2 AND status='running'",
            params![attempt_status, s.case_run_id],
        )
        .map_err(error)?;
        tx.execute(
            "UPDATE benchmark_runs SET status=?1,finished_at=?2 WHERE id=?3 AND status IN ('running','interrupted','failed')",
            params![run_status, now(), id],
        )
        .map_err(error)?;
        benchmark_event(
            &tx,
            id,
            workspace,
            &s.attempt_id,
            "evaluation_saved",
            json!({"engineVersion":evaluation::ENGINE_VERSION,"state":score.state,"automaticRetry":false}),
        )?;
        tx.commit().map_err(error)
    }

    pub fn benchmark_statistics(&self, workspace: &str, id: &str) -> AppResult<Value> {
        if !self.benchmark_modern(workspace, id)? {
            return Err("该统计接口只读取v2运行。".into());
        }
        let c = self.connection()?;
        let _ = summary(&c, workspace, id)?;
        let mut stmt=c.prepare("SELECT a.status,e.metrics_json,e.details_json,EXISTS(SELECT 1 FROM execution_events x WHERE x.run_id=a.execution_run_id AND x.kind='model_request_started'),r.status,c.case_id,json_extract(bc.scoring_json,'$.id'),bc.difficulty,EXISTS(SELECT 1 FROM benchmark_events be WHERE be.run_id=c.run_id AND be.kind='result_save_failed') FROM benchmark_case_runs c JOIN benchmark_cases bc ON bc.dataset_id=c.dataset_id AND bc.dataset_version=c.dataset_version AND bc.case_id=c.case_id JOIN benchmark_attempts a ON a.case_run_id=c.id AND a.attempt_no=1 LEFT JOIN execution_runs r ON r.id=a.execution_run_id LEFT JOIN benchmark_evaluations e ON e.id=(SELECT ee.id FROM benchmark_evaluations ee WHERE ee.attempt_id=a.id AND json_type(ee.details_json,'$.regrade') IS NULL ORDER BY ee.created_at DESC,ee.id DESC LIMIT 1) WHERE c.run_id=?1 AND c.workspace_id=?2").map_err(error)?;
        let rows = stmt
            .query_map(params![id, workspace], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, bool>(8)?,
                ))
            })
            .map_err(error)?;
        let mut items = Vec::new();
        let mut distinct = std::collections::BTreeSet::new();
        let mut by_rule: std::collections::BTreeMap<String, Vec<_>> =
            std::collections::BTreeMap::new();
        let mut by_difficulty: std::collections::BTreeMap<String, Vec<_>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (status, metrics, details, attempted, raw, case, rule, difficulty, save_failed) =
                row.map_err(error)?;
            let score = if let (Some(metrics), Some(details)) = (metrics, details) {
                let details: Value = serde_json::from_str(&details).map_err(json_error)?;
                Score {
                    state: serde_json::from_value(details["state"].clone()).map_err(json_error)?,
                    metrics: serde_json::from_str(&metrics).map_err(json_error)?,
                    details,
                }
            } else {
                Score::state(
                    if save_failed {
                        State::ScoringError
                    } else if status == "running" {
                        State::InProgress
                    } else if raw.as_deref() == Some("completed") {
                        State::NeedsReview
                    } else if ["failed", "blocked"].contains(&status.as_str()) {
                        State::ExecutionError
                    } else {
                        State::NotRun
                    },
                    status,
                )
            };
            if matches!(score.state, State::Passed | State::Failed) {
                distinct.insert(case);
            }
            let item = (score, attempted, raw.as_deref() == Some("completed"));
            by_rule.entry(rule).or_default().push(item.clone());
            by_difficulty
                .entry(difficulty)
                .or_default()
                .push(item.clone());
            items.push(item);
        }
        let mut stats = serde_json::to_value(evaluation::statistics(&items)).map_err(json_error)?;
        let count:u32=c.query_row("SELECT count(*) FROM benchmark_cases bc JOIN benchmark_runs br ON br.dataset_id=bc.dataset_id AND br.dataset_version=bc.dataset_version WHERE br.id=?1 AND br.workspace_id=?2",params![id,workspace],|r|r.get(0)).map_err(error)?;
        stats["datasetCases"] = json!(count);
        stats["distinctCasesScored"] = json!(distinct.len());
        stats["datasetScoringCoverage"] = json!(distinct.len() as f64 / count.max(1) as f64);
        stats["engineVersion"] = json!(evaluation::ENGINE_VERSION);
        stats["level"] = json!(if count == 60 { "smoke" } else { "personalized" });
        for rule in [
            "exact_alias",
            "integer_equal",
            "schema_semantic",
            "evidence_match",
            "native_tool_equal",
            "python_assertions",
        ] {
            stats["byEvaluator"][rule] = serde_json::to_value(evaluation::statistics(
                by_rule.get(rule).map_or(&[], |v| v.as_slice()),
            ))
            .map_err(json_error)?;
        }
        for difficulty in ["easy", "medium", "hard"] {
            stats["byDifficulty"][difficulty] = serde_json::to_value(evaluation::statistics(
                by_difficulty.get(difficulty).map_or(&[], |v| v.as_slice()),
            ))
            .map_err(json_error)?;
        }
        Ok(stats)
    }
}

fn saved_output_pending(
    c: &rusqlite::Connection,
    s: &super::repository::Summary,
) -> AppResult<bool> {
    c.query_row("SELECT a.status='completed' AND r.status='completed' AND b.evaluator_version=?2 AND NOT EXISTS(SELECT 1 FROM benchmark_evaluations e WHERE e.attempt_id=a.id) FROM benchmark_attempts a JOIN execution_runs r ON r.id=a.execution_run_id JOIN benchmark_case_runs cr ON cr.id=a.case_run_id JOIN benchmark_runs b ON b.id=cr.run_id WHERE a.id=?1",params![s.attempt_id,evaluation::ENGINE_VERSION],|r|r.get(0)).map_err(error)
}

#[cfg(test)]
#[path = "scoring_repository_tests.rs"]
mod tests;
