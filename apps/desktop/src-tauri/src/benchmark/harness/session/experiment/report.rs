use super::*;
use crate::benchmark::report_transfer::{Format, cell};
use chrono::DateTime;

fn duration_ms(start: &Value, end: &Value) -> Option<i64> {
    let start = DateTime::parse_from_rfc3339(start.as_str()?).ok()?;
    let end = DateTime::parse_from_rfc3339(end.as_str()?).ok()?;
    Some((end - start).num_milliseconds().max(0))
}

fn failure_category(status: &str, reason: &str, score: &Value) -> &'static str {
    if status == "queued" || status == "not_run" {
        return "not_run";
    }
    if matches!(status, "cancelled" | "interrupted" | "closed") {
        return "interruption";
    }
    if reason.starts_with("aider_edit_outside")
        || reason == "workspace_project_edit_outside_scope"
        || matches!(
            reason,
            "aider_signature_changed" | "container_limits_not_confirmed" | "tool_approval_required"
        )
    {
        return "scope_or_permission";
    }
    if reason.contains("timeout") || reason.contains("budget") || reason.contains("limit") {
        return "budget";
    }
    if reason == "model_identity_changed" {
        return "model_or_provider";
    }
    if status == "unsupported"
        || reason.starts_with("unsupported_")
        || score["state"] == "Unsupported"
    {
        return "unsupported";
    }
    if reason.contains("model_") {
        return "model_or_provider";
    }
    if reason.contains("docker")
        || reason.contains("container")
        || status == "cleanup_required"
        || score["state"] == "Execution Error"
    {
        return "environment";
    }
    if score["state"] == "Failed" {
        return "assertions";
    }
    if score["state"] == "Scoring Error" {
        return "scoring";
    }
    if status == "completed" && score["state"] == "Passed" {
        return "none";
    }
    if status == "failed" {
        return "execution";
    }
    "none"
}

fn row(item: &Value, record: Option<&Value>, experiment_status: &str) -> Value {
    let status =
        record
            .and_then(|r| r["status"].as_str())
            .unwrap_or(if experiment_status == "running" {
                "queued"
            } else {
                "not_run"
            });
    let events = record
        .and_then(|r| r["events"].as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let find = |kind: &str| {
        events
            .iter()
            .find(|e| e["kind"] == kind)
            .map(|e| &e["data"])
    };
    let score = find("score").map(|d| &d["score"]).unwrap_or(&Value::Null);
    let candidate = find("candidate").map(|d| &d["patch"]);
    let reason = record.and_then(|r| r["error"].as_str()).unwrap_or_default();
    let requests = events
        .iter()
        .filter(|e| e["kind"] == "model_requested")
        .count();
    let mut responses = 0;
    let (mut input, mut output) = (Some(0_u64), Some(0_u64));
    for event in events.iter().filter(|e| e["kind"] == "model_response") {
        let d = &event["data"];
        let usage = if d.get("usage").is_some() {
            (&d["usage"]["promptTokens"], &d["usage"]["outputTokens"])
        } else {
            (
                &d["response"]["prompt_eval_count"],
                &d["response"]["eval_count"],
            )
        };
        input = input
            .zip(usage.0.as_u64())
            .and_then(|(a, b)| a.checked_add(b));
        output = output
            .zip(usage.1.as_u64())
            .and_then(|(a, b)| a.checked_add(b));
        responses += 1;
    }
    if record.is_none() || responses != requests {
        input = None;
        output = None;
    }
    let changed = candidate.map(|p| {
        if p["format"] == "project-replacement-1" {
            return p["files"].as_array().is_some_and(|a| !a.is_empty());
        }
        p["before"]
            .as_str()
            .zip(p["after"].as_str())
            .is_some_and(|(before, after)| before.trim_end() != after.trim_end())
    });
    let duration = record.and_then(|r| duration_ms(&r["createdAt"], &r["finishedAt"]));
    let passed = status == "completed" && score["state"] == "Passed" && changed == Some(true);
    let build_status = if item["bundle"]["scope"] == "python_project" {
        score["details"]["buildStatus"]
            .as_str()
            .unwrap_or("not_run")
    } else {
        "not_applicable"
    };
    let native_denials = find("permission_observation")
        .and_then(|d| d["denials"].as_array())
        .map(Vec::len)
        .or_else(|| {
            events
                .iter()
                .find(|e| e["kind"] == "cli" && e["data"]["parsed"]["type"] == "result")
                .and_then(|e| e["data"]["parsed"]["permission_denials"].as_array())
                .map(Vec::len)
        });
    let denied = events
        .iter()
        .filter(|e| e["kind"] == "permission_decision" && e["data"]["decision"] == "denied")
        .count();
    let blocked = native_denials.unwrap_or(0)
        + denied.max(usize::from(
            failure_category(status, reason, score) == "scope_or_permission",
        ));
    json!({"id":item["owner"],"taskKey":item["taskKey"],"function":item["function"],"datasetId":item["datasetId"],"datasetVersion":item["datasetVersion"],"weight":item["weight"],"repetition":item["repetition"],"harness":item["harness"],"harnessVersion":item["harnessVersion"],"model":item["model"]["model"],"digest":item["model"]["expectedDigest"],"status":status,"error":record.map(|r|&r["error"]),"failureCategory":failure_category(status,reason,score),"scoreState":score["state"],"assertions":score["metrics"],"candidateChanged":changed,"passed":passed,"durationMs":duration,"requests":requests,"responses":responses,"inputTokens":input,"outputTokens":output,"apiCost":if requests==responses && record.is_some(){Some(0)}else{None},"buildStatus":build_status,"recorded":record.is_some(),"clean":record.map(|r|&r["clean"]),"pendingGraders":record.map(|r|&r["pendingGraders"]),"blockedActions":blocked,"nativePermissionDenials":native_denials,"approvalCount":Value::Null})
}

fn group_key(row: &Value) -> String {
    json!([
        row["harness"],
        row["harnessVersion"],
        row["model"],
        row["digest"]
    ])
    .to_string()
}

fn wilson(passed: usize, n: usize) -> Value {
    if n == 0 {
        return Value::Null;
    }
    let n = n as f64;
    let p = passed as f64 / n;
    let z = 1.959963984540054_f64;
    let d = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / d;
    let width = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    json!([(center - width).max(0.0), (center + width).min(1.0)])
}

fn sum(rows: &[&Value], key: &str) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(row[key].as_u64()?))
}

