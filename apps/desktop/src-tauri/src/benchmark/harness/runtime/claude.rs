//! Claude-only task binding; common runtime records, cleanup and worker stay shared.
use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeInput {
    assessment_version: u32,
    dataset_id: String,
    version: u32,
    dataset_hash: String,
    image_id: String,
    confirmed: bool,
    #[serde(default)]
    security_policy: Option<String>,
}
impl Database {
    fn claude_task(&self, w: &str, i: &ClaudeInput) -> AppResult<Value> {
        python::claude_policy::describe(
            i.security_policy
                .as_deref()
                .unwrap_or(python::codex_policy::DEFAULT),
        )?;
        if !i.confirmed || !python::valid_image(&i.image_id) {
            return Err("请确认已发布任务、固定镜像及无模型检查范围。".into());
        }
        self.preflight_task(
            w,
            &preflight::CheckInput {
                assessment_version: i.assessment_version,
                dataset_id: i.dataset_id.clone(),
                version: i.version,
                dataset_hash: i.dataset_hash.clone(),
            },
        )
    }
    fn begin_claude_runtime(
        &self,
        w: &str,
        id: &str,
        i: ClaudeInput,
        environment: Value,
    ) -> AppResult<()> {
        let task = self.claude_task(w, &i)?;
        let plan = json!({"purpose":Kind::Claude.purpose(),"version":"claude-runtime-check-2","owner":id,"workspaceId":w,"assessmentVersion":i.assessment_version,"datasetId":i.dataset_id,"datasetVersion":i.version,"datasetHash":i.dataset_hash,"function":task["payload"]["task"]["body"]["spec"]["function"],"repositoryHash":task["payload"]["task"]["body"]["repositoryHash"],"definition":python::claude::definition(),"probeVersion":"claude-program-2","imageId":i.image_id,"environment":environment,"containerName":format!("cortexos-bench-{}",new_id()),"securityPolicy":python::claude_policy::describe(i.security_policy.as_deref().unwrap_or(python::codex_policy::DEFAULT))?,"modelCalls":0,"taskExecution":"Not Run","confirmed":true,"createdAt":now()});
        self.insert_runtime_record(w, id, Kind::Claude, plan)
    }
    pub(super) fn tool_runtime_state(
        &self,
        w: &str,
        assessment: u32,
        id: Option<&str>,
        kind: Kind,
    ) -> AppResult<Value> {
        self.fit_read(w, Some(assessment))?;
        let c = self.connection()?;
        let mut q=c.prepare("SELECT id,version,content_hash,json_extract(manifest_json,'$.payload.task.body.spec.function') FROM benchmark_datasets WHERE json_extract(provenance_json,'$.workspaceId')=?1 AND json_extract(provenance_json,'$.purpose')='harness_programming_tasks' AND json_extract(manifest_json,'$.payload.task.body.origin.assessmentVersion')=?2 ORDER BY created_at DESC,id,version DESC LIMIT 100").map_err(error)?;
        let datasets=q.query_map(params![w,assessment],|r|Ok(json!({"id":r.get::<_,String>(0)?,"version":r.get::<_,u32>(1)?,"contentHash":r.get::<_,String>(2)?,"function":r.get::<_,Option<String>>(3)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        let mut q=c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.assessmentVersion')=?3 ORDER BY created_at DESC,rowid DESC LIMIT 30").map_err(error)?;
        let history=q.query_map(params![w,kind.purpose(),assessment],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(q);
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|h| h["id"].as_str()))
            .map(|id| self.runtime_record_for(w, id, kind))
            .transpose()?;
        if selected
            .as_ref()
            .is_some_and(|r| r["plan"]["assessmentVersion"] != assessment)
        {
            return Err("检查不属于当前分析版本。".into());
        }
        let active=self.pending_runtime_jobs(kind)?.into_iter().find(|(scope,_)|scope==w).map(|(_,id)|self.runtime_record_for(w,&id,kind)).transpose()?.map(|r|json!({"id":r["id"],"assessmentVersion":r["plan"]["assessmentVersion"],"status":r["status"]}));
        Ok(
            json!({"datasets":datasets,"history":history,"selected":selected,"active":active,"canRun":false,"modelCalls":0}),
        )
    }
}
#[tauri::command]
pub async fn get_harness_claude_images(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        require_workspace(&*db.connection()?, &workspace_id)?;
        python::claude::images()
    })
    .await
}
#[tauri::command]
pub async fn get_harness_claude_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.tool_runtime_state(
            &workspace_id,
            assessment_version,
            job_id.as_deref(),
            Kind::Claude,
        )
    })
    .await
}
#[tauri::command]
pub async fn check_harness_claude_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    input: ClaudeInput,
) -> AppResult<Value> {
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let job = id.clone();
    let cancel = lease.receiver();
    blocking(db.clone(), move |db| {
        db.claude_task(&scope, &input)?;
        let env = python::claude::environment_for(
            &input.image_id,
            input
                .security_policy
                .as_deref()
                .unwrap_or(python::codex_policy::DEFAULT),
            &cancel,
        )?;
        db.begin_claude_runtime(&scope, &job, input, env)
    })
    .await?;
    let result = json!({"id":id});
    spawn_check(db, workspace_id, id, Kind::Claude, lease);
    Ok(result)
}
#[tauri::command]
pub async fn cleanup_harness_claude_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
) -> AppResult<Value> {
    cleanup_command(state, workspace_id, job_id, Kind::Claude).await
}
