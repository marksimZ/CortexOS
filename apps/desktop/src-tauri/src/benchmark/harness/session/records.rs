use super::*;
impl Database {
    pub(super) fn session_source(
        &self,
        scope: Kind,
        w: &str,
        input: &Input,
    ) -> AppResult<(Value, Value)> {
        let runtime = self.runtime_record_for(w, &input.runtime_id, scope.runtime())?;
        let p = &runtime["plan"];
        if !input.confirmed
            || input.model.expected_digest.is_none()
            || serde_json::to_value(&input.model.parameters).map_err(json_error)?
                != serde_json::to_value(parameters()).map_err(json_error)?
            || input.model.requirements.as_slice()
                != if scope == Kind::Aider {
                    &["completion"][..]
                } else {
                    &["completion", "tool_calls"][..]
                }
            || runtime["planHash"] != input.runtime_hash
            || runtime["status"] != "completed"
            || runtime["clean"] != true
            || p["assessmentVersion"] != input.assessment_version
            || (scope == Kind::Codex && p["checkId"] != input.check_id)
            || p["securityPolicy"] != scope.runtime_policy()?
            || !experiment::runtime_ready(scope, &runtime)
        {
            return Err("请核对已通过的兼容隔离检查、实际模型版本和本次单题调用确认。".into());
        }
        let task = self.preflight_task(w, &workspace::task_input(p)?)?;
        Ok((runtime, task))
    }
    pub(super) fn session_plan(
        &self,
        scope: Kind,
        w: &str,
        id: &str,
        input: Input,
        inspection: Value,
        env: Value,
    ) -> AppResult<Value> {
        let (runtime, task) = self.session_source(scope, w, &input)?;
        let source = &runtime["plan"];
        let mut plan = json!({"purpose":scope.purpose(),"version":scope.version(),"adapter":python::session::descriptor_for(scope),"owner":id,"workspaceId":w,"assessmentVersion":input.assessment_version,"checkId":input.check_id,"runtimeId":input.runtime_id,"runtimeHash":input.runtime_hash,"datasetId":source["datasetId"],"datasetVersion":source["datasetVersion"],"datasetHash":source["datasetHash"],"imageId":source["imageId"],"gradingImageId":task["payload"]["validation"]["plan"]["imageId"],"containerName":format!("cortexos-bench-{}",new_id()),"environment":env,"model":input.model,"modelInspection":inspection,"securityPolicy":scope.policy()?,"budget":scope.budget(),"bundle":workspace::package::bundle(&task)?,"evaluatorVersion":evaluation::ENGINE_VERSION,"confirmed":true,"createdAt":now()});
        if plan["bundle"]["scope"] == "python_project" {
            plan["projectRunner"] = task["payload"]["validation"]["plan"]["projectRunner"].clone();
            if plan["projectRunner"] != python::project_runner_hash() {
                return Err("评分环境已变更，请用当前版本重新验证并发布此项目任务。".into());
            }
        }
        Ok(plan)
    }
    pub(super) fn insert_session_plan(&self, w: &str, id: &str, plan: &Value) -> AppResult<()> {
        let raw = plan.to_string();
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose') IN ('harness_codex_session','harness_claude_session','harness_aider_session','harness_codex_runtime','harness_claude_runtime','harness_aider_runtime','harness_workspace') AND status IN ('running','checking','opening','ready','grading','interrupted','cleanup_required'))",[],|r|r.get(0)).map_err(error)?;
        let experiment = plan["experimentId"].as_str();
        let competing: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')='harness_experiment' AND status='running' AND id<>COALESCE(?1,''))",
            [experiment], |r| r.get(0),
        ).map_err(error)?;
        if competing {
            return Err("已有组合实验正在执行。".into());
        }
        if occupied {
            return Err("还有任务或隔离副本未结束，请先停止或清理。".into());
        }
        tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'running',?3,?4,?5)",params![id,w,raw,hash(raw.as_bytes()),now()]).map_err(error)?;
        tx.commit().map_err(error)
    }
    pub(super) fn session_record(&self, scope: Kind, w: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, w)?;
        let (raw,digest,status,created,finished,problem):(String,String,String,String,Option<String>,Option<String>)=c.query_row("SELECT plan_json,plan_hash,status,created_at,finished_at,error FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')=?3",params![w,id,scope.purpose()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_|"执行记录不存在或不属于当前工作区。")?;
        let p: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if hash(raw.as_bytes()) != digest
            || p["purpose"] != scope.purpose()
            || p["version"] != scope.version()
            || p["owner"] != id
            || p["workspaceId"] != w
            || p["budget"] != scope.budget()
            || p["securityPolicy"] != scope.policy()?
        {
            return Err("执行计划、预算或归属校验失败。".into());
        }
        let mut q=c.prepare("SELECT candidate_json,validation_json FROM benchmark_generated_cases WHERE job_id=?1 ORDER BY rowid").map_err(error)?;
        let rows = q
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let events = rows
            .into_iter()
            .map(|(raw, proof)| -> AppResult<Value> {
                let proof: Value = serde_json::from_str(&proof).map_err(json_error)?;
                if proof["contentHash"] != hash(raw.as_bytes()) {
                    return Err("执行事件指纹不匹配。".into());
                }
                serde_json::from_str(&raw).map_err(json_error)
            })
            .collect::<AppResult<Vec<_>>>()?;
        let clean = events.iter().any(|e| {
            e["kind"] == "cleanup"
                && e["data"]["clean"] == true
                && e["data"]["name"] == p["containerName"]
        });
        let pending: Vec<_> = events
            .iter()
            .filter(|e| e["kind"] == "grader_container")
            .filter_map(|e| e["data"]["name"].as_str())
            .filter(|name| {
                !events.iter().any(|e| {
                    e["kind"] == "grader_cleanup"
                        && e["data"]["name"] == *name
                        && e["data"]["clean"] == true
                        || e["kind"] == "score"
                            && e["data"]["score"]["details"]["ownedContainers"]
                                .as_array()
                                .is_some_and(|a| {
                                    a.iter().any(|r| r["name"] == *name && r["removed"] == true)
                                })
                })
            })
            .collect();
        Ok(
            json!({"id":id,"plan":p,"planHash":digest,"status":status,"createdAt":created,"finishedAt":finished,"error":problem,"clean":clean,"pendingGraders":pending,"modelCalls":events.iter().filter(|e|e["kind"]=="model_requested").count(),"events":events}),
        )
    }
    pub(super) fn session_event(
        &self,
        scope: Kind,
        w: &str,
        id: &str,
        kind: &str,
        data: Value,
    ) -> AppResult<()> {
        self.session_record(scope, w, id)?;
        let raw = json!({"kind":kind,"data":data,"createdAt":now()}).to_string();
        self.connection()?
            .execute(
                "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
                params![
                    id,
                    new_id(),
                    raw,
                    json!({"contentHash":hash(raw.as_bytes())}).to_string(),
                    now()
                ],
            )
            .map_err(error)?;
        Ok(())
    }
    pub(super) fn finish_session(
        &self,
        scope: Kind,
        w: &str,
        id: &str,
        status: &str,
        problem: Option<String>,
    ) -> AppResult<()> {
        let r = self.session_record(scope, w, id)?;
        if status != "cleanup_required"
            && (r["clean"] != true || r["pendingGraders"].as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err("执行副本或测试容器尚未清理。".into());
        }
        self.connection()?.execute("UPDATE benchmark_generation_jobs SET status=?1,error=COALESCE(?2,error),finished_at=?3 WHERE workspace_id=?4 AND id=?5 AND json_extract(plan_json,'$.purpose')=?6",params![status,problem,now(),w,id,scope.purpose()]).map_err(error)?;
        Ok(())
    }
    pub(crate) fn pending_sessions(&self, scope: Kind) -> AppResult<Vec<(String, String)>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT workspace_id,id FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')=?1 AND status IN ('running','grading','interrupted','cleanup_required') ORDER BY created_at").map_err(error)?;
        q.query_map([scope.purpose()], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)
    }
    pub(super) fn session_state(
        &self,
        scope: Kind,
        w: &str,
        assessment: u32,
        check: &str,
        id: Option<&str>,
    ) -> AppResult<Value> {
        if scope == Kind::Codex {
            self.harness_preflight(w, assessment, Some(check))?;
        } else {
            self.fit_read(w, Some(assessment))?;
        }
        let c = self.connection()?;
        let mut q=c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.checkId')=?3 AND json_extract(plan_json,'$.assessmentVersion')=?4 ORDER BY created_at DESC,rowid DESC LIMIT 30").map_err(error)?;
        let history=q.query_map(params![w,scope.purpose(),check,assessment],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|r| r["id"].as_str()))
            .map(|id| self.session_record(scope, w, id))
            .transpose()?;
        if selected.as_ref().is_some_and(|r| {
            r["plan"]["assessmentVersion"] != assessment || r["plan"]["checkId"] != check
        }) {
            return Err("执行记录不属于当前检查和分析版本。".into());
        }
        let active = self
            .pending_sessions(scope)?
            .into_iter()
            .find(|(scope, _)| scope == w)
            .map(|(_, id)| self.session_record(scope, w, &id))
            .transpose()?
            .map(|r| json!({"id":r["id"],"checkId":r["plan"]["checkId"],"assessmentVersion":r["plan"]["assessmentVersion"],"status":r["status"]}));
        Ok(json!({"history":history,"selected":selected,"active":active,"budget":scope.budget()}))
    }
}
