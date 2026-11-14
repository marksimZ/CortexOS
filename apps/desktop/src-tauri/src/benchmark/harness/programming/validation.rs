//! Audited pure-function controls; no model calls or repository script execution.
use super::*;
use crate::{
    benchmark::{
        evaluation::{self, State as Grade},
        python,
    },
    tasks::TaskKind,
};
use tokio::sync::watch;

const PURPOSE: &str = "harness_programming_validation";
const RULES: &str = "python-task-controls-1";
const PUBLISHED: &str = "harness_programming_tasks";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    draft_id: String,
    expected_revision: u32,
    draft_hash: String,
    image_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publish {
    draft_id: String,
    expected_revision: u32,
    validation_id: String,
    validation_hash: String,
    reviewer: String,
    approved: bool,
    expected_version: u32,
}

fn complete(score: &Value, count: usize) -> bool {
    score["details"]["assertions"].as_array().is_some_and(|a| {
        a.len() == count
            && a.iter()
                .enumerate()
                .all(|(i, r)| r["index"] == i + 1 && r["passed"].is_boolean())
    }) && score["details"]["ownedContainers"]
        .as_array()
        .is_some_and(|a| {
            a.len() == count + usize::from(score["details"]["buildStatus"] == "passed")
        })
}
fn comparisons(body: &Value, records: &serde_json::Map<String, Value>) -> Value {
    let examples = body["spec"]["examples"].as_array().unwrap();
    let result = |name: &str| {
        records
            .get(name)
            .map(|v| &v["score"])
            .unwrap_or(&Value::Null)
    };
    let reference = result("reference");
    let original = result("unchanged");
    let wrong = result("wrong");
    let full = [reference, original, wrong]
        .iter()
        .all(|s| complete(s, examples.len()));
    let original_results = original["details"]["assertions"].as_array();
    let target_failed = examples.iter().enumerate().any(|(i, e)| {
        e["role"] == "target"
            && original_results
                .and_then(|a| a.get(i))
                .is_some_and(|r| r["passed"] == false)
    });
    let regression = examples
        .iter()
        .enumerate()
        .filter(|(_, e)| e["role"] == "regression")
        .all(|(i, _)| {
            original_results
                .and_then(|a| a.get(i))
                .is_some_and(|r| r["passed"] == true)
        });
    let negative = wrong["details"]["assertions"].as_array().is_some_and(|a| {
        a.iter()
            .any(|r| r["passed"] == false && r["observation"]["outcome"] == "returned")
    });
    let cleaned = [reference, original, wrong].iter().all(|s| {
        s["details"]["ownedContainers"]
            .as_array()
            .is_some_and(|a| !a.is_empty() && a.iter().all(|c| c["removed"] == true))
    });
    let checks = json!({"referencePassed":reference["state"]=="Passed","originalTargetFailed":original["state"]=="Failed"&&target_failed,"regressionPreserved":regression,"negativeDetected":wrong["state"]=="Failed"&&negative,"allAssertionsRecorded":full,"containersClean":cleaned});
    json!({"passed":checks.as_object().unwrap().values().all(|v|v==true),"checks":checks,"rulesVersion":RULES})
}

