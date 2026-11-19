use super::{
    cases::{self, Candidate},
    plan::{self, GenerationInput, GenerationPlan},
};
use crate::{
    benchmark::dataset::{self, Case, DatasetSummary},
    cloud::{CloudState, completion},
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
    tasks::TaskKind,
};
use rusqlite::params;
use serde_json::{Value, json};
use tauri::State;

impl Database {
    pub fn plan_generation(
        &self,
        workspace: &str,
        connection: &crate::cloud::Connection,
        input: GenerationInput,
    ) -> AppResult<Value> {
        let profile = self
            .benchmark_profile(workspace)?
            .ok_or("请先保存或跳过使用偏好。")?;
        if profile.version != input.profile_version || profile.action == "reset" {
            return Err("使用偏好已变化，请重新读取。".into());
        }
        let repo = input
            .repository_id
            .as_ref()
            .map(|id| self.repository_snapshot(workspace, id))
            .transpose()?;
        let plan = plan::build(
            connection,
            input,
            serde_json::to_value(profile).map_err(json_error)?,
            repo.as_ref(),
        )?;
        let raw = serde_json::to_value(&plan).map_err(json_error)?.to_string();
        let hash = dataset::hash(raw.as_bytes());
        let id = new_id();
        self.connection()?.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'planned',?3,?4,?5)",params![id,workspace,raw,hash,now()]).map_err(error)?;
        self.generation_job(workspace, &id)
    }
    pub fn generation_job(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        let mut job=c.query_row("SELECT id,status,plan_json,plan_hash,response_json,usage_json,error,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND COALESCE(json_extract(plan_json,'$.purpose'),'personalized')='personalized'",params![workspace,id],|r|{let parse=|i|->rusqlite::Result<Value>{let raw:Option<String>=r.get(i)?;raw.map(|s|serde_json::from_str(&s).map_err(|_|rusqlite::Error::InvalidQuery)).transpose().map(|v|v.unwrap_or(Value::Null))};Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"plan":parse(2)?,"planHash":r.get::<_,String>(3)?,"response":parse(4)?,"usage":parse(5)?,"error":r.get::<_,Option<String>>(6)?,"createdAt":r.get::<_,String>(7)?}))}).map_err(|_|"生成记录不存在或不属于此工作区。".to_string())?;
        let mut q=c.prepare("SELECT g.case_id,g.revision,g.candidate_json,g.validation_json FROM benchmark_generated_cases g WHERE g.job_id=?1 AND g.revision=(SELECT MAX(h.revision) FROM benchmark_generated_cases h WHERE h.job_id=g.job_id AND h.case_id=g.case_id) ORDER BY g.case_id").map_err(error)?;
        let items=q.query_map([id],|r|{let a:String=r.get(2)?;let b:String=r.get(3)?;Ok(json!({"id":r.get::<_,String>(0)?,"revision":r.get::<_,u32>(1)?,"candidate":serde_json::from_str::<Value>(&a).map_err(|_|rusqlite::Error::InvalidQuery)?,"validation":serde_json::from_str::<Value>(&b).map_err(|_|rusqlite::Error::InvalidQuery)?}))}).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        job["candidates"] = json!(items);
        Ok(job)
    }
    pub fn generation_history(&self, workspace: &str) -> AppResult<Vec<Value>> {
        let c = self.connection()?;
        dataset::require_workspace(&c, workspace)?;
        let mut q=c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND COALESCE(json_extract(plan_json,'$.purpose'),'personalized')='personalized' ORDER BY created_at DESC LIMIT 50").map_err(error)?;
        q.query_map([workspace],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)
    }
    pub fn begin_generation(
        &self,
        workspace: &str,
        id: &str,
        hash: &str,
        connection: &crate::cloud::Connection,
    ) -> AppResult<GenerationPlan> {
        let job = self.generation_job(workspace, id)?;
        let plan: GenerationPlan =
            serde_json::from_value(job["plan"].clone()).map_err(json_error)?;
        if job["planHash"] != hash
            || dataset::hash(job["plan"].to_string().as_bytes()) != hash
            || !plan.binding.matches(connection)
        {
            return Err("发送计划或云端连接已变化，请重新确认。".into());
        }
        let profile = self
            .benchmark_profile(workspace)?
            .ok_or("使用偏好不存在。")?;
        if profile.version != plan.input.profile_version {
            return Err("使用偏好已更新，请建立新计划。".into());
        }
        if self.connection()?.execute("UPDATE benchmark_generation_jobs SET status='sending' WHERE workspace_id=?1 AND id=?2 AND status='planned'",params![workspace,id]).map_err(error)?!=1{return Err("该计划已发送或已终止，不会重复计费；请查看原记录。".into());}
        Ok(plan)
    }
    pub fn finish_generation(
        &self,
        workspace: &str,
        id: &str,
        result: Result<completion::CompletionOutput, String>,
    ) -> AppResult<()> {
        let job = self.generation_job(workspace, id)?;
        let plan: GenerationPlan =
            serde_json::from_value(job["plan"].clone()).map_err(json_error)?;
        let repo = plan
            .input
            .repository_id
            .as_ref()
            .map(|id| self.repository_snapshot(workspace, id))
            .transpose()?;
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        if job["status"] != "sending" {
            return Err("生成任务已结束。".into());
        }
        match result {
            Err(reason) => {
                tx.execute("UPDATE benchmark_generation_jobs SET status=?1,error=?2,usage_json=?3,finished_at=?4 WHERE workspace_id=?5 AND id=?6",params![if reason=="用户已停止"{"cancelled"}else{"failed"},reason,json!({"availability":"unknown","reservedCost":plan.estimated_cost,"automaticRetry":false}).to_string(),now(),workspace,id]).map_err(error)?;
            }
            Ok(output) => {
                let (input, output_tokens) =
                    completion::tokens(plan.binding.provider, &output.usage);
                let usage = json!({"raw":output.usage,"inputTokens":input,"outputTokens":output_tokens,"elapsedMs":output.elapsed_ms,"requestHash":output.request_hash,"costEstimate":input.zip(output_tokens).map(|(i,o)|(i as f64*plan.input.price.input_per_million+o as f64*plan.input.price.output_per_million)/1_000_000.),"currency":plan.input.price.currency,"priceSource":plan.input.price.reference,"priceRecordedAt":job["createdAt"],"billing":"estimate_only_cache_reasoning_and_provider_rates_may_differ","reservedCost":plan.estimated_cost});
                let parsed = crate::benchmark::evaluation_json::decode(&output.text);
                let candidates = parsed
                    .as_ref()
                    .ok()
                    .and_then(|v| v["candidates"].as_array());
                let mut status = "invalid";
                let mut message = None;
                if output.finish_reason == "length" {
                    message = Some("输出达到长度限制，未发布候选；可调整设置创建新计划。");
                } else if let Some(items) =
                    candidates.filter(|a| a.len() == plan.input.count as usize && a.len() <= 12)
                {
                    let mut fingerprints = std::collections::BTreeSet::new();
                    let distribution_matches = plan.input.difficulty != "mixed"
                        || plan.input.difficulty_mix.iter().all(|(level, count)| {
                            items
                                .iter()
                                .filter(|v| v["case"]["difficulty"] == *level)
                                .count()
                                == *count as usize
                        });
                    for item in items {
                        let raw: Candidate = match serde_json::from_value(item.clone()) {
                            Ok(c) => c,
                            Err(_) => {
                                message =
                                    Some("候选结构不符合约定，完整响应已保留，请检查后重新生成。");
                                continue;
                            }
                        };
                        let candidate = cases::assigned(raw)?;
                        let mut validation =
                            cases::validate(&candidate, repo.as_ref(), &plan.input.snippets);
                        if !distribution_matches {
                            validation["valid"] = json!(false);
                            validation["errors"]
                                .as_array_mut()
                                .unwrap()
                                .push(json!("生成的难度分布与计划不符。"));
                        }
                        let fingerprint =
                            dataset::hash(candidate.case["input"].to_string().as_bytes());
                        if !fingerprints.insert(fingerprint) {
                            validation["valid"] = json!(false);
                            validation["errors"]
                                .as_array_mut()
                                .unwrap()
                                .push(json!("本次生成包含重复题目。"));
                        }
                        if !plan
                            .input
                            .categories
                            .iter()
                            .any(|s| candidate.case["scoring"]["id"] == *s)
                            || (plan.input.difficulty != "mixed"
                                && candidate.case["difficulty"] != plan.input.difficulty)
                        {
                            validation["valid"] = json!(false);
                            validation["errors"]
                                .as_array_mut()
                                .unwrap()
                                .push(json!("类别或难度不符合发送计划。"));
                        }
                        tx.execute(
                            "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
                            params![
                                id,
                                candidate.case["case_id"].as_str().ok_or("题目编号缺失。")?,
                                serde_json::to_string(&candidate).map_err(json_error)?,
                                validation.to_string(),
                                now()
                            ],
                        )
                        .map_err(error)?;
                    }
                    status = "review";
                } else {
                    message = Some("响应不是有效 JSON，或题目数量不符合请求。原始响应已保留。");
                }
                tx.execute("UPDATE benchmark_generation_jobs SET status=?1,response_json=?2,usage_json=?3,error=?4,finished_at=?5 WHERE workspace_id=?6 AND id=?7",params![status,serde_json::to_string(&output).map_err(json_error)?,usage.to_string(),message,now(),workspace,id]).map_err(error)?;
            }
        }
        tx.commit().map_err(error)
    }
    pub fn revise_candidate(
        &self,
        workspace: &str,
        job_id: &str,
        case_id: &str,
        expected_revision: u32,
        mut candidate: Candidate,
    ) -> AppResult<Value> {
        let job = self.generation_job(workspace, job_id)?;
        let plan: GenerationPlan =
            serde_json::from_value(job["plan"].clone()).map_err(json_error)?;
        let old = job["candidates"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == case_id))
            .ok_or("候选不存在。")?;
        if old["revision"] != expected_revision || job["status"] != "review" {
            return Err("候选已更新或尚未生成完毕，请重新读取。".into());
        }
        let mut case: Case =
            serde_json::from_value(candidate.case).map_err(|_| "请保留完整题目结构。")?;
        case.case_id = case_id.into();
        case.dataset_version = 1;
        cases::seal(&mut case)?;
        candidate.case = serde_json::to_value(case).map_err(json_error)?;
        let repo = plan
            .input
            .repository_id
            .as_ref()
            .map(|id| self.repository_snapshot(workspace, id))
            .transpose()?;
        let validation = cases::validate(&candidate, repo.as_ref(), &plan.input.snippets);
        self.connection()?
            .execute(
                "INSERT INTO benchmark_generated_cases VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    job_id,
                    case_id,
                    expected_revision + 1,
                    serde_json::to_string(&candidate).map_err(json_error)?,
                    validation.to_string(),
                    now()
                ],
            )
            .map_err(error)?;
        self.generation_job(workspace, job_id)
    }
    pub fn publish_personalized(
        &self,
        workspace: &str,
        job_id: &str,
        selection: Value,
        reviewer: &str,
    ) -> AppResult<DatasetSummary> {
        let job = self.generation_job(workspace, job_id)?;
        if job["status"] != "review" || reviewer.trim().is_empty() || reviewer.len() > 100 {
            return Err("请填写审核人，并完成候选审核。".into());
        }
        let selected = selection
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 12)
            .ok_or("请选择已核对题意、答案与证据的题目。")?;
        let id = format!("personal-{job_id}");
        let plan: GenerationPlan =
            serde_json::from_value(job["plan"].clone()).map_err(json_error)?;
        let repo = plan
            .input
            .repository_id
            .as_ref()
            .map(|id| self.repository_snapshot(workspace, id))
            .transpose()?;
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let version: u32 = tx
            .query_row(
                "SELECT COALESCE(MAX(version),0)+1 FROM benchmark_datasets WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(error)?;
        let mut cases = vec![];
        let mut unique = std::collections::BTreeSet::new();
        let mut prompts = std::collections::BTreeSet::new();
        for s in selected {
            let current:u32=tx.query_row("SELECT MAX(revision) FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?2",params![job_id,s["id"].as_str().ok_or("题目编号无效。")?],|r|r.get(0)).map_err(error)?;
            if Some(current as u64) != s["revision"].as_u64() {
                return Err("题目已修订，请重新审核。".into());
            }
            let old = job["candidates"]
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .find(|v| v["id"] == s["id"] && v["revision"] == s["revision"])
                })
                .ok_or("题目已修改，请重新审核当前版本。")?;
            let candidate: Candidate =
                serde_json::from_value(old["candidate"].clone()).map_err(json_error)?;
            let check = cases::validate(&candidate, repo.as_ref(), &plan.input.snippets);
            if check["valid"] != true
                || check["supported"] != true
                || s["approved"] != true
                || !unique.insert(s["id"].to_string())
                || !prompts.insert(candidate.case["input"].to_string())
            {
                return Err("只能发布无重复、校验通过且已人工批准的题目。".into());
            }
            let mut case: Case = serde_json::from_value(candidate.case).map_err(json_error)?;
            case.dataset_version = version;
            cases::seal(&mut case)?;
            cases.push(case);
        }
        let review = new_id();
        let hash = dataset::hash(
            serde_json::to_string(&cases)
                .map_err(json_error)?
                .as_bytes(),
        );
        let manifest = json!({"name":"我的专属测试集","level":"personalized","version":version,"case_count":cases.len(),"review_status":"human_approved","content_hash":hash,"prompt_template":{"system":"Complete the given benchmark task using only its supplied context. Follow its requested output format. Do not call tools except the explicitly supplied function definitions.","version":1},"provenance":{"workspaceId":workspace,"generationJobId":job_id,"generationPlanHash":job["planHash"],"binding":job["plan"]["binding"],"profile":job["plan"]["profile"],"repositoryHash":job["plan"]["repositoryHash"],"reviewId":review},"evaluatorVersion":crate::benchmark::evaluation::ENGINE_VERSION});
        tx.execute(
            "INSERT INTO benchmark_datasets VALUES(?1,?2,'我的专属测试集',2,?3,?4,?5,?6)",
            params![
                id,
                version,
                hash,
                manifest.to_string(),
                manifest["provenance"].to_string(),
                now()
            ],
        )
        .map_err(error)?;
        for case in cases {
            tx.execute(
                "INSERT INTO benchmark_cases VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    id,
                    version,
                    case.case_id,
                    case.category,
                    serde_json::to_string(&case.input).map_err(json_error)?,
                    serde_json::to_string(&case.expected).map_err(json_error)?,
                    serde_json::to_string(&case.scoring).map_err(json_error)?,
                    case.difficulty,
                    case.output_format,
                    serde_json::to_string(&case.requirements).map_err(json_error)?,
                    serde_json::to_string(&case.fixture).map_err(json_error)?,
                    case.content_hash
                ],
            )
            .map_err(error)?;
        }
        tx.execute(
            "INSERT INTO benchmark_dataset_reviews VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                review,
                workspace,
                job_id,
                selection.to_string(),
                id,
                version,
                reviewer,
                now()
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(DatasetSummary {
            id,
            version,
            content_hash: hash,
            manifest,
            runtime: Some(json!({"scoringVersion":2})),
        })
    }
}

