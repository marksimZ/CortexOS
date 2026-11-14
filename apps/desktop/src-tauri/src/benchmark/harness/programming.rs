//! Immutable authoring; container validation lives in the explicit validation module.
pub(super) mod project;
pub mod validation;
use super::*;
use crate::benchmark::{
    dataset::{self, Assertion, Case, Expected, Scoring},
    personalized::cases,
};

const PURPOSE: &str = "harness_programming_drafts";
const VERSION: &str = "python-task-draft-1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Origin {
    assessment_version: u32,
    repository_id: String,
    path: String,
    start: usize,
    end: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameter {
    name: String,
    kind: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Example {
    label: String,
    role: String,
    args: Vec<Value>,
    output: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    basis: String,
    rationale: String,
    prompt: String,
    function: String,
    parameters: Vec<Parameter>,
    return_type: String,
    examples: Vec<Example>,
    reference_code: String,
    wrong_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project: Option<project::Spec>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveInput {
    job_id: Option<String>,
    expected_revision: u32,
    origin: Origin,
    spec: Spec,
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && ![
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ]
        .contains(&s)
}
fn schema(kind: &str) -> AppResult<Value> {
    Ok(match kind {
        "integer" => {
            json!({"type":"integer","minimum":-9007199254740991_i64,"maximum":9007199254740991_i64})
        }
        "string" => json!({"type":"string","maxLength":4096}),
        "boolean" => json!({"type":"boolean"}),
        "integer_list" | "string_list" | "boolean_list" => {
            json!({"type":"array","maxItems":128,"items":schema(kind.trim_end_matches("_list"))?})
        }
        _ => return Err("类型请选择整数、文本、布尔值或这些类型的列表。".into()),
    })
}

fn prepare(
    db: &Database,
    workspace: &str,
    origin: &Origin,
    spec: &Spec,
    id: &str,
) -> AppResult<Value> {
    let analysis = db.fit_read(workspace, Some(origin.assessment_version))?;
    let project = tasks::checked_snapshot(db, workspace, &origin.repository_id)?;
    let file = project
        .files
        .iter()
        .find(|f| f.path == origin.path)
        .ok_or("目标文件不在固定项目快照中。")?;
    let lines: Vec<_> = file.content.lines().collect();
    if project.profile["kind"] == "harness_document"
        || !file.path.ends_with(".py")
        || origin.start == 0
        || origin.end < origin.start
        || origin.end > lines.len()
        || origin.end - origin.start >= 120
    {
        return Err("请选择项目快照中的Python文件和有效函数范围，最多120行。".into());
    }
    let original_code = lines[origin.start - 1..origin.end].join("\n");
    let project_spec = spec
        .project
        .as_ref()
        .map(|s| project::prepare(s, &project, &file.path))
        .transpose()?;
    if original_code.trim().is_empty() || original_code.len() > 24000 {
        return Err("原函数范围不能为空或超过24KB。".into());
    }
    if !["confirmed_requirement", "reported_bug"].contains(&spec.basis.as_str())
        || spec.rationale.trim().is_empty()
        || spec.rationale.chars().count() > 2000
        || spec.prompt.trim().is_empty()
        || spec.prompt.len() > 16000
        || !identifier(&spec.function)
        || spec.parameters.is_empty()
        || spec.parameters.len() > 5
        || spec.parameters.iter().any(|p| !identifier(&p.name))
        || spec
            .parameters
            .iter()
            .map(|p| &p.name)
            .collect::<BTreeSet<_>>()
            .len()
            != spec.parameters.len()
    {
        return Err("请填写需求依据、明确的题目、有效函数名及1至5个不重复的参数名。".into());
    }
    if project_spec.is_none() {
        for code in [&spec.reference_code, &spec.wrong_code] {
            if code.trim().is_empty() || code.len() > 24000 {
                return Err("参考与错误实现均须填写，每份最多24KB。".into());
            }
        }
        if spec.reference_code.trim() == spec.wrong_code.trim()
            || spec.reference_code.trim() == original_code.trim()
        {
            return Err("参考实现须与错误实现、原函数有区别；实际行为仍需容器验证。".into());
        }
    }
    let mut inputs = BTreeSet::new();
    if !(7..=32).contains(&spec.examples.len())
        || spec.examples.iter().any(|e| {
            e.label.trim().is_empty()
                || e.label.chars().count() > 100
                || !["target", "boundary", "regression"].contains(&e.role.as_str())
                || !inputs.insert(json!(e.args).to_string())
        })
        || ["target", "boundary", "regression"]
            .iter()
            .any(|role| !spec.examples.iter().any(|e| e.role == *role))
    {
        return Err("请填写7至32组不同输入的用例，包含目标行为、边界情况和非目标回归；同一输入只能有一个预期输出。".into());
    }
    let parameter_schemas = spec
        .parameters
        .iter()
        .map(|p| schema(&p.kind))
        .collect::<AppResult<Vec<_>>>()?;
    let signature = format!(
        "{}({})",
        spec.function,
        spec.parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let case_prompt = if project_spec.is_some() {
        format!(
            "{}\n\n项目评分入口：{} 中的 {signature}。根据指定的文件范围完成任务，保持调用接口。",
            spec.prompt, file.path
        )
    } else {
        format!(
            "{}\n\n函数签名：{signature}。仅返回Python纯函数实现，不修改输入，不读写文件或联网。\n\n固定快照中的原函数：\n{original_code}",
            spec.prompt
        )
    };
    let mut case = Case {
        case_id: format!("custom-{id}"),
        dataset_version: 1,
        category: "coding".into(),
        input: dataset::Input {
            prompt: case_prompt,
            response_schema: None,
            tools: vec![],
        },
        expected: Expected::Python {
            function: spec.function.clone(),
            tests: spec
                .examples
                .iter()
                .map(|e| Assertion {
                    args: e.args.clone(),
                    output: e.output.clone(),
                })
                .collect(),
        },
        scoring: Scoring {
            id: "python_assertions".into(),
            version: 2,
            contract: Some(
                json!({"python_version":"3.13","parameter_schemas":parameter_schemas,"return_schema":schema(&spec.return_type)?,"strict_python_types":true,"input_mutation":"forbidden","preconditions":[]}),
            ),
        },
        difficulty: "medium".into(),
        output_format: "python".into(),
        requirements: vec!["completion".into(), "python_sandbox".into()],
        fixture: vec![],
        content_hash: String::new(),
    };
    cases::seal(&mut case)?;
    dataset::validate_runtime_case(&case).map_err(|_| {
        "输入或预期输出不符合所选类型、数量或大小限制；请逐条检查用例。".to_string()
    })?;
    let mut body = json!({"version":VERSION,"origin":origin,"assessmentHash":analysis["selected"]["contentHash"],"repositoryHash":project.content_hash,
        "source":{"sourceId":project.id,"path":file.path,"contentHash":file.content_hash,"startLine":origin.start,"endLine":origin.end,"excerpt":original_code},
        "spec":spec,"case":case,"policy":{"allowedPaths":[file.path],"writePaths":[file.path],"executionUnit":"pure_function_only","repositoryExecution":false,"sandbox":crate::benchmark::python::policy()},
        "validation":{"staticValid":true,"publishable":false,"taskExecution":"Not Run","containerValidation":"Not Run","checks":[{"name":"reference","expected":"all_pass","actual":"Not Run"},{"name":"unchanged","expected":"target_failure_and_regression_pass","actual":"Not Run"},{"name":"wrong","expected":"at_least_one_failure","actual":"Not Run"}],"warnings":["仅检查资料与评分数据结构；未验证Python语法、代码行为、边界覆盖或Bug复现。","原代码、参考实现和错误实现均未执行；完成容器正反对照并人工审核后才能发布。"]}});
    if let Some(project) = project_spec {
        body["policy"]["executionUnit"] = json!("python_project");
        body["policy"]["repositoryExecution"] = json!(true);
        body["policy"]["writePaths"] = project["writePaths"].clone();
        body["policy"]["allowedPaths"] = json!(
            project["original"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
        );
        body["project"] = project;
    }
    if body.to_string().len() > 192000 {
        return Err("草案内容过大，请缩小代码或用例范围。".into());
    }
    Ok(body)
}

impl Database {
    fn programming_job(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw, stored): (String,String) = c.query_row("SELECT plan_json,plan_hash FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')='harness_programming_drafts'",params![workspace,id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"编程草案不存在或不属于当前工作区。".to_string())?;
        if hash(raw.as_bytes()) != stored {
            return Err("编程草案计划校验失败。".into());
        }
        let plan: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if plan["version"] != VERSION {
            return Err("编程草案版本不受支持。".into());
        }
        let mut query = c.prepare("SELECT revision,candidate_json,validation_json,created_at FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?1 ORDER BY revision DESC LIMIT 100").map_err(error)?;
        let revisions = query
            .query_map([id], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        drop(query);
        drop(c);
        let mut items = vec![];
        for (revision, raw, saved, created) in revisions {
            let body: Value = serde_json::from_str(&raw).map_err(json_error)?;
            let validation: Value = serde_json::from_str(&saved).map_err(json_error)?;
            if validation["contentHash"] != hash(raw.as_bytes())
                || validation["publishable"] != false
                || body["origin"] != plan["origin"]
                || body["assessmentHash"] != plan["assessmentHash"]
                || body["repositoryHash"] != plan["repositoryHash"]
            {
                return Err("编程草案内容校验失败，请保留数据。".into());
            }
            items.push(json!({"revision":revision,"body":body,"contentHash":validation["contentHash"],"createdAt":created}));
        }
        if items.is_empty() {
            return Err("编程草案修订记录缺失。".into());
        }
        Ok(json!({"id":id,"plan":plan,"revisions":items}))
    }
    pub fn programming_drafts(&self, workspace: &str, id: Option<&str>) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let mut q = c.prepare("SELECT id,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')='harness_programming_drafts' ORDER BY created_at DESC,id DESC LIMIT 50").map_err(error)?;
        let history = q
            .query_map([workspace], |r| {
                Ok(json!({"id":r.get::<_,String>(0)?,"createdAt":r.get::<_,String>(1)?}))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        drop(q);
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|v| v["id"].as_str()))
            .map(|id| self.programming_job(workspace, id))
            .transpose()?;
        Ok(json!({"history":history,"selected":selected}))
    }
    pub fn save_programming_draft(&self, workspace: &str, input: SaveInput) -> AppResult<Value> {
        let old = input
            .job_id
            .as_deref()
            .map(|id| self.programming_job(workspace, id))
            .transpose()?;
        let revision = old
            .as_ref()
            .and_then(|v| v["revisions"][0]["revision"].as_u64())
            .unwrap_or(0);
        if revision != u64::from(input.expected_revision) || revision >= 100 {
            return Err("草案已更新或达到100次修订，请重新读取或另建草案。".into());
        }
        let id = input.job_id.unwrap_or_else(new_id);
        let body = prepare(self, workspace, &input.origin, &input.spec, &id)?;
        let plan = json!({"purpose":PURPOSE,"version":VERSION,"origin":input.origin,"assessmentHash":body["assessmentHash"],"repositoryHash":body["repositoryHash"]});
        if old.as_ref().is_some_and(|v| v["plan"] != plan) {
            return Err("修订须保持原分析版本与项目范围；更换来源请另建草案。".into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let current: u32 = tx.query_row("SELECT COALESCE(MAX(revision),0) FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?1",[&id],|r|r.get(0)).map_err(error)?;
        if current != input.expected_revision {
            return Err("草案已被修订，请重新读取。".into());
        }
        if old.is_none() {
            let raw = plan.to_string();
            tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at,finished_at) VALUES(?1,?2,'review',?3,?4,?5,?5)",params![id,workspace,raw,hash(raw.as_bytes()),now()]).map_err(error)?;
        }
        let raw = body.to_string();
        tx.execute("INSERT INTO benchmark_generated_cases VALUES(?1,?1,?2,?3,?4,?5)",params![id,current+1,raw,json!({"contentHash":hash(raw.as_bytes()),"publishable":false,"containerValidation":"Not Run"}).to_string(),now()]).map_err(error)?;
        tx.commit().map_err(error)?;
        drop(c);
        self.programming_job(workspace, &id)
    }
}

#[tauri::command]
pub async fn get_harness_programming_drafts(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.programming_drafts(&workspace_id, job_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn save_harness_programming_draft(
    state: State<'_, AppState>,
    workspace_id: String,
    input: SaveInput,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.save_programming_draft(&workspace_id, input)
    })
    .await
}

#[cfg(test)]
mod tests;
