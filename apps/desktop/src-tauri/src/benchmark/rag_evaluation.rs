use crate::{
    benchmark::dataset::{hash, require_workspace},
    commands::{AppState, blocking},
    db::{Database, error},
    knowledge_scope,
    models::{AppResult, ModelSettings, new_id, now},
    retrieval::GraphMode,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

mod fixture;
pub mod runtime;
mod score;
use fixture::{Case, Suite};

const WORKSPACE: &str = "cortex-rag-fixture";
const POLICY_VERSION: &str = "cortex-rag-policy-1";

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Off,
    Fts5,
    Hybrid,
    HybridRerank,
    Graph,
    VersionAware,
}

impl Policy {
    fn semantic(self) -> bool {
        matches!(self, Self::Hybrid | Self::HybridRerank)
    }

    fn run(
        self,
        db: &Database,
        suite: &Suite,
        conversation: &str,
        settings: &ModelSettings,
    ) -> AppResult<Value> {
        db.set_conversation_retrieval(WORKSPACE, conversation, self != Self::Off)?;
        db.connection()?
            .execute(
                "UPDATE rag_workspace_settings SET enabled=?2,rerank=?3 WHERE workspace_id=?1",
                params![WORKSPACE, self.semantic(), self == Self::HybridRerank],
            )
            .map_err(error)?;
        let mut results = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            let query = if self.semantic() {
                let connection = db.connection()?;
                Some(crate::rag::evaluation_query(
                    &connection,
                    WORKSPACE,
                    suite.vector(&case.semantic_hints),
                )?)
            } else {
                None
            };
            let scope = (self == Self::VersionAware).then(|| knowledge_scope::Request::FactDate {
                date: case
                    .fact_date
                    .as_ref()
                    .unwrap_or(&suite.snapshot_date)
                    .clone(),
            });
            let graph = if self == Self::Graph {
                GraphMode::Always
            } else {
                GraphMode::Disabled
            };
            let run = db.prepare_context_run_with_retrieval(
                &new_id(),
                WORKSPACE,
                conversation,
                &case.question,
                settings,
                &[],
                true,
                query.as_ref(),
                scope.as_ref(),
                graph,
            )?;
            results.push(score::evaluate(case, &run.manifest)?);
        }
        Ok(json!({"policy":self,"summary":score::summarize(&results),"cases":results}))
    }
}

fn compare(suite: &Suite, a: Policy, b: Policy) -> AppResult<Value> {
    if a == b {
        return Err("请选择两种不同的查找策略。".into());
    }
    let metadata = suite.metadata();
    let snapshot_hash = metadata["snapshotHash"]
        .as_str()
        .ok_or("固定知识缺少内容指纹。")?;
    let scratch = suite.database(snapshot_hash, a.semantic() || b.semantic())?;
    let conversation = scratch.create_conversation(WORKSPACE)?.id;
    let settings = ModelSettings {
        model: "fixed-replay".into(),
        num_ctx: 8192,
        num_predict: 512,
        temperature: 0.,
        timeout_seconds: 30,
        ..Default::default()
    };
    let result_a = a.run(&scratch, suite, &conversation, &settings)?;
    let result_b = b.run(&scratch, suite, &conversation, &settings)?;
    Ok(
        json!({"id":new_id(),"createdAt":now(),"dataset":metadata,"strategies":[a,b],"settings":settings,
        "mode":"fixed_fixture","generation":"not_run","llmJudge":"not_run","results":[result_a,result_b]}),
    )
}

fn snapshot(db: &Database, workspace: &str, selected: Option<&str>) -> AppResult<Value> {
    let c = db.connection()?;
    require_workspace(&c, workspace)?;
    let mut statement = c.prepare("SELECT id,created_at,json_extract(body_json,'$.strategies'),json_extract(body_json,'$.dataset.version')
        FROM rag_evaluations WHERE workspace_id=?1 ORDER BY created_at DESC,id DESC").map_err(error)?;
    let history = statement
        .query_map([workspace], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
            ))
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    let history: Vec<Value> = history
        .into_iter()
        .map(|(id, date, strategies, version)| {
            let strategies: Value =
                serde_json::from_str(&strategies).map_err(crate::db::json_error)?;
            Ok(json!({"id":id,"createdAt":date,"strategies":strategies,"datasetVersion":version}))
        })
        .collect::<AppResult<_>>()?;
    let id = selected.or_else(|| history.first().and_then(|item| item["id"].as_str()));
    let report = id.map(|id| -> AppResult<Value> {
        let (raw,digest): (String,String) = c.query_row("SELECT body_json,body_hash FROM rag_evaluations WHERE workspace_id=?1 AND id=?2",
            params![workspace,id], |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(error)?
            .ok_or("找不到此工作区的固定检索评测。")?;
        if hash(raw.as_bytes()) != digest { return Err("此检索评测的内容校验失败。".into()); }
        serde_json::from_str(&raw).map_err(crate::db::json_error)
    }).transpose()?;
    Ok(json!({"dataset":Suite::load()?.metadata(),"history":history,"report":report}))
}

fn run_and_save(db: &Database, workspace: &str, a: Policy, b: Policy) -> AppResult<Value> {
    {
        let c = db.connection()?;
        require_workspace(&c, workspace)?;
    }
    let report = compare(&Suite::load()?, a, b)?;
    let raw = report.to_string();
    db.connection()?
        .execute(
            "INSERT INTO rag_evaluations VALUES(?1,?2,?3,?4,?5)",
            params![
                workspace,
                report["id"].as_str(),
                report["createdAt"].as_str(),
                raw,
                hash(raw.as_bytes())
            ],
        )
        .map_err(error)?;
    Ok(report)
}

#[tauri::command]
pub async fn get_rag_evaluation(
    state: State<'_, AppState>,
    workspace_id: String,
    evaluation_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        snapshot(&db, &workspace_id, evaluation_id.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn run_rag_evaluation(
    state: State<'_, AppState>,
    workspace_id: String,
    policy_a: Policy,
    policy_b: Policy,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        run_and_save(&db, &workspace_id, policy_a, policy_b)
    })
    .await
}

#[cfg(test)]
mod tests;
