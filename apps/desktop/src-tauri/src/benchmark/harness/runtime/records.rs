use super::*;
impl Database {
    pub(super) fn runtime_task(&self, workspace: &str, input: &Input) -> AppResult<Value> {
        python::codex_policy::describe(
            input
                .security_policy
                .as_deref()
                .unwrap_or(python::codex_policy::DEFAULT),
        )?;
        let check = self.harness_preflight(
            workspace,
            input.assessment_version,
            Some(&input.check_id),
        )?["selected"]
            .clone();
        if !input.confirmed
            || check["contentHash"] != input.check_hash
            || !python::valid_image(&input.image_id)
        {
            return Err("请确认当前任务检查、固定镜像和无模型检查范围。".into());
        }
        let task:preflight::CheckInput=serde_json::from_value(json!({"assessmentVersion":input.assessment_version,"datasetId":check["body"]["datasetId"],"version":check["body"]["datasetVersion"],"datasetHash":check["body"]["datasetHash"]})).map_err(json_error)?;
        self.preflight_task(workspace, &task)?;
        Ok(check)
    }
    pub(super) fn begin_runtime(
        &self,
        workspace: &str,
        id: &str,
        input: &Input,
        env: Value,
    ) -> AppResult<()> {
        let check = self.runtime_task(workspace, input)?;
        let created = now();
        let plan = json!({"purpose":PURPOSE,"version":VERSION,"owner":id,"workspaceId":workspace,"assessmentVersion":input.assessment_version,"checkId":input.check_id,"checkHash":input.check_hash,"datasetId":check["body"]["datasetId"],"datasetVersion":check["body"]["datasetVersion"],"datasetHash":check["body"]["datasetHash"],"imageId":input.image_id,"containerName":format!("cortexos-bench-{}",new_id()),"environment":env,"securityPolicy":python::codex_policy::describe(input.security_policy.as_deref().unwrap_or(python::codex_policy::DEFAULT))?,"probeVersion":"codex-sandbox-2","confirmed":true,"modelCalls":0,"taskExecution":"Not Run","createdAt":created});
        self.insert_runtime_record(workspace, id, Kind::Codex, plan)
    }
    pub(super) fn insert_runtime_record(
        &self,
        workspace: &str,
        id: &str,
        kind: Kind,
        plan: Value,
    ) -> AppResult<()> {
        if plan["purpose"] != kind.purpose()
            || plan["workspaceId"] != workspace
            || plan["owner"] != id
        {
            return Err("检查计划用途或归属不匹配。".into());
        }
        let created = plan["createdAt"].as_str().ok_or("检查创建时间缺失。")?;
        let raw = plan.to_string();
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')=?1 AND status IN ('checking','interrupted','cleanup_required'))",[kind.purpose()],|r|r.get(0)).map_err(error)?;
        if occupied {
            return Err("还有隔离环境检查未结束或待清理，请先处理原记录。".into());
        }
        tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'checking',?3,?4,?5)",params![id,workspace,raw,hash(raw.as_bytes()),created]).map_err(error)?;
        tx.commit().map_err(error)
    }
    pub(in crate::benchmark::harness) fn runtime_record(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Value> {
        self.runtime_record_for(workspace, id, Kind::Codex)
    }
    pub(in crate::benchmark::harness) fn runtime_record_for(
        &self,
        workspace: &str,
        id: &str,
        kind: Kind,
    ) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw,digest,status,created,finished,problem):(String,String,String,String,Option<String>,Option<String>)=c.query_row("SELECT plan_json,plan_hash,status,created_at,finished_at,error FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')=?3",params![workspace,id,kind.purpose()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_|"检查记录不存在或不属于当前工作区。".to_string())?;
        let plan: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if hash(raw.as_bytes()) != digest
            || plan["workspaceId"] != workspace
            || plan["owner"] != id
            || plan["purpose"] != kind.purpose()
            || match kind {
                Kind::Aider => plan["version"] != "aider-runtime-check-1",
                Kind::Codex => {
                    !(plan["version"] == VERSION || plan["version"] == "codex-runtime-check-1")
                }
                Kind::Claude => {
                    !(plan["version"] == "claude-runtime-check-1"
                        || plan["version"] == "claude-runtime-check-2")
                }
            }
        {
            return Err("隔离检查的版本或指纹不匹配。".into());
        }
        if kind == Kind::Aider
            && (plan["definition"] != python::aider::definition()
                || plan["securityPolicy"] != python::aider::security_policy())
        {
            return Err("Aider检查版本或保护范围不匹配。".into());
        }
        if kind == Kind::Claude {
            let legacy = plan["version"] == "claude-runtime-check-1";
            let expected = if legacy {
                "claude-program-1"
            } else {
                "claude-program-2"
            };
            let policy = if legacy {
                json!({"id":"docker-default"})
            } else {
                python::claude_policy::describe(
                    plan["securityPolicy"]["id"]
                        .as_str()
                        .ok_or("保护策略记录缺失。")?,
                )?
            };
            if plan["definition"] != python::claude::definition_for(expected)
                || plan["securityPolicy"] != policy
                || plan["probeVersion"] != expected
            {
                return Err("Claude检查版本或保护范围不匹配，已保留记录。".into());
            }
        }
        if kind == Kind::Codex && plan["version"] == VERSION {
            let policy_id = plan["securityPolicy"]["id"]
                .as_str()
                .ok_or("保护策略记录缺失。")?;
            if plan["securityPolicy"] != python::codex_policy::describe(policy_id)?
                || plan["probeVersion"] != "codex-sandbox-2"
            {
                return Err("保护策略或探针版本不匹配，已保留原记录。".into());
            }
        }
        let mut q=c.prepare("SELECT candidate_json,validation_json FROM benchmark_generated_cases WHERE job_id=?1 ORDER BY rowid").map_err(error)?;
        let events = q
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?
            .into_iter()
            .map(|(raw, proof)| -> AppResult<Value> {
                let proof: Value = serde_json::from_str(&proof).map_err(json_error)?;
                if proof["contentHash"] != hash(raw.as_bytes()) {
                    return Err("隔离检查结果指纹不匹配。".into());
                }
                serde_json::from_str(&raw).map_err(json_error)
            })
            .collect::<AppResult<Vec<_>>>()?;
        let clean = events.iter().any(|e| {
            e["kind"] == "cleanup"
                && e["data"]["clean"] == true
                && e["data"]["name"] == plan["containerName"]
        });
        Ok(
            json!({"id":id,"plan":plan,"planHash":digest,"status":status,"createdAt":created,"finishedAt":finished,"error":problem,"events":events,"clean":clean,"modelCalls":0,"taskExecution":"Not Run"}),
        )
    }
    pub(super) fn runtime_event_for(
        &self,
        workspace: &str,
        id: &str,
        kind_scope: Kind,
        kind: &str,
        data: Value,
    ) -> AppResult<()> {
        self.runtime_record_for(workspace, id, kind_scope)?;
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
    pub(super) fn finish_runtime_for(
        &self,
        workspace: &str,
        id: &str,
        kind: Kind,
        status: &str,
        problem: Option<String>,
    ) -> AppResult<()> {
        let r = self.runtime_record_for(workspace, id, kind)?;
        if status != "cleanup_required" && r["clean"] != true {
            return Err("检查容器尚未清理，不能结束记录。".into());
        }
        self.connection()?.execute("UPDATE benchmark_generation_jobs SET status=?1,error=COALESCE(?2,error),finished_at=?3 WHERE workspace_id=?4 AND id=?5 AND json_extract(plan_json,'$.purpose')=?6",params![status,problem,now(),workspace,id,kind.purpose()]).map_err(error)?;
        Ok(())
    }
    pub(crate) fn pending_codex_runtime_jobs(&self) -> AppResult<Vec<(String, String)>> {
        self.pending_runtime_jobs(Kind::Codex)
    }
    pub(crate) fn pending_aider_runtime_jobs(&self) -> AppResult<Vec<(String, String)>> {
        self.pending_runtime_jobs(Kind::Aider)
    }
    pub(crate) fn pending_claude_runtime_jobs(&self) -> AppResult<Vec<(String, String)>> {
        self.pending_runtime_jobs(Kind::Claude)
    }
    pub(super) fn pending_runtime_jobs(&self, kind: Kind) -> AppResult<Vec<(String, String)>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT workspace_id,id FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')=?1 AND status IN ('checking','interrupted','cleanup_required') ORDER BY created_at").map_err(error)?;
        q.query_map([kind.purpose()], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)
    }
    pub(super) fn runtime_state(
        &self,
        workspace: &str,
        assessment: u32,
        check: &str,
        id: Option<&str>,
    ) -> AppResult<Value> {
        self.harness_preflight(workspace, assessment, Some(check))?;
        let c = self.connection()?;
        let mut q=c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.checkId')=?3 ORDER BY created_at DESC,rowid DESC LIMIT 30").map_err(error)?;
        let history=q.query_map(params![workspace,PURPOSE,check],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|r| r["id"].as_str()))
            .map(|id| self.runtime_record(workspace, id))
            .transpose()?;
        if selected.as_ref().is_some_and(|r| {
            r["plan"]["checkId"] != check || r["plan"]["assessmentVersion"] != assessment
        }) {
            return Err("检查不属于当前任务或分析版本。".into());
        }
        let active = self
            .pending_codex_runtime_jobs()?
            .into_iter()
            .find(|(w, _)| w == workspace)
            .map(|(_, id)| self.runtime_record(workspace, &id))
            .transpose()?
            .map(|r| json!({"id":r["id"],"checkId":r["plan"]["checkId"],"status":r["status"]}));
        Ok(json!({"history":history,"selected":selected,"active":active}))
    }
}
