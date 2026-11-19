use super::*;
impl Database {
    pub(in crate::benchmark::harness) fn begin_workspace(
        &self,
        workspace: &str,
        id: &str,
        input: &Prepare,
    ) -> AppResult<()> {
        let check = self.harness_preflight(
            workspace,
            input.assessment_version,
            Some(&input.check_id),
        )?["selected"]
            .clone();
        if check["contentHash"] != input.check_hash {
            return Err("执行前检查已不匹配，请重新读取。".into());
        }
        let mut plan = check["body"].clone();
        let task = self.preflight_task(workspace, &task_input(&plan)?)?;
        let bundle = package::bundle(&task)?;
        if bundle["scope"] == "python_project" {
            return Err("项目任务请在编程工具运行区提交，手动交接只接收单个函数。".into());
        }
        let created = now();
        let expires = (chrono::Utc::now() + chrono::Duration::seconds(600))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        plan = json!({"purpose":PURPOSE,"version":VERSION,"owner":id,"workspaceId":workspace,"assessmentVersion":input.assessment_version,"checkId":input.check_id,"checkHash":input.check_hash,"datasetId":plan["datasetId"],"datasetVersion":plan["datasetVersion"],"datasetHash":plan["datasetHash"],"imageId":plan["imageId"],"containerName":format!("cortexos-bench-{}",new_id()),"bundle":bundle,"evaluatorVersion":evaluation::ENGINE_VERSION,"taskExecution":"Not Run","modelCalls":0,"createdAt":created,"expiresAt":expires});
        let raw = plan.to_string();
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')=?1 AND status IN ('opening','ready','grading','interrupted','cleanup_required'))",[PURPOSE],|r|r.get(0)).map_err(error)?;
        if occupied {
            return Err("已有任务副本尚未关闭或清理，请先处理原副本。".into());
        }
        tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'opening',?3,?4,?5)",params![id,workspace,raw,hash(raw.as_bytes()),created]).map_err(error)?;
        tx.commit().map_err(error)
    }
    pub(in crate::benchmark::harness) fn workspace_record(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw,digest,status,created,finished,problem):(String,String,String,String,Option<String>,Option<String>)=c.query_row("SELECT plan_json,plan_hash,status,created_at,finished_at,error FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')=?3",params![workspace,id,PURPOSE],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_|"任务副本不存在或不属于当前工作区。".to_string())?;
        if hash(raw.as_bytes()) != digest {
            return Err("副本计划指纹校验失败。".into());
        }
        let plan: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if plan["version"] != VERSION || plan["workspaceId"] != workspace || plan["owner"] != id {
            return Err("副本版本或归属无效。".into());
        }
        let mut q=c.prepare("SELECT candidate_json,validation_json,created_at FROM benchmark_generated_cases WHERE job_id=?1 ORDER BY rowid").map_err(error)?;
        let rows = q
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
        let events = rows
            .into_iter()
            .map(|(raw, proof, created)| -> AppResult<Value> {
                let proof: Value = serde_json::from_str(&proof).map_err(json_error)?;
                if proof["contentHash"] != hash(raw.as_bytes()) {
                    return Err("副本观察记录指纹校验失败。".into());
                }
                let mut body: Value = serde_json::from_str(&raw).map_err(json_error)?;
                body["createdAt"] = json!(created);
                Ok(body)
            })
            .collect::<AppResult<Vec<_>>>()?;
        let pending: Vec<_> = events
            .iter()
            .filter(|e| e["kind"] == "container")
            .filter_map(|e| e["data"]["name"].as_str())
            .filter(|name| {
                !events.iter().any(|e| {
                    (e["kind"] == "cleanup"
                        && e["data"]["name"] == *name
                        && e["data"]["clean"] == true)
                        || (e["kind"] == "score"
                            && e["data"]["score"]["details"]["ownedContainers"]
                                .as_array()
                                .is_some_and(|a| {
                                    a.iter().any(|r| r["name"] == *name && r["removed"] == true)
                                }))
                })
            })
            .collect();
        Ok(
            json!({"id":id,"plan":plan,"planHash":digest,"status":status,"createdAt":created,"finishedAt":finished,"error":problem,"pendingContainers":pending,"events":events,"taskExecution":"Not Run"}),
        )
    }
    pub(in crate::benchmark::harness) fn workspace_event(
        &self,
        workspace: &str,
        id: &str,
        kind: &str,
        data: Value,
    ) -> AppResult<()> {
        self.workspace_record(workspace, id)?;
        let body = json!({"kind":kind,"data":data});
        let raw = body.to_string();
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
    pub(in crate::benchmark::harness) fn workspace_finish(
        &self,
        workspace: &str,
        id: &str,
        status: &str,
        problem: Option<String>,
    ) -> AppResult<()> {
        let record = self.workspace_record(workspace, id)?;
        if ["closed", "completed", "failed", "cancelled"].contains(&status)
            && record["pendingContainers"]
                .as_array()
                .is_none_or(|a| !a.is_empty())
        {
            return Err("副本仍有待清理容器，不能标记为已结束。".into());
        }
        self.connection()?.execute("UPDATE benchmark_generation_jobs SET status=?1,error=COALESCE(?2,error),finished_at=?3 WHERE workspace_id=?4 AND id=?5 AND json_extract(plan_json,'$.purpose')=?6",params![status,problem,if status=="ready"{None}else{Some(now())},workspace,id,PURPOSE]).map_err(error)?;
        Ok(())
    }
    pub(in crate::benchmark::harness) fn begin_workspace_submit(
        &self,
        workspace: &str,
        input: Submit,
    ) -> AppResult<(String, String)> {
        let record = self.workspace_record(workspace, &input.job_id)?;
        if !input.confirmed
            || input.plan_hash != record["planHash"]
            || record["status"] != "ready"
            || !record["plan"]["expiresAt"]
                .as_str()
                .is_some_and(|value| value > now().as_str())
            || input.code.trim().is_empty()
            || input.code.len() > 24000
            || record["plan"]["bundle"]["originalCode"] == input.code
        {
            return Err(
                "请确认仍可编辑的副本及新修改；代码须非空、不超过24KB，且与原文不同。".into(),
            );
        }
        self.preflight_task(workspace, &task_input(&record["plan"])?)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(error)?;
        let changed=transaction.execute("UPDATE benchmark_generation_jobs SET status='grading' WHERE workspace_id=?1 AND id=?2 AND status='ready' AND plan_hash=?3 AND json_extract(plan_json,'$.purpose')=?4",params![workspace,input.job_id,input.plan_hash,PURPOSE]).map_err(error)?;
        if changed != 1 {
            return Err("副本状态已变化，请重新读取。".into());
        }
        let body=json!({"kind":"submission","data":{"code":input.code,"codeHash":hash(input.code.as_bytes()),"provenance":"manual_handoff","confirmed":true}}).to_string();
        transaction
            .execute(
                "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
                params![
                    input.job_id,
                    new_id(),
                    body,
                    json!({"contentHash":hash(body.as_bytes())}).to_string(),
                    now()
                ],
            )
            .map_err(error)?;
        transaction.commit().map_err(error)?;
        Ok((input.job_id, input.code))
    }
    pub(in crate::benchmark::harness) fn workspace_state(
        &self,
        workspace: &str,
        assessment: u32,
        check_id: &str,
        id: Option<&str>,
    ) -> AppResult<Value> {
        self.harness_preflight(workspace, assessment, Some(check_id))?;
        let c = self.connection()?;
        let mut q=c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.checkId')=?3 ORDER BY created_at DESC,rowid DESC LIMIT 50").map_err(error)?;
        let history=q.query_map(params![workspace,PURPOSE,check_id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|r| r["id"].as_str()))
            .map(|id| self.workspace_record(workspace, id))
            .transpose()?;
        if selected.as_ref().is_some_and(|r| {
            r["plan"]["checkId"] != check_id || r["plan"]["assessmentVersion"] != assessment
        }) {
            return Err("副本不属于当前检查或分析版本。".into());
        }
        let active = self
            .pending_workspace_jobs()?
            .into_iter()
            .find(|(w, _)| w == workspace)
            .map(|(_, id)| self.workspace_record(workspace, &id))
            .transpose()?
            .map(|r| json!({"id":r["id"],"checkId":r["plan"]["checkId"],"status":r["status"]}));
        Ok(json!({"history":history,"selected":selected,"active":active}))
    }
    pub(crate) fn pending_workspace_jobs(&self) -> AppResult<Vec<(String, String)>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT workspace_id,id FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')=?1 AND status IN ('opening','ready','grading','interrupted','cleanup_required') ORDER BY created_at").map_err(error)?;
        q.query_map([PURPOSE], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)
    }
}