#[tauri::command]
pub fn plan_benchmark_generation(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    input: GenerationInput,
) -> AppResult<Value> {
    let connection = cloud.guard()?;
    state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .plan_generation(&workspace_id, &connection, input)
}
#[tauri::command]
pub fn get_benchmark_generation(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    job_id: String,
) -> AppResult<Value> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .generation_job(&workspace_id, &job_id)
    })
}
#[tauri::command]
pub fn list_benchmark_generations(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
) -> AppResult<Vec<Value>> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .generation_history(&workspace_id)
    })
}
#[tauri::command]
pub async fn start_benchmark_generation(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    job_id: String,
    approved_plan_hash: String,
) -> AppResult<Value> {
    let connection = cloud.guard()?;
    let lease = state.claim_task(&job_id, &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let plan = db.begin_generation(&workspace_id, &job_id, &approved_plan_hash, &connection)?;
    let initial = db.generation_job(&workspace_id, &job_id)?;
    let cloud = cloud.inner().clone();
    tauri::async_runtime::spawn(async move {
        let mut cancel = lease.receiver();
        let result = tokio::select! {biased;_=cancel.changed()=>Err("用户已停止".into()),r=cloud.complete(&connection,&plan.request)=>r.map_err(|s|format!("云端请求未完成：{s:?}。不会自动重试。"))};
        let _ = blocking(db, move |db| {
            db.finish_generation(&workspace_id, &job_id, result)
        })
        .await;
        drop(lease);
    });
    Ok(initial)
}
#[tauri::command]
pub fn stop_personalized_task(
    state: State<'_, AppState>,
    workspace_id: String,
    task_id: String,
    only_waiting: Option<bool>,
) -> AppResult<()> {
    if only_waiting == Some(true) { return state.tasks.cancel_waiting(&task_id, &workspace_id, crate::tasks::TaskKind::Benchmark); }
    state.cancel_personalized_task(&workspace_id, &task_id)?;
    state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .mark_batch_stopping(&workspace_id, &task_id)
}
#[tauri::command]
pub fn revise_benchmark_candidate(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    job_id: String,
    case_id: String,
    expected_revision: u32,
    candidate: Candidate,
) -> AppResult<Value> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .revise_candidate(
                &workspace_id,
                &job_id,
                &case_id,
                expected_revision,
                candidate,
            )
    })
}
#[tauri::command]
pub fn publish_benchmark_dataset(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    job_id: String,
    selection: Value,
    reviewer: String,
) -> AppResult<DatasetSummary> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .publish_personalized(&workspace_id, &job_id, selection, &reviewer)
    })
}
