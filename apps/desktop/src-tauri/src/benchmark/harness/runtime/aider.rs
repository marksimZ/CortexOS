//! Fixed task binding; shared runtime storage, cancellation and cleanup.
use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiderInput {
    assessment_version: u32,
    dataset_id: String,
    version: u32,
    dataset_hash: String,
    image_id: String,
    confirmed: bool,
}
#[tauri::command]
pub async fn get_harness_aider_images(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        require_workspace(&*db.connection()?, &workspace_id)?;
        python::aider::images()
    })
    .await
}
#[tauri::command]
pub async fn get_harness_aider_runtime(
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
            Kind::Aider,
        )
    })
    .await
}
#[tauri::command]
pub async fn check_harness_aider_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    input: AiderInput,
) -> AppResult<Value> {
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let w = workspace_id.clone();
    let job = id.clone();
    let cancel = lease.receiver();
    blocking(db.clone(),move|db|{
        if !input.confirmed || !python::valid_image(&input.image_id){return Err("请确认已发布任务、固定镜像和无模型检查范围。".into());}
        let task=db.preflight_task(&w,&preflight::CheckInput{assessment_version:input.assessment_version,dataset_id:input.dataset_id.clone(),version:input.version,dataset_hash:input.dataset_hash.clone()})?;
        let env=python::aider::environment_for(&input.image_id,&cancel)?;
        let plan=json!({"purpose":Kind::Aider.purpose(),"version":"aider-runtime-check-1","owner":job,"workspaceId":w,"assessmentVersion":input.assessment_version,"datasetId":input.dataset_id,"datasetVersion":input.version,"datasetHash":input.dataset_hash,"function":task["payload"]["task"]["body"]["spec"]["function"],"repositoryHash":task["payload"]["task"]["body"]["repositoryHash"],"definition":python::aider::definition(),"imageId":input.image_id,"environment":env,"containerName":format!("cortexos-bench-{}",new_id()),"securityPolicy":python::aider::security_policy(),"modelCalls":0,"taskExecution":"Not Run","confirmed":true,"createdAt":now()});
        db.insert_runtime_record(&w,&job,Kind::Aider,plan)
    }).await?;
    let result = json!({"id":id});
    spawn_check(db, workspace_id, id, Kind::Aider, lease);
    Ok(result)
}
#[tauri::command]
pub async fn cleanup_harness_aider_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
) -> AppResult<Value> {
    cleanup_command(state, workspace_id, job_id, Kind::Aider).await
}