fn summarize(rows: &[Value]) -> Vec<Value> {
    let mut arms: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for row in rows {
        arms.entry(group_key(row)).or_default().push(row);
    }
    arms.into_values().map(|all| {
        let first=all[0];
        let done:Vec<_>=all.iter().copied().filter(|r|matches!(r["status"].as_str(),Some("completed"|"failed"|"unsupported"|"cancelled"|"interrupted"|"closed"|"cleanup_required"))).collect();
        let successful:Vec<_>=done.iter().copied().filter(|r|r["passed"]==true).collect();
        let mut failures=BTreeMap::new();
        for r in &done {
            if r["passed"]!=true {
                let key=r["failureCategory"].as_str().filter(|v|*v!="none").unwrap_or("not_scored");
                *failures.entry(key).or_insert(0)+=1;
            }
        }
        let metric_sum = |key: &str| all.iter().filter_map(|r|r["assertions"][key].as_u64()).sum::<u64>();
        let weighted=sum(&done,"weight").unwrap_or_default();
        let duration:Vec<_>=done.iter().filter_map(|r|r["durationMs"].as_u64()).collect();
        let builds_required = all.iter().filter(|r| r["buildStatus"] != "not_applicable").count();
        let builds_passed = all.iter().filter(|r| r["buildStatus"] == "passed").count();
        let builds_run = builds_passed + all.iter().filter(|r| r["buildStatus"] == "failed").count();
        json!({"harness":first["harness"],"harnessVersion":first["harnessVersion"],"model":first["model"],"digest":first["digest"],"planned":all.len(),"finished":done.len(),"passed":successful.len(),"weightedPassed":sum(&successful,"weight"),"weightedFinished":weighted,"weightedSuccess":(weighted>0).then(||sum(&successful,"weight").unwrap_or_default() as f64/weighted as f64),"successInterval95":wilson(successful.len(),done.len()),"assertionsPassed":metric_sum("assertionsPassed"),"assertionsRun":metric_sum("assertionsRun"),"assertionsPlannedForScoredTasks":metric_sum("assertionsPlanned"),"scoredTasks":all.iter().filter(|r|r["scoreState"].is_string()).count(),"patches":all.iter().filter(|r|r["candidateChanged"]==true).count(),"durationMeanMs":(!duration.is_empty()).then(||duration.iter().sum::<u64>() as f64/duration.len() as f64),"durationSamples":duration.len(),"inputTokens":(!done.is_empty()).then(||sum(&done,"inputTokens")).flatten(),"outputTokens":(!done.is_empty()).then(||sum(&done,"outputTokens")).flatten(),"apiCost":(!done.is_empty()).then(||sum(&done,"apiCost")).flatten(),"buildsRequired":builds_required,"buildsRun":builds_run,"buildsPassed":builds_passed,"approvalCount":Value::Null,"nativePermissionDenials":done.iter().filter_map(|r|r["nativePermissionDenials"].as_u64()).reduce(|a,b|a+b),"blockedActions":sum(&done,"blockedActions"),"failures":failures})
    }).collect()
}