impl Database {
    fn checked_draft_revision(
        &self,
        workspace: &str,
        id: &str,
        revision: u32,
        latest: bool,
    ) -> AppResult<Value> {
        let draft = self.programming_job(workspace, id)?;
        let rows = draft["revisions"].as_array().ok_or("草案修订缺失。")?;
        if latest && rows[0]["revision"] != revision {
            return Err("草案已有新修订，请重新读取并验证。".into());
        }
        let row = rows
            .iter()
            .find(|r| r["revision"] == revision)
            .ok_or("草案修订不存在。")?;
        let origin: Origin =
            serde_json::from_value(row["body"]["origin"].clone()).map_err(json_error)?;
        let spec: Spec = serde_json::from_value(row["body"]["spec"].clone()).map_err(json_error)?;
        if prepare(self, workspace, &origin, &spec, id)? != row["body"] {
            return Err("草案来源或评分合同已不一致，请保留数据并另存新修订。".into());
        }
        Ok(row.clone())
    }
    fn validation_record(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw,hash_value,status,created,finished,error_value):(String,String,String,String,Option<String>,Option<String>)=c.query_row("SELECT plan_json,plan_hash,status,created_at,finished_at,error FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')='harness_programming_validation'",params![workspace,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_|"验证记录不存在或不属于当前工作区。".to_string())?;
        if hash(raw.as_bytes()) != hash_value {
            return Err("验证计划指纹校验失败。".into());
        }
        let plan: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if plan["rulesVersion"] != RULES {
            return Err("验证规则版本不受支持。".into());
        }
        let mut q=c.prepare("SELECT case_id,candidate_json,validation_json FROM benchmark_generated_cases WHERE job_id=?1 AND revision=1 ORDER BY rowid").map_err(error)?;
        let stored = q
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        drop(q);
        drop(c);
        let mut records = serde_json::Map::new();
        for (key, body, verification) in stored {
            let h: Value = serde_json::from_str(&verification).map_err(json_error)?;
            if h["contentHash"] != hash(body.as_bytes()) {
                return Err("验证结果指纹校验失败。".into());
            }
            records.insert(key, serde_json::from_str(&body).map_err(json_error)?);
        }
        let draft = self.checked_draft_revision(
            workspace,
            plan["draftId"].as_str().ok_or("验证草案缺失。")?,
            plan["revision"].as_u64().ok_or("修订号缺失。")? as u32,
            false,
        )?;
        if draft["contentHash"] != plan["draftHash"] {
            return Err("验证与草案指纹不一致。".into());
        }
        let mut pending = BTreeSet::new();
        for (key, value) in &records {
            if key.starts_with("container-") {
                pending.insert(
                    value["name"]
                        .as_str()
                        .ok_or("容器归属记录缺失。")?
                        .to_string(),
                );
            }
        }
        for value in records.values() {
            if let Some(containers) = value["score"]["details"]["ownedContainers"].as_array() {
                for c in containers {
                    if c["removed"] == true {
                        if let Some(name) = c["name"].as_str() {
                            pending.remove(name);
                        }
                    }
                }
            }
            if let Some(cleaned) = value["cleanup"].as_array() {
                for c in cleaned {
                    if c["clean"] == true {
                        if let Some(name) = c["name"].as_str() {
                            pending.remove(name);
                        }
                    }
                }
            }
        }
        let mut outcome = comparisons(&draft["body"], &records);
        let evidence_bound = ["reference", "unchanged", "wrong"].iter().all(|name| {
            records.get(*name).is_some_and(|r| {
                r["codeHash"] == plan["codeHashes"][*name]
                    && r["score"]["details"]["imageId"] == plan["imageId"]
                    && r["score"]["details"]["sandboxPolicy"] == plan["sandboxPolicy"]
                    && r["score"]["details"]["engineVersion"] == plan["evaluatorVersion"]
                    && r["score"]["details"]["projectRunner"] == plan["projectRunner"]
                    && r["score"]["details"]["ownedContainers"]
                        .as_array()
                        .is_some_and(|a| {
                            a.iter().all(|container| {
                                container["name"].as_str().is_some_and(|n| {
                                    records.get(&format!("container-{n}")).is_some_and(|owner| {
                                        owner["name"] == n
                                            && owner["owner"] == id
                                            && owner["variant"] == *name
                                    })
                                })
                            })
                        })
            })
        });
        outcome["checks"]["evidenceBound"] = json!(evidence_bound);
        outcome["passed"] = json!(
            outcome["checks"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == true)
        );
        let content_hash = hash(
            json!({"plan":plan,"records":records,"status":status})
                .to_string()
                .as_bytes(),
        );
        Ok(
            json!({"id":id,"plan":plan,"planHash":hash_value,"status":status,"createdAt":created,"finishedAt":finished,"error":error_value,"records":records,"examples":draft["body"]["spec"]["examples"],"comparison":outcome,"evidenceBound":evidence_bound,"pendingContainers":pending,"publishable":status=="completed"&&outcome["passed"]==true&&pending.is_empty()&&evidence_bound,"contentHash":content_hash}),
        )
    }
    fn append_validation(
        &self,
        workspace: &str,
        id: &str,
        key: &str,
        body: Value,
    ) -> AppResult<()> {
        let c = self.connection()?;
        let active:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND status='validating' AND json_extract(plan_json,'$.purpose')='harness_programming_validation')",params![workspace,id],|r|r.get(0)).map_err(error)?;
        if !active {
            return Err("验证已结束，不追加执行结果。".into());
        }
        let raw = body.to_string();
        c.execute(
            "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
            params![
                id,
                key,
                raw,
                json!({"contentHash":hash(raw.as_bytes())}).to_string(),
                now()
            ],
        )
        .map_err(error)?;
        Ok(())
    }
    fn begin_programming_validation(
        &self,
        workspace: &str,
        id: &str,
        input: &Start,
    ) -> AppResult<Value> {
        if !python::valid_image(&input.image_id) {
            return Err("请选择实际的本地Python镜像。".into());
        }
        let draft =
            self.checked_draft_revision(workspace, &input.draft_id, input.expected_revision, true)?;
        if draft["contentHash"] != input.draft_hash {
            return Err("草案已更新，请重新读取。".into());
        }
        let mut plan = json!({"purpose":PURPOSE,"rulesVersion":RULES,"draftId":input.draft_id,"revision":input.expected_revision,"draftHash":input.draft_hash,"imageId":input.image_id,"caseHash":draft["body"]["case"]["content_hash"],"sandboxPolicy":python::policy(),"evaluatorVersion":evaluation::ENGINE_VERSION,"taskExecution":"Not Run"});
        plan["codeHashes"] = json!(
            project::variants(&draft["body"])
                .into_iter()
                .map(|(name, content)| (name, project::content_hash(&content)))
                .collect::<std::collections::BTreeMap<_, _>>()
        );
        if draft["body"]["project"].is_object() {
            plan["projectRunner"] = json!(python::project_runner_hash());
        }
        let raw = plan.to_string();
        let c = self.connection()?;
        c.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'validating',?3,?4,?5)",params![id,workspace,raw,hash(raw.as_bytes()),now()]).map_err(error)?;
        Ok(draft["body"].clone())
    }
    fn finish_programming_validation(
        &self,
        workspace: &str,
        id: &str,
        status: &str,
        problem: Option<String>,
    ) -> AppResult<()> {
        self.connection()?.execute("UPDATE benchmark_generation_jobs SET status=?1,error=?2,finished_at=?3 WHERE workspace_id=?4 AND id=?5 AND status='validating' AND json_extract(plan_json,'$.purpose')='harness_programming_validation'",params![status,problem,now(),workspace,id]).map_err(error)?;
        Ok(())
    }
    pub fn programming_validation_state(
        &self,
        workspace: &str,
        draft: &str,
        selected: Option<&str>,
    ) -> AppResult<Value> {
        self.programming_job(workspace, draft)?;
        let c = self.connection()?;
        let mut q=c.prepare("SELECT id,status,created_at,json_extract(plan_json,'$.revision') FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')='harness_programming_validation' AND json_extract(plan_json,'$.draftId')=?2 ORDER BY created_at DESC,id DESC LIMIT 50").map_err(error)?;
        let history=q.query_map(params![workspace,draft],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?,"revision":r.get::<_,u32>(3)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        let mut q=c.prepare("SELECT id,version,content_hash,created_at FROM benchmark_datasets WHERE json_extract(provenance_json,'$.workspaceId')=?1 AND json_extract(provenance_json,'$.purpose')='harness_programming_tasks' AND json_extract(provenance_json,'$.generationJobId')=?2 ORDER BY version DESC LIMIT 100").map_err(error)?;
        let published=q.query_map(params![workspace,draft],|r|Ok(json!({"id":r.get::<_,String>(0)?,"version":r.get::<_,u32>(1)?,"contentHash":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        let record = selected
            .or_else(|| history.first().and_then(|r| r["id"].as_str()))
            .map(|id| self.validation_record(workspace, id))
            .transpose()?;
        if record
            .as_ref()
            .is_some_and(|r| r["plan"]["draftId"] != draft)
        {
            return Err("验证记录不属于此草案。".into());
        }
        Ok(json!({"history":history,"selected":record,"published":published}))
    }
    fn cleanup_programming_validation(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let record = self.validation_record(workspace, id)?;
        if record["status"] == "validating" {
            return Err("请先停止正在进行的验证。".into());
        }
        let pending = record["pendingContainers"]
            .as_array()
            .ok_or("容器记录缺失。")?;
        if pending.is_empty() {
            return Ok(record);
        }
        let endpoint = record["records"]["environment"]["endpoint"]
            .as_str()
            .ok_or("原Docker端点记录缺失，停止自动清理。")?;
        let image = record["plan"]["imageId"].as_str().ok_or("镜像记录缺失。")?;
        let mut cleanup = vec![];
        for item in pending {
            let name = item.as_str().ok_or("容器名缺失。")?;
            match python::cleanup_validation_container(endpoint, image, id, name) {
                Ok(value) => cleanup.push(value),
                Err(reason) => cleanup.push(json!({"name":name,"clean":false,"reason":reason})),
            }
        }
        let body = json!({"cleanup":cleanup,"createdAt":now()});
        let raw = body.to_string();
        self.connection()?
            .execute(
                "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
                params![
                    id,
                    format!("cleanup-{}", new_id()),
                    raw,
                    json!({"contentHash":hash(raw.as_bytes())}).to_string(),
                    now()
                ],
            )
            .map_err(error)?;
        self.validation_record(workspace, id)
    }
    fn publish_programming_task(&self, workspace: &str, input: Publish) -> AppResult<Value> {
        if !input.approved
            || input.reviewer.trim().is_empty()
            || input.reviewer.chars().count() > 100
        {
            return Err("请完成人工核对并填写审核人。".into());
        }
        let draft =
            self.checked_draft_revision(workspace, &input.draft_id, input.expected_revision, true)?;
        let validation = self.validation_record(workspace, &input.validation_id)?;
        if validation["publishable"] != true
            || validation["contentHash"] != input.validation_hash
            || validation["plan"]["draftId"] != input.draft_id
            || validation["plan"]["revision"] != input.expected_revision
            || validation["plan"]["draftHash"] != draft["contentHash"]
        {
            return Err("当前修订尚无通过的完整验证，或验证记录已变化。".into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let latest:u32=tx.query_row("SELECT MAX(revision) FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?1",[&input.draft_id],|r|r.get(0)).map_err(error)?;
        if latest != input.expected_revision {
            return Err("草案刚被修订，请重新验证。".into());
        }
        let id = format!("harness-python-{}", input.draft_id);
        let current: u32 = tx
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM benchmark_datasets WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if current != input.expected_version {
            return Err("已有新发布版本，请重新读取。".into());
        }
        let version = current.checked_add(1).ok_or("发布版本过多。")?;
        let mut case: Case =
            serde_json::from_value(draft["body"]["case"].clone()).map_err(json_error)?;
        case.dataset_version = version;
        cases::seal(&mut case)?;
        let review = new_id();
        let payload = json!({"task":draft,"case":case,"validation":validation,"review":{"id":review,"reviewer":input.reviewer.trim(),"approvedRevision":input.expected_revision,"createdAt":now()},"taskExecution":"Not Run"});
        let content_hash = hash(payload.to_string().as_bytes());
        let manifest = json!({"name":"项目Python任务","version":version,"case_count":1,"content_hash":content_hash,"review_status":"human_approved","execution":"Not Run","payload":payload,"provenance":{"purpose":PUBLISHED,"workspaceId":workspace,"generationJobId":input.draft_id,"validationId":input.validation_id,"reviewId":review}});
        tx.execute(
            "INSERT INTO benchmark_datasets VALUES(?1,?2,'项目Python任务',2,?3,?4,?5,?6)",
            params![
                id,
                version,
                content_hash,
                manifest.to_string(),
                manifest["provenance"].to_string(),
                now()
            ],
        )
        .map_err(error)?;
        tx.execute("INSERT INTO benchmark_dataset_reviews VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![review,workspace,input.draft_id,json!({"revision":input.expected_revision,"draftHash":payload["task"]["contentHash"],"validationId":input.validation_id,"validationHash":input.validation_hash,"approved":true}).to_string(),id,version,input.reviewer.trim(),now()]).map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(manifest)
    }
    pub(crate) fn programming_taskset(
        &self,
        workspace: &str,
        id: &str,
        version: u32,
    ) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let(raw,stored):(String,String)=c.query_row("SELECT manifest_json,content_hash FROM benchmark_datasets WHERE id=?1 AND version=?2 AND json_extract(provenance_json,'$.workspaceId')=?3 AND json_extract(provenance_json,'$.purpose')='harness_programming_tasks'",params![id,version,workspace],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"发布版本不存在或不属于当前工作区。".to_string())?;
        let body: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if body["content_hash"] != stored || hash(body["payload"].to_string().as_bytes()) != stored
        {
            return Err("发布版本指纹校验失败。".into());
        }
        Ok(body)
    }
}

fn execute(
    db: &Database,
    workspace: &str,
    id: &str,
    body: Value,
    image: &str,
    cancel: watch::Receiver<bool>,
) -> AppResult<()> {
    let environment = match python::harness_environment(image, Some(&cancel)) {
        Ok(e) => e,
        Err(reason) => {
            db.finish_programming_validation(
                workspace,
                id,
                if reason == "cancelled" {
                    "cancelled"
                } else {
                    "failed"
                },
                Some(reason),
            )?;
            return Ok(());
        }
    };
    db.append_validation(workspace, id, "environment", environment)?;
    let case: Case = serde_json::from_value(body["case"].clone()).map_err(json_error)?;
    for (name, content) in project::variants(&body) {
        if *cancel.borrow() {
            return db.finish_programming_validation(workspace, id, "cancelled", None);
        }
        let started = now();
        let score = python::score_candidate(
            &case,
            &content,
            body["project"]["entry"].as_str(),
            image,
            id,
            &cancel,
            |container| {
                db.append_validation(
                    workspace,
                    id,
                    &format!("container-{container}"),
                    json!({"name":container,"owner":id,"variant":name,"createdAt":now()}),
                )
            },
        );
        db.append_validation(workspace,id,name,json!({"score":score,"codeHash":project::content_hash(&content),"startedAt":started,"finishedAt":now()}))?;
        if score.state == Grade::Cancelled {
            return db.finish_programming_validation(workspace, id, "cancelled", None);
        }
        if matches!(score.state, Grade::ExecutionError | Grade::ScoringError) {
            return db.finish_programming_validation(
                workspace,
                id,
                "failed",
                Some(
                    score.details["reason"]
                        .as_str()
                        .unwrap_or("validation_failed")
                        .into(),
                ),
            );
        }
    }
    db.finish_programming_validation(workspace, id, "completed", None)
}

#[tauri::command]
pub async fn get_harness_python_environment() -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(python::harness_images)
        .await
        .map_err(|_| "无法读取容器环境。".to_string())?
        .map_err(|e| format!("无法读取本地Python镜像，请确认Docker的Linux引擎正在运行。错误：{e}"))
}
#[tauri::command]
pub async fn get_harness_programming_validation(
    state: State<'_, AppState>,
    workspace_id: String,
    draft_id: String,
    validation_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.programming_validation_state(&workspace_id, &draft_id, validation_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn start_harness_programming_validation(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Start,
) -> AppResult<Value> {
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let task = id.clone();
    let image = input.image_id.clone();
    let body = blocking(db.clone(), move |db| {
        db.begin_programming_validation(&scope, &task, &input)
    })
    .await?;
    let cancel = lease.receiver();
    let scope = workspace_id.clone();
    let task = id.clone();
    let work_db = db.clone();
    let response = json!({"id":id});
    tauri::async_runtime::spawn(async move {
        let work = blocking(work_db, move |db| {
            execute(db, &scope, &task, body, &image, cancel)
        })
        .await;
        if let Err(error) = work {
            let scope = workspace_id.clone();
            let task = id.clone();
            let _ = blocking(db, move |db| {
                db.finish_programming_validation(&scope, &task, "failed", Some(error))
            })
            .await;
        }
        drop(lease);
    });
    Ok(response)
}
#[tauri::command]
pub async fn stop_harness_programming_validation(
    state: State<'_, AppState>,
    workspace_id: String,
    validation_id: String,
) -> AppResult<()> {
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let task = validation_id.clone();
    blocking(db, move |db| db.validation_record(&scope, &task)).await?;
    state.cancel_personalized_task(&workspace_id, &validation_id)
}
#[tauri::command]
pub async fn cleanup_harness_programming_validation(
    state: State<'_, AppState>,
    workspace_id: String,
    validation_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.cleanup_programming_validation(&workspace_id, &validation_id)
    })
    .await
}
#[tauri::command]
pub async fn publish_harness_programming_task(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Publish,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.publish_programming_task(&workspace_id, input)
    })
    .await
}
#[tauri::command]
pub async fn get_harness_programming_taskset(
    state: State<'_, AppState>,
    workspace_id: String,
    dataset_id: String,
    version: u32,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.programming_taskset(&workspace_id, &dataset_id, version)
    })
    .await
}

#[cfg(test)]
mod tests;
