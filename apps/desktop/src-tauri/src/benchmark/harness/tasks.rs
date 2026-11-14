//! Read-only project tasks. No process/model calls; execution belongs to the D3 gate.
use super::{extract::Evidence, *};
use crate::benchmark::{
    dataset::{self, Case, Expected, Scoring},
    evaluation::{self, State as ScoreState},
    personalized::cases,
};

const PURPOSE: &str = "harness_tasks";
const GENERATOR: &str = "project-readonly-tasks-1";
pub(crate) mod scenarios;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recipe {
    kind: String,
    source_id: String,
    path: String,
    content_hash: String,
    start: usize,
    end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scenario: Option<Scenario>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scenario {
    kind: String,
    symbols: Vec<String>,
    path: String,
    start: usize,
    end: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    purpose: String,
    generator_version: String,
    assessment_version: u32,
    assessment_hash: String,
    repository_id: Option<String>,
    repository_hash: Option<String>,
    recipes: Vec<Recipe>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInput {
    assessment_version: u32,
    repository_id: Option<String>,
    requirement_ids: Vec<String>,
    locations: Vec<crate::benchmark::personalized::cases::Location>,
    #[serde(default)]
    scenarios: Vec<Scenario>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edit {
    prompt: String,
    rationale: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Task {
    kind: String,
    case: Case,
    rationale: String,
    sources: Vec<Evidence>,
    policy: Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Approval {
    pub(super) id: String,
    pub(super) revision: u32,
    pub(super) approved: bool,
}

pub(super) fn checked_snapshot(
    db: &Database,
    workspace: &str,
    id: &str,
) -> AppResult<RepositorySnapshot> {
    let s = db.repository_snapshot(workspace, id)?;
    if s.files.is_empty()
        || s.files.len() > 2000
        || s.files.iter().any(|f| {
            f.path.is_empty()
                || f.path.contains('\\')
                || f.path.starts_with('/')
                || f.path
                    .split('/')
                    .any(|p| p == ".." || p == "." || p.is_empty())
                || f.path.contains(':')
                || hash(f.content.as_bytes()) != f.content_hash
        })
        || s.files
            .iter()
            .map(|f| &f.path)
            .collect::<BTreeSet<_>>()
            .len()
            != s.files.len()
    {
        return Err("项目快照文件或内容校验失败。".into());
    }
    Ok(s)
}

fn materialize(
    db: &Database,
    workspace: &str,
    recipe: &Recipe,
    id: &str,
    edit: Option<Edit>,
) -> AppResult<(Task, Value)> {
    let s = checked_snapshot(db, workspace, &recipe.source_id)?;
    let f = s
        .files
        .iter()
        .find(|f| f.path == recipe.path)
        .ok_or("任务来源文件不存在。")?;
    if f.content_hash != recipe.content_hash {
        return Err("任务来源内容指纹不一致。".into());
    }
    let lines: Vec<_> = f.content.lines().collect();
    if recipe.start == 0
        || recipe.end < recipe.start
        || recipe.end > lines.len()
        || recipe.end - recipe.start >= 30
    {
        return Err("请选择有效的原文位置，每项最多30行。".into());
    }
    let excerpt = lines[recipe.start - 1..recipe.end].join("\n");
    if excerpt.trim().is_empty() || excerpt.len() > 8000 {
        return Err("任务依据不能为空或超过8KB。".into());
    }
    let evidence = |file: &RepositoryFile, start, end, excerpt: String| Evidence {
        source_id: s.id.clone(),
        path: file.path.clone(),
        content_hash: file.content_hash.clone(),
        start_line: start,
        end_line: end,
        excerpt,
    };
    if let Some(scenario) = &recipe.scenario {
        return scenarios::materialize(
            scenario,
            &s,
            f,
            evidence(f, recipe.start, recipe.end, excerpt),
            id,
            edit,
        );
    }
    let (prompt, answers, sources, allowed_paths) = match recipe.kind.as_str() {
        "requirement_quote" => (
            format!(
                "查看固定资料 {} 的第 {}–{} 行。逐字复述这段项目规范，仅输出原文，不附加解释。保留原文大小写与标点，首尾空白不计。",
                f.path, recipe.start, recipe.end
            ),
            vec![excerpt.trim().to_string()],
            vec![evidence(f, recipe.start, recipe.end, excerpt)],
            vec![f.path.clone()],
        ),
        "locate_excerpt" => {
            if s.profile["kind"] == "harness_document" || f.language == "文档" {
                return Err("代码定位任务请选择实际项目快照中的代码或配置文件。".into());
            }
            let mut sources = vec![];
            // ponytail: bounded snapshot scan; indexing is unnecessary for at most 12 tasks / 20MiB.
            for file in &s.files {
                let file_lines: Vec<_> = file.content.lines().collect();
                for (index, window) in file_lines
                    .windows(recipe.end - recipe.start + 1)
                    .enumerate()
                {
                    if window.join("\n") == excerpt {
                        sources.push(evidence(
                            file,
                            index + 1,
                            index + window.len(),
                            excerpt.clone(),
                        ));
                        break;
                    }
                }
            }
            if sources.is_empty() || sources.len() > 20 {
                return Err("片段没有有效匹配或匹配超过20个文件，请选择更具体的片段。".into());
            }
            let answers = sources.iter().map(|e| e.path.clone()).collect();
            (
                format!(
                    "在固定项目快照中定位包含以下完整连续行的任意一个文件。仅输出从项目根目录起的相对路径，使用 /，不加引号或解释。多个文件包含同一片段时任一正确路径均可。\n\n{excerpt}"
                ),
                answers,
                sources,
                s.files.iter().map(|f| f.path.clone()).collect(),
            )
        }
        _ => return Err("当前只支持规范原文复述和代码片段定位。".into()),
    };
    let edit = edit.unwrap_or(Edit {
        prompt,
        rationale: "根据固定快照中的原文生成；答案来自实际文件，未推断存在Bug。".into(),
    });
    if edit.prompt.trim().is_empty()
        || edit.prompt.len() > 24000
        || edit.rationale.trim().is_empty()
        || edit.rationale.chars().count() > 2000
    {
        return Err("请填写题目与依据；题目最多24KB，依据最多2000字。".into());
    }
    let mut case = Case {
        case_id: id.into(),
        dataset_version: 1,
        category: "zh_knowledge".into(),
        input: dataset::Input {
            prompt: edit.prompt,
            response_schema: None,
            tools: vec![],
        },
        expected: Expected::Aliases { answers },
        scoring: Scoring {
            id: "exact_alias".into(),
            version: 2,
            contract: Some(
                json!({"normalization":"trim_unicode_whitespace","case_sensitive":true,"unicode_folding":false}),
            ),
        },
        difficulty: "easy".into(),
        output_format: "text".into(),
        requirements: vec!["completion".into()],
        fixture: vec![],
        content_hash: String::new(),
    };
    cases::seal(&mut case)?;
    dataset::validate_runtime_case(&case)?;
    let Expected::Aliases { answers } = &case.expected else {
        unreachable!()
    };
    let wrong = format!("[错误对照:{}]", hash(case.case_id.as_bytes()));
    let mut checks = vec![];
    for answer in answers {
        let result = evaluation::evaluate(&case, answer, &[]);
        checks.push(json!({"name":"reference_answer","input":answer,"expected":"Passed","actual":result.state,"passed":result.state==ScoreState::Passed}));
    }
    for (name, text, calls) in [
        ("empty_answer", "", vec![]),
        ("wrong_answer", wrong.as_str(), vec![]),
        (
            "unexpected_tool_call",
            answers[0].as_str(),
            vec![json!({"function":{"name":"unrequested","arguments":{}}})],
        ),
    ] {
        let result = evaluation::evaluate(&case, text, &calls);
        checks.push(json!({"name":name,"input":text,"toolCalls":calls,"expected":"Failed","actual":result.state,"passed":result.state==ScoreState::Failed}));
    }
    let valid = checks.iter().all(|v| v["passed"] == true);
    let validation = json!({"valid":valid,"checks":checks,"evaluatorVersion":evaluation::ENGINE_VERSION,"humanReview":"required","taskExecution":"Not Run","referencePatch":"not_applicable_readonly","warnings":["仅验证原文/路径评分规则；题目表述是否与标准答案一致仍须人工审核。","不是代码修改、调用链理解或Harness性能实测。"]});
    Ok((
        Task {
            kind: recipe.kind.clone(),
            case,
            rationale: edit.rationale,
            sources,
            policy: json!({"readOnly":true,"allowedPaths":allowed_paths,"writePaths":[],"network":"none","tools":"none","terminal":"forbidden","sideEffects":"forbidden","timeoutSeconds":120,"maxOutputBytes":16384,"processExecution":"none","context":"fixed_snapshot_files_only"}),
        },
        validation,
    ))
}

impl Database {
    pub fn harness_task_job(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw, plan_hash, created): (String,String,String) = c.query_row("SELECT plan_json,plan_hash,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')='harness_tasks'",params![workspace,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"任务草案不存在或不属于此工作区。".to_string())?;
        if hash(raw.as_bytes()) != plan_hash {
            return Err("任务计划校验失败。".into());
        }
        let plan: Plan = serde_json::from_str(&raw).map_err(json_error)?;
        if plan.purpose != PURPOSE || plan.generator_version != GENERATOR {
            return Err("任务生成版本不受支持。".into());
        }
        let mut q=c.prepare("SELECT case_id,revision,candidate_json,validation_json FROM benchmark_generated_cases g WHERE job_id=?1 AND revision=(SELECT MAX(revision) FROM benchmark_generated_cases WHERE job_id=g.job_id AND case_id=g.case_id) ORDER BY (SELECT MIN(h.rowid) FROM benchmark_generated_cases h WHERE h.job_id=g.job_id AND h.case_id=g.case_id)").map_err(error)?;
        let items=q.query_map([id],|r|{let a:String=r.get(2)?;let b:String=r.get(3)?;Ok(json!({"id":r.get::<_,String>(0)?,"revision":r.get::<_,u32>(1)?,"task":serde_json::from_str::<Value>(&a).map_err(|_|rusqlite::Error::InvalidQuery)?,"validation":serde_json::from_str::<Value>(&b).map_err(|_|rusqlite::Error::InvalidQuery)?}))}).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        Ok(json!({"id":id,"plan":plan,"planHash":plan_hash,"createdAt":created,"candidates":items}))
    }
    pub fn harness_tasks(&self, workspace: &str, id: Option<&str>) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let history = {
            let mut q=c.prepare("SELECT id,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.purpose')='harness_tasks' ORDER BY created_at DESC,id DESC LIMIT 50").map_err(error)?;
            q.query_map([workspace], |r| {
                Ok(json!({"id":r.get::<_,String>(0)?,"createdAt":r.get::<_,String>(1)?}))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?
        };
        let sets = {
            let mut q=c.prepare("SELECT id,version,content_hash,created_at,manifest_json FROM benchmark_datasets WHERE json_extract(provenance_json,'$.workspaceId')=?1 AND json_extract(provenance_json,'$.purpose')='harness_tasks' ORDER BY created_at DESC,id,version DESC LIMIT 50").map_err(error)?;
            q.query_map([workspace],|r|{let raw:String=r.get(4)?;let m:Value=serde_json::from_str(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?;Ok(json!({"id":r.get::<_,String>(0)?,"version":r.get::<_,u32>(1)?,"contentHash":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?,"count":m["case_count"]}))}).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?
        };
        drop(c);
        let selected = id
            .or_else(|| history.first().and_then(|v| v["id"].as_str()))
            .map(|id| self.harness_task_job(workspace, id))
            .transpose()?;
        Ok(json!({"history":history,"selected":selected,"tasksets":sets}))
    }
    pub fn create_harness_tasks(&self, workspace: &str, input: CreateInput) -> AppResult<Value> {
        if !(1..=12).contains(
            &(input.requirement_ids.len() + input.locations.len() + input.scenarios.len()),
        ) {
            return Err("每次请选择1至12项规范或代码片段。".into());
        }
        let analysis = self.fit_read(workspace, Some(input.assessment_version))?;
        let mut recipes = vec![];
        let mut unique = BTreeSet::new();
        for id in input.requirement_ids {
            let r: Requirement = serde_json::from_value(
                analysis["selected"]["body"]["requirements"]
                    .as_array()
                    .and_then(|a| a.iter().find(|r| r["id"] == id))
                    .ok_or("需求不在该分析版本中。")?
                    .clone(),
            )
            .map_err(json_error)?;
            if !matches!(r.status.as_str(), "confirmed" | "edited") {
                return Err("请先确认项目要求并保存分析版本。".into());
            }
            let e = r
                .evidence
                .ok_or("手动补充但没有原文的要求不能自动生成原文题。")?;
            let recipe = Recipe {
                kind: "requirement_quote".into(),
                source_id: e.source_id,
                path: e.path,
                content_hash: e.content_hash,
                start: e.start_line,
                end: e.end_line,
                scenario: None,
            };
            if !unique.insert(serde_json::to_string(&recipe).map_err(json_error)?) {
                return Err("选择的规范指向同一段原文，请去除重复项。".into());
            }
            recipes.push(recipe);
        }
        let repository = input
            .repository_id
            .as_ref()
            .map(|id| checked_snapshot(self, workspace, id))
            .transpose()?;
        for loc in input.locations {
            let s = repository
                .as_ref()
                .ok_or("代码定位任务需要选择项目快照。")?;
            let file = s
                .files
                .iter()
                .find(|f| f.path == loc.path)
                .ok_or("来源文件不存在。")?;
            let recipe = Recipe {
                kind: "locate_excerpt".into(),
                source_id: s.id.clone(),
                path: loc.path,
                content_hash: file.content_hash.clone(),
                start: loc.start,
                end: loc.end,
                scenario: None,
            };
            if !unique.insert(serde_json::to_string(&recipe).map_err(json_error)?) {
                return Err("代码片段重复。".into());
            }
            recipes.push(recipe);
        }
        for scenario in input.scenarios {
            let snapshot = repository
                .as_ref()
                .ok_or("请选择场景使用的固定项目快照。")?;
            let file = snapshot
                .files
                .iter()
                .find(|f| f.path == scenario.path)
                .ok_or("场景来源文件不存在。")?;
            let recipe = Recipe {
                kind: scenario.kind.clone(),
                source_id: snapshot.id.clone(),
                path: scenario.path.clone(),
                content_hash: file.content_hash.clone(),
                start: scenario.start,
                end: scenario.end,
                scenario: Some(scenario),
            };
            if !unique.insert(serde_json::to_string(&recipe).map_err(json_error)?) {
                return Err("场景重复。".into());
            }
            recipes.push(recipe);
        }
        let plan = Plan {
            purpose: PURPOSE.into(),
            generator_version: GENERATOR.into(),
            assessment_version: input.assessment_version,
            assessment_hash: analysis["selected"]["contentHash"]
                .as_str()
                .ok_or("分析版本缺失。")?
                .into(),
            repository_id: repository.as_ref().map(|s| s.id.clone()),
            repository_hash: repository.as_ref().map(|s| s.content_hash.clone()),
            recipes,
        };
        let tasks = plan
            .recipes
            .iter()
            .map(|r| {
                let id = format!("custom-{}", new_id());
                materialize(self, workspace, r, &id, None).map(|(t, v)| (id, t, v))
            })
            .collect::<AppResult<Vec<_>>>()?;
        if tasks
            .iter()
            .map(|(_, task, _)| &task.case.input.prompt)
            .collect::<BTreeSet<_>>()
            .len()
            != tasks.len()
        {
            return Err("所选片段产生了重复任务，请移除重复来源后再生成。".into());
        }
        let raw = serde_json::to_string(&plan).map_err(json_error)?;
        let id = new_id();
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at,finished_at) VALUES(?1,?2,'review',?3,?4,?5,?5)",params![id,workspace,raw,hash(raw.as_bytes()),now()]).map_err(error)?;
        for (task_id, t, v) in tasks {
            tx.execute(
                "INSERT INTO benchmark_generated_cases VALUES(?1,?2,1,?3,?4,?5)",
                params![
                    id,
                    task_id,
                    serde_json::to_string(&t).map_err(json_error)?,
                    v.to_string(),
                    now()
                ],
            )
            .map_err(error)?;
        }
        tx.commit().map_err(error)?;
        drop(c);
        self.harness_task_job(workspace, &id)
    }
    pub fn revise_harness_task(
        &self,
        workspace: &str,
        job: &str,
        id: &str,
        revision: u32,
        edit: Edit,
    ) -> AppResult<Value> {
        let old = self.harness_task_job(workspace, job)?;
        let items = old["candidates"].as_array().ok_or("任务清单缺失。")?;
        let index = items
            .iter()
            .position(|v| v["id"] == id)
            .ok_or("任务不存在。")?;
        if items[index]["revision"] != revision {
            return Err("任务已被修订，请重新读取后审核。".into());
        }
        let plan: Plan = serde_json::from_value(old["plan"].clone()).map_err(json_error)?;
        // Recipes and candidates retain insertion order; IDs never come from model output.
        let recipe = plan.recipes.get(index).ok_or("任务来源缺失。")?;
        let (task, validation) = materialize(self, workspace, recipe, id, Some(edit))?;
        let next = revision.checked_add(1).ok_or("修订数量超出限制。")?;
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let current:u32=tx.query_row("SELECT MAX(revision) FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?2",params![job,id],|r|r.get(0)).map_err(error)?;
        if current != revision {
            return Err("任务已被其他窗口修改，当前草稿已保留。".into());
        }
        tx.execute(
            "INSERT INTO benchmark_generated_cases VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                job,
                id,
                next,
                serde_json::to_string(&task).map_err(json_error)?,
                validation.to_string(),
                now()
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        drop(c);
        self.harness_task_job(workspace, job)
    }
    pub fn publish_harness_tasks(
        &self,
        workspace: &str,
        job: &str,
        selection: Vec<Approval>,
        reviewer: &str,
        expected_version: u32,
    ) -> AppResult<Value> {
        if selection.is_empty()
            || selection.len() > 12
            || reviewer.trim().is_empty()
            || reviewer.chars().count() > 100
        {
            return Err("请选择已审核的任务并填写审核人（最多100字）。".into());
        }
        let data = self.harness_task_job(workspace, job)?;
        let plan: Plan = serde_json::from_value(data["plan"].clone()).map_err(json_error)?;
        if self.fit_read(workspace, Some(plan.assessment_version))?["selected"]["contentHash"]
            != plan.assessment_hash
        {
            return Err("分析版本校验失败。".into());
        }
        if let Some(id) = &plan.repository_id {
            if checked_snapshot(self, workspace, id)?.content_hash
                != *plan.repository_hash.as_ref().ok_or("仓库指纹缺失。")?
            {
                return Err("项目快照已变化。".into());
            }
        }
        let items = data["candidates"].as_array().ok_or("任务清单缺失。")?;
        let mut approved = vec![];
        let mut unique = BTreeSet::new();
        let mut prompts = BTreeSet::new();
        for s in &selection {
            let index = items
                .iter()
                .position(|v| v["id"] == s.id)
                .ok_or("任务不存在。")?;
            let item = &items[index];
            if !s.approved || !unique.insert(&s.id) || item["revision"] != s.revision {
                return Err("只能发布当前修订且已人工批准的任务。".into());
            }
            let old: Task = serde_json::from_value(item["task"].clone()).map_err(json_error)?;
            let (task, check) = materialize(
                self,
                workspace,
                plan.recipes.get(index).ok_or("任务来源缺失。")?,
                &s.id,
                Some(Edit {
                    prompt: old.case.input.prompt,
                    rationale: old.rationale,
                }),
            )?;
            if check["valid"] != true
                || serde_json::to_value(&task).map_err(json_error)? != item["task"]
                || !prompts.insert(task.case.input.prompt.clone())
            {
                return Err("任务依据或评分规则检查失败，请重新修订。".into());
            }
            approved.push(json!({"id":s.id,"revision":s.revision,"task":task,"validation":check}));
        }
        let id = format!("harness-{job}");
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let current: u32 = tx
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM benchmark_datasets WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if current != expected_version {
            return Err("任务集已有新发布版本，请重新读取后确认。".into());
        }
        for s in &selection {
            let rev:u32=tx.query_row("SELECT MAX(revision) FROM benchmark_generated_cases WHERE job_id=?1 AND case_id=?2",params![job,s.id],|r|r.get(0)).map_err(error)?;
            if rev != s.revision {
                return Err("任务已修订，请重新批准。".into());
            }
        }
        let version = current.checked_add(1).ok_or("版本数量超出限制。")?;
        for item in &mut approved {
            let mut task: Task =
                serde_json::from_value(item["task"].clone()).map_err(json_error)?;
            task.case.dataset_version = version;
            cases::seal(&mut task.case)?;
            item["task"] = serde_json::to_value(task).map_err(json_error)?;
        }
        let review = new_id();
        let content_hash = hash(
            json!({"plan":plan,"tasks":approved,"evaluatorVersion":evaluation::ENGINE_VERSION})
                .to_string()
                .as_bytes(),
        );
        let manifest = json!({"name":"项目只读任务集","level":"harness_tasks","version":version,"case_count":approved.len(),"content_hash":content_hash,"review_status":"human_approved","execution":"Not Run","plan":plan,"tasks":approved,"evaluatorVersion":evaluation::ENGINE_VERSION,"provenance":{"purpose":PURPOSE,"workspaceId":workspace,"generationJobId":job,"generationPlanHash":data["planHash"],"reviewId":review}});
        tx.execute(
            "INSERT INTO benchmark_datasets VALUES(?1,?2,'项目只读任务集',2,?3,?4,?5,?6)",
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
        tx.execute(
            "INSERT INTO benchmark_dataset_reviews VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                review,
                workspace,
                job,
                serde_json::to_string(
                    &selection
                        .iter()
                        .map(|s| json!({"id":s.id,"revision":s.revision,"approved":true}))
                        .collect::<Vec<_>>()
                )
                .map_err(json_error)?,
                id,
                version,
                reviewer.trim(),
                now()
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(manifest)
    }
    pub fn harness_taskset(&self, workspace: &str, id: &str, version: u32) -> AppResult<Value> {
        let c = self.connection()?;
        require_workspace(&c, workspace)?;
        let (raw,stored):(String,String)=c.query_row("SELECT manifest_json,content_hash FROM benchmark_datasets WHERE id=?1 AND version=?2 AND json_extract(provenance_json,'$.workspaceId')=?3 AND json_extract(provenance_json,'$.purpose')='harness_tasks'",params![id,version,workspace],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"任务集不存在或不属于当前工作区。".to_string())?;
        let m: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if hash(
            json!({"plan":m["plan"],"tasks":m["tasks"],"evaluatorVersion":m["evaluatorVersion"]})
                .to_string()
                .as_bytes(),
        ) != stored
            || m["content_hash"] != stored
        {
            return Err("任务集内容校验失败。".into());
        }
        Ok(m)
    }
}

#[tauri::command]
pub async fn get_harness_tasks(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.harness_tasks(&workspace_id, job_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn create_harness_tasks(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    input: CreateInput,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.create_harness_tasks(&workspace_id, input)
    })
    .await
}
#[tauri::command]
pub async fn revise_harness_task(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    job_id: String,
    task_id: String,
    expected_revision: u32,
    edit: Edit,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.revise_harness_task(&workspace_id, &job_id, &task_id, expected_revision, edit)
    })
    .await
}
#[tauri::command]
pub async fn publish_harness_tasks(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    job_id: String,
    selection: Vec<Approval>,
    reviewer: String,
    expected_version: u32,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.publish_harness_tasks(
            &workspace_id,
            &job_id,
            selection,
            &reviewer,
            expected_version,
        )
    })
    .await
}
#[tauri::command]
pub async fn get_harness_taskset(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    dataset_id: String,
    version: u32,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.harness_taskset(&workspace_id, &dataset_id, version)
    })
    .await
}
#[tauri::command]
pub async fn get_harness_task_project(
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    snapshot_id: String,
) -> AppResult<RepositorySnapshot> {
    blocking(state.database.clone()?, move |db| {
        checked_snapshot(db, &workspace_id, &snapshot_id)
    })
    .await
}
#[tauri::command]
pub async fn import_harness_repository(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    workspace_id: String,
    kind: String,
) -> AppResult<Option<RepositorySnapshot>> {
    let db = state.database.clone()?;
    {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
    }
    let snapshot = crate::benchmark::personalized::repository::pick_repository(app, &kind).await?;
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    blocking(db, move |db| {
        db.save_repository_snapshot(&workspace_id, snapshot)
    })
    .await
    .map(Some)
}

#[cfg(test)]
mod tests;