impl Database {
    fn experiment_child(&self, w: &str, id: &str, item: &Value) -> AppResult<Option<Value>> {
        let run = item["owner"].as_str().ok_or("任务编号缺失。")?;
        let kind = KINDS
            .into_iter()
            .find(|k| item["purpose"] == k.purpose())
            .ok_or("执行器缺失。")?;
        let exists:bool=self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE id=?1 AND workspace_id=?2)",params![run,w],|r|r.get(0)).map_err(error)?;
        if !exists {
            return Ok(None);
        }
        let record = self.session_record(kind, w, run)?;
        let mut p = record["plan"].clone();
        p["createdAt"] = item["createdAt"].clone();
        if p != *item || p["experimentId"] != id {
            return Err("实验子任务与原计划不一致。".into());
        }
        Ok(Some(record))
    }

    pub(super) fn experiment_report(&self, w: &str, id: &str) -> AppResult<Value> {
        let (plan, digest, status, finished, problem) = self.experiment_plan(w, id)?;
        let assessment = plan["assessmentVersion"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("分析版本缺失。")?;
        let analysis = self.fit_read(w, Some(assessment))?;
        let items = plan["items"].as_array().ok_or("实验任务清单缺失。")?;
        let mut rows = Vec::with_capacity(items.len());
        for item in items {
            rows.push(row(
                item,
                self.experiment_child(w, id, item)?.as_ref(),
                &status,
            ));
        }
        let summary = summarize(&rows);
        let mut coverage: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for row in &rows {
            coverage
                .entry(group_key(row))
                .or_default()
                .insert(row["taskKey"].to_string());
        }
        let same_tasks = coverage
            .values()
            .next()
            .is_some_and(|first| coverage.values().all(|tasks| tasks == first));
        let complete = summary.iter().all(|s| s["planned"] == s["finished"]);
        let enough = coverage.len() >= 2
            && coverage.values().all(|keys| keys.len() >= 3)
            && summary
                .iter()
                .all(|s| s["finished"].as_u64().unwrap_or(0) >= 3);
        let docker_versions: BTreeSet<_> = items
            .iter()
            .map(|p| p["environment"]["dockerServer"].to_string())
            .collect();
        let grading_versions: BTreeSet<_> = items
            .iter()
            .map(|p| json!([p["gradingImageId"], p["evaluatorVersion"]]).to_string())
            .collect();
        let matched_environment = docker_versions.len() == 1 && grading_versions.len() == 1;
        let same_project_runner = items
            .iter()
            .filter_map(|p| p["projectRunner"].as_str())
            .collect::<BTreeSet<_>>()
            .len()
            <= 1;
        let comparable = same_tasks
            && complete
            && matched_environment
            && same_project_runner
            && status == "completed"
            && plan["evidence"] == "local_model";
        // A candidate is only reported when its unweighted interval separates it from all peers
        // and it also leads the user's weighted measure.
        let body = &analysis["selected"]["body"];
        let suggested = if comparable && enough {
            summary.iter().find(|a| {
                if body["report"]["results"].as_array().is_some_and(|results| {
                    results
                        .iter()
                        .any(|r| r["harnessId"] == a["harness"] && r["eligibility"] == "excluded")
                }) {
                    return false;
                }
                summary.iter().filter(|b| *b != *a).all(|b| {
                    a["successInterval95"][0]
                        .as_f64()
                        .zip(b["successInterval95"][1].as_f64())
                        .is_some_and(|(low, high)| low > high)
                        && a["weightedSuccess"]
                            .as_f64()
                            .zip(b["weightedSuccess"].as_f64())
                            .is_some_and(|(x, y)| x > y)
                })
            })
        } else {
            None
        };
        let static_fit = json!({"catalogVersion":body["catalog"]["version"],"analysisHash":analysis["selected"]["contentHash"],"kind":body["report"]["kind"],"results":body["report"]["results"]});
        let mut public_plan = plan.clone();
        if let Some(items) = public_plan["items"].as_array_mut() {
            for item in items {
                item.as_object_mut()
                    .ok_or("实验计划格式错误。")?
                    .remove("bundle");
            }
        }
        Ok(
            json!({"id":id,"planHash":digest,"status":status,"finishedAt":finished,"error":problem,"plan":public_plan,"rows":rows,"summary":summary,"staticFit":static_fit,"experimentApprovals":1,"comparison":{"sameTasks":same_tasks,"sameScoringEnvironment":matched_environment && same_project_runner,"complete":complete,"comparable":comparable,"suggested":suggested}}),
        )
    }

    pub(super) fn experiment_trace(&self, w: &str, id: &str, run: &str) -> AppResult<Value> {
        let (plan, _, _, _, _) = self.experiment_plan(w, id)?;
        let item = plan["items"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["owner"] == run))
            .ok_or("任务不属于此实验。")?;
        self.experiment_child(w, id, item)?
            .ok_or_else(|| "任务尚未运行。".into())
    }

    pub(super) fn experiment_export(
        &self,
        w: &str,
        id: &str,
        format: Format,
    ) -> AppResult<(Vec<u8>, &'static str)> {
        let report = self.experiment_report(w, id)?;
        if report["status"] == "running" {
            return Err("实验结束后即可导出固定结果。".into());
        }
        let rows = report["rows"].as_array().ok_or("实验结果缺失。")?;
        if matches!(format, Format::Json) {
            let traces = rows
                .iter()
                .filter(|r| r["recorded"] == true)
                .map(|r| self.experiment_trace(w, id, r["id"].as_str().ok_or("任务编号缺失。")?))
                .collect::<AppResult<Vec<_>>>()?;
            let (plan, _, _, _, _) = self.experiment_plan(w, id)?;
            let payload = json!({"report":report,"plan":plan,"traces":traces});
            let envelope = json!({"reportSchema":"cortexos.harness-experiment","schemaVersion":1,"contentHash":hash(&serde_json::to_vec(&payload).map_err(json_error)?),"payload":payload});
            return Ok((
                serde_json::to_vec_pretty(&envelope).map_err(json_error)?,
                "json",
            ));
        }
        let fields = [
            "id",
            "datasetId",
            "datasetVersion",
            "function",
            "harness",
            "harnessVersion",
            "model",
            "digest",
            "repetition",
            "weight",
            "status",
            "scoreState",
            "failureCategory",
            "error",
            "assertions",
            "candidateChanged",
            "passed",
            "durationMs",
            "inputTokens",
            "outputTokens",
            "apiCost",
            "approvalCount",
            "blockedActions",
            "buildStatus",
            "nativePermissionDenials",
        ];
        let mut lines = vec![format!(
            "experiment_id,plan_hash,evidence,{},hardware_json,configuration_json",
            fields.join(",")
        )];
        let configs = report["plan"]["items"].as_array().ok_or("实验配置缺失。")?;
        for (row, config) in rows.iter().zip(configs) {
            let fields = fields
                .iter()
                .map(|k| cell(&row[*k]))
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "{},{},{},{},{},{}",
                cell(&report["id"]),
                cell(&report["planHash"]),
                cell(&report["plan"]["evidence"]),
                fields,
                cell(&report["plan"]["hardware"]),
                cell(config)
            ));
        }
        Ok((
            format!("\u{feff}{}\r\n", lines.join("\r\n")).into_bytes(),
            "csv",
        ))
    }
}
