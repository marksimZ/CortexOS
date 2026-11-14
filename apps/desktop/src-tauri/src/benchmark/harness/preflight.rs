//! D3-0 records readiness observations. It cannot start a Harness task.
use super::*;
use crate::{benchmark::python, tasks::TaskKind};
mod codex;
const PURPOSE: &str = "harness_execution_preflight";
const VERSION: &str = "codex-preflight-1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckInput {
    pub(super) assessment_version: u32,
    pub(super) dataset_id: String,
    pub(super) version: u32,
    pub(super) dataset_hash: String,
}

impl Database {
    pub(super) fn preflight_task(&self, workspace: &str, input: &CheckInput) -> AppResult<Value> {
        let task = self.programming_taskset(workspace, &input.dataset_id, input.version)?;
        if task["content_hash"] != input.dataset_hash
            || task["payload"]["task"]["body"]["origin"]["assessmentVersion"]
                != input.assessment_version
        {
            return Err("任务版本、指纹或分析归属不匹配，请重新选择。".into());
        }
        let payload = &task["payload"];
        if payload["validation"]["publishable"] != true
            || payload["validation"]["evidenceBound"] != true
            || payload["review"]["reviewer"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err("发布任务缺少有效的实际对照或人工审核记录。".into());
        }
        let body = &payload["task"]["body"];
        let repository = tasks::checked_snapshot(
            self,
            workspace,
            body["origin"]["repositoryId"]
                .as_str()
                .ok_or("任务来源缺失。")?,
        )?;
        if body["repositoryHash"] != repository.content_hash
            || self.fit_read(workspace, Some(input.assessment_version))?["selected"]["contentHash"]
                != body["assessmentHash"]
        {
            return Err("任务项目快照或分析依据校验失败。".into());
        }
        if !python::valid_image(
            payload["validation"]["plan"]["imageId"]
                .as_str()
                .unwrap_or(""),
        ) {
            return Err("发布验证未绑定有效镜像。".into());
        }
        Ok(task)
    }
    pub(super) fn save_harness_preflight(
        &self,
        workspace: &str,
        input: &CheckInput,
        cli: Value,
        docker: Value,
        started: &str,
    ) -> AppResult<Value> {
        let task = self.preflight_task(workspace, input)?;
        let id = new_id();
        let checks = json!([
            {"id":"published_task","name":"任务、审核与来源","state":"passed","detail":"已核对已发布版本、指纹、人工审核与固定来源。"},
            {"id":"host_cli","name":"宿主 Codex CLI","state":if cli["state"]=="detected"{"passed"}else{"blocked"},"detail":if cli["state"]=="detected"{"已读取宿主程序版本；不代表隔离环境中可运行。"}else{"未确认原生程序版本，查看环境记录中的原因。"}},
            {"id":"docker_image","name":"Docker 与评分镜像","state":if docker["state"]=="available"{"passed"}else{"blocked"},"detail":if docker["state"]=="available"{"本地Linux引擎与发布时固定的Python镜像可读取；未执行测试代码。"}else{"本地引擎或固定镜像不可用；没有自动下载。"}},
            {"id":"agent_environment","name":"Agent 隔离环境与工作副本","state":"blocked","detail":"尚未建立包含CLI的独立执行环境和可写工作副本。"},
            {"id":"model_access","name":"模型访问与认证","state":"not_checked","detail":"尚未配置受控模型通道；没有读取个人登录或凭据。"},
            {"id":"grading_handoff","name":"补丁与独立评分交接","state":"blocked","detail":"尚未建立Agent补丁与独立评分的交接；参考实现和测试不会交给Agent。"},
            {"id":"budget","name":"运行预算与执行批准","state":"blocked","detail":"尚未绑定真实任务的时间、请求数及费用上限与执行批准。"}
        ]);
        let body = json!({"purpose":PURPOSE,"preflightVersion":VERSION,"workspaceId":workspace,"assessmentVersion":input.assessment_version,"harnessId":"codex","datasetId":input.dataset_id,"datasetVersion":input.version,"datasetHash":input.dataset_hash,"repositoryId":task["payload"]["task"]["body"]["origin"]["repositoryId"],"repositoryHash":task["payload"]["task"]["body"]["repositoryHash"],"validationId":task["payload"]["validation"]["id"],"imageId":task["payload"]["validation"]["plan"]["imageId"],"cli":cli,"docker":docker,"checks":checks,"canRun":false,"taskExecution":"Not Run","startedAt":started,"finishedAt":now(),"observationsAreHistorical":true});
        let raw = body.to_string();
        let digest = hash(raw.as_bytes());
        self.connection()?.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at,finished_at) VALUES(?1,?2,'completed',?3,?4,?5,?6)", params![id,workspace,raw,digest,started,body["finishedAt"].as_str().unwrap()]).map_err(error)?;
        Ok(json!({"id":id,"contentHash":digest,"body":body}))
    }
    pub(super) fn harness_preflight(
        &self,
        workspace: &str,
        assessment: u32,
        id: Option<&str>,
    ) -> AppResult<Value> {
        self.fit_read(workspace, Some(assessment))?;
        let c = self.connection()?;
        let mut q = c.prepare("SELECT id,plan_json,plan_hash FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.assessmentVersion')=?3 ORDER BY created_at DESC,rowid DESC LIMIT 30").map_err(error)?;
        let rows = q
            .query_map(params![workspace, PURPOSE, assessment], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let history = rows
            .iter()
            .map(|(id, raw, digest)| -> AppResult<Value> {
                if hash(raw.as_bytes()) != *digest {
                    return Err("执行检查记录校验失败，请保留数据。".into());
                }
                let body: Value = serde_json::from_str(raw).map_err(json_error)?;
                if body["preflightVersion"] != VERSION
                    || body["workspaceId"] != workspace
                    || body["canRun"] != false
                {
                    return Err("执行检查版本或范围无效。".into());
                }
                Ok(json!({"id":id,"contentHash":digest,"body":body}))
            })
            .collect::<AppResult<Vec<_>>>()?;
        let selected = match id {
            Some(id) => Some(
                history
                    .iter()
                    .find(|r| r["id"] == id)
                    .ok_or("此分析版本中没有该检查记录。")?
                    .clone(),
            ),
            None => history.first().cloned(),
        };
        let mut q = c.prepare("SELECT id,version,content_hash,created_at FROM benchmark_datasets WHERE json_extract(provenance_json,'$.workspaceId')=?1 AND json_extract(provenance_json,'$.purpose')='harness_programming_tasks' AND json_extract(manifest_json,'$.payload.task.body.origin.assessmentVersion')=?2 ORDER BY created_at DESC,id,version DESC LIMIT 100").map_err(error)?;
        let datasets = q.query_map(params![workspace,assessment],|r|Ok(json!({"id":r.get::<_,String>(0)?,"version":r.get::<_,u32>(1)?,"contentHash":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        Ok(json!({"datasets":datasets,"history":history,"selected":selected,"canRun":false}))
    }
}

#[tauri::command]
pub async fn get_harness_execution_preflight(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    check_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.harness_preflight(&workspace_id, assessment_version, check_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn check_harness_execution_preflight(
    state: State<'_, AppState>,
    workspace_id: String,
    input: CheckInput,
) -> AppResult<Value> {
    let lease = state.claim_task(&new_id(), &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let (task, input) = blocking(db.clone(), move |db| {
        Ok((db.preflight_task(&scope, &input)?, input))
    })
    .await?;
    let started = now();
    let cli = codex::observe().await;
    let cancel = lease.receiver();
    if *cancel.borrow() {
        return Err("检查已停止，没有运行编程任务。".into());
    }
    let image = task["payload"]["validation"]["plan"]["imageId"]
        .as_str()
        .unwrap()
        .to_owned();
    let docker = tauri::async_runtime::spawn_blocking(move || {
        match python::harness_environment(&image, Some(&cancel)) {
            Ok(environment) => json!({"state":"available","environment":environment}),
            Err(reason) => json!({"state":"unavailable","reason":reason}),
        }
    })
    .await
    .map_err(|_| "无法读取Docker环境。")?;
    if *lease.receiver().borrow() {
        return Err("检查已停止，没有运行编程任务。".into());
    }
    blocking(db, move |db| {
        db.save_harness_preflight(&workspace_id, &input, cli, docker, &started)
    })
    .await
}
