use crate::{
    benchmark::evaluation::{self, Score, State},
    cloud::CloudState,
    commands::AppState,
    db::{Database, json_error},
    models::AppResult,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tauri::State as TauriState;
impl Database {
    pub fn personalized_report(&self, workspace: &str, id: &str) -> AppResult<Value> {
        self.require_batch_scope(workspace, id, false)?;
        self.batch_report(workspace, id, false)
    }
    pub fn standard_report(&self, workspace: &str, id: &str) -> AppResult<Value> {
        self.require_batch_scope(workspace, id, true)?;
        self.batch_report(workspace, id, false)
    }
    pub(crate) fn batch_overview(&self, workspace: &str, id: &str) -> AppResult<Value> {
        self.batch_report(workspace, id, true)
    }
    fn batch_report(&self, workspace: &str, id: &str, compact: bool) -> AppResult<Value> {
        let mut batch = self.personalized_batch(workspace, id)?;
        let models = batch["plan"]["input"]["models"]
            .as_array()
            .ok_or("模型配置缺失。")?;
        let mut reports = vec![];
        let dataset_id = batch["plan"]["input"]["datasetId"]
            .as_str()
            .ok_or("测试集缺失。")?;
        let version = batch["plan"]["input"]["datasetVersion"]
            .as_u64()
            .ok_or("版本缺失。")? as u32;
        let cases = self.benchmark_dataset_cases(workspace, dataset_id, version)?;
        let standard = dataset_id == "cortex-core-60";
        let mut can_resume = batch["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["status"] == "queued");
        for (index, model) in models.iter().enumerate() {
            let mut results = vec![];
            let mut stats_items = vec![];
            let mut categories: BTreeMap<String, Vec<_>> = BTreeMap::new();
            let mut difficulties: BTreeMap<String, Vec<_>> = BTreeMap::new();
            let mut measures = vec![];
            let mut repeats: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
            let mut cost = 0.;
            let mut unknown_cost = 0;
            for item in batch["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|i| i["modelIndex"] == index)
            {
                let mut result = item.clone();
                let mut attempted = false;
                let mut succeeded = false;
                let mut score = Score::state(
                    match item["status"].as_str() {
                        Some("running") => State::InProgress,
                        Some("error") => State::ExecutionError,
                        Some("cancelled") => State::Cancelled,
                        _ => State::NotRun,
                    },
                    "not_scored",
                );
                let case = cases
                    .iter()
                    .find(|c| item["caseId"] == c.case_id)
                    .ok_or("题目未找到。")?;
                let mut category = case.scoring.id.clone();
                let mut difficulty = case.difficulty.clone();
                if !compact {
                    result["question"] = json!(case.input.prompt);
                }
                if let Some(run) = item["runId"].as_str() {
                    let (read, detail) = if compact {
                        (
                            {
                                let reader = self.connection()?;
                                crate::benchmark::report_read::read_item(&reader, workspace, run)?
                            },
                            None,
                        )
                    } else {
                        let detail = self.benchmark_detail(workspace, run)?;
                        (
                            crate::benchmark::report_read::ReportRead::from_detail(&detail),
                            Some(detail),
                        )
                    };
                    if let Some(last) = &read.evaluation {
                        score = Score {
                            state: serde_json::from_value(last["details"]["state"].clone())
                                .map_err(json_error)?,
                            metrics: last["metrics"].clone(),
                            details: last["details"].clone(),
                        };
                        attempted = last["details"]["attempted"] == true;
                        succeeded = last["details"]["executionSucceeded"] == true;
                    } else {
                        score = Score::state(
                            match read.status.as_str() {
                                "completed" => State::NeedsReview,
                                "running" => State::InProgress,
                                "cancelled" | "interrupted" => State::Cancelled,
                                _ => State::ExecutionError,
                            },
                            "output_not_scored",
                        );
                        attempted = read.attempted;
                        succeeded = read.status == "completed";
                    }
                    if item["status"] != "finished"
                        && read.status == "completed"
                        && (read.evaluation.is_some()
                            || self.benchmark_score_pending(workspace, run)?)
                    {
                        can_resume = true;
                        result["recoverable"] = json!(true);
                    }
                    let perf = if read.metrics.is_null() {
                        &score.metrics["performance"]
                    } else {
                        &read.metrics
                    };
                    let it = perf["promptEvalCount"].as_u64();
                    let ot = perf["evalCount"].as_u64();
                    let measured = super::super::telemetry::measure(
                        perf,
                        &read.status,
                        read.logical.as_ref(),
                        read.resources,
                    );
                    measures.push(measured.clone());
                    repeats
                        .entry(item["repetition"].as_u64().unwrap_or(1))
                        .or_default()
                        .push(measured.clone());
                    if !compact {
                        result["performance"] = measured;
                    }
                    if model["kind"] == "cloud" && attempted {
                        if let (Some(i), Some(o), Some(price)) =
                            (it, ot, batch["plan"]["input"].get("price"))
                        {
                            cost += (i as f64 * price["inputPerMillion"].as_f64().unwrap_or(0.)
                                + o as f64 * price["outputPerMillion"].as_f64().unwrap_or(0.))
                                / 1_000_000.;
                        } else {
                            unknown_cost += 1;
                        }
                    }
                    if let Some(detail) = detail {
                        category = detail.case.scoring.id.clone();
                        difficulty = detail.case.difficulty.clone();
                        result["hardware"] = detail.hardware.clone();
                        result["question"] = json!(detail.case.input.prompt);
                        result["output"] = json!(detail.output);
                        result["executionRunId"] = json!(detail.summary.execution_run_id);
                        result["evaluations"] = json!(detail.evaluations);
                        result["events"] = json!(detail.events);
                    }
                }
                result["score"] = serde_json::to_value(&score).map_err(json_error)?;
                let tuple = (score, attempted, succeeded);
                stats_items.push(tuple.clone());
                categories.entry(category).or_default().push(tuple.clone());
                difficulties.entry(difficulty).or_default().push(tuple);
                results.push(result);
            }
            let category_stats: BTreeMap<_, _> = categories
                .into_iter()
                .map(|(k, v)| (k, evaluation::statistics(&v)))
                .collect();
            let difficulty_stats: BTreeMap<_, _> = difficulties
                .into_iter()
                .map(|(k, v)| (k, evaluation::statistics(&v)))
                .collect();
            let mut performance = super::super::telemetry::summarize(&measures, stats_items.len());
            performance["costEstimate"] = json!(if model["kind"] == "cloud" && unknown_cost == 0 {
                Some(cost)
            } else {
                None
            });
            performance["unknownCostRequests"] = json!(unknown_cost);
            performance["currency"] = batch["plan"]["input"]["price"]["currency"].clone();
            performance["billing"] = json!("estimate_not_invoice");
            let per_repeat = batch["plan"]["cases"]
                .as_array()
                .map(|c| c.len())
                .unwrap_or(stats_items.len())
                / batch["plan"]["input"]["repetitions"].as_u64().unwrap_or(1) as usize;
            let repeat_count = batch["plan"]["input"]["repetitions"].as_u64().unwrap_or(1);
            performance["byRepetition"] = json!((1..=repeat_count).map(|r|json!({"repetition":r,"performance":super::super::telemetry::summarize(repeats.get(&r).map(Vec::as_slice).unwrap_or(&[]),per_repeat)})).collect::<Vec<_>>());
            reports.push(json!({"modelIndex":index,"name":if model["kind"]=="local"{model["local"]["model"].clone()}else{batch["plan"]["binding"]["model"].clone()},"kind":model["kind"],"statistics":evaluation::statistics(&stats_items),"byEvaluator":category_stats,"byDifficulty":difficulty_stats,"performance":performance,"results":results}));
        }
        batch["reports"] = json!(reports);
        batch["canResume"] = json!(
            can_resume
                && !matches!(
                    batch["status"].as_str(),
                    Some("running" | "stopping" | "completed")
                )
        );
        batch["remainingRequests"] = json!(
            batch["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|i| i["status"] == "queued")
                .count()
        );
        batch["datasetCases"] = json!(cases.len());
        batch["selectedCases"] = json!(
            batch["plan"]["cases"]
                .as_array()
                .ok_or("计划题目缺失。")?
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        batch["scope"] = json!(if standard { "standard" } else { "personalized" });
        batch["comparisonPolicy"] = json!({"datasetId":batch["plan"]["input"]["datasetId"],"datasetVersion":batch["plan"]["input"]["datasetVersion"],"datasetHash":batch["plan"]["input"]["datasetHash"],"weights":batch["plan"]["profile"]["profile"]["weights"],"scope":"same_fixed_questions_only","recommendation":"仅比较本批次同题表现；请结合覆盖率、失败案例和偏好选择。缺失费用或显存不计作零成本。未生成综合分。"});
        let complete = reports
            .iter()
            .all(|r| r["statistics"]["scoringCoverage"] == 1.0);
        let mut evidence = vec![];
        if complete && !reports.is_empty() && !standard {
            let best = reports
                .iter()
                .filter_map(|r| r["statistics"]["correctness"].as_f64())
                .fold(0., f64::max);
            for r in reports
                .iter()
                .filter(|r| r["statistics"]["correctness"].as_f64() == Some(best))
            {
                evidence.push(json!({"model":r["name"],"finding":"highest_correctness_on_this_complete_fixed_set","correctness":best,"passed":r["statistics"]["passed"],"scored":r["statistics"]["scored"],"evidenceRunIds":r["results"].as_array().unwrap().iter().filter_map(|v|v["runId"].as_str()).collect::<Vec<_>>(),"limit":"This dataset only; ties remain ties. Weights are preferences, not a hidden composite score."}));
            }
        }
        batch["recommendationEvidence"] = json!(evidence);
        if compact {
            batch["pagedResults"] = json!(true);
            batch.as_object_mut().unwrap().remove("items");
            for report in batch["reports"].as_array_mut().unwrap() {
                report["results"] = json!([]);
            }
        }
        Ok(batch)
    }
}
#[tauri::command]
pub async fn export_personalized_report(
    app: tauri::AppHandle,
    state: TauriState<'_, AppState>,
    cloud: TauriState<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
) -> AppResult<bool> {
    crate::benchmark::report_transfer::export_benchmark_report(
        app,
        state,
        cloud,
        workspace_id,
        batch_id,
        crate::benchmark::report_read::ReportScope::Personalized,
        crate::benchmark::report_transfer::Format::Json,
    )
    .await
}
