use super::*;
use crate::{
    benchmark::resources::Sampler,
    generation,
    models::ContextSelection,
    retrieval,
    runtime_profiles::{ChatRuntime, CloudChat, CloudTargets, ranking},
    runtime_usage::Meter,
};
use std::time::{Duration, Instant};
use tokio::sync::watch;

pub(super) struct Corpus {
    pub database: Arc<Database>,
    pub grants: Vec<ContextSelection>,
}

impl Corpus {
    pub fn open(
        suite: &Suite,
        date: &str,
        targets: &CloudTargets,
        local_preferences: crate::local_runtime::Preferences,
    ) -> AppResult<Self> {
        let database = Arc::new(suite.database_at("", false, date)?);
        let grants = {
            let c = database.connection()?;
            local_preferences.save(&c)?;
            let grants = c.prepare("SELECT node_id,version FROM rag_current_knowledge WHERE workspace_id=?1 ORDER BY node_id").map_err(error)?
                .query_map([WORKSPACE], |r| Ok(ContextSelection {node_id:r.get(0)?,version:r.get(1)?})).map_err(error)?
                .collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
            for target in [&targets.generation, &targets.embedding, &targets.reranking]
                .into_iter()
                .flatten()
            {
                c.execute(
                    "INSERT OR IGNORE INTO model_cloud_connections VALUES(?1,?2,?3)",
                    params![
                        WORKSPACE,
                        target.connection.id,
                        json!(target.connection).to_string()
                    ],
                )
                .map_err(error)?;
                c.execute("INSERT OR IGNORE INTO model_cloud_knowledge
                    SELECT workspace_id,?2,node_id,version,?3 FROM rag_current_knowledge WHERE workspace_id=?1",
                    params![WORKSPACE,target.connection.id,now()]).map_err(error)?;
            }
            grants
        };
        Ok(Self { database, grants })
    }
}

pub(super) async fn index(
    corpus: &Corpus,
    arm: &Arm,
    targets: &CloudTargets,
    provider: Option<&Ollama>,
    meter: &Meter,
    cancel: watch::Receiver<bool>,
) -> AppResult<Value> {
    let timeout = Duration::from_secs(arm.settings.timeout_seconds);
    if let Some(target) = &targets.embedding {
        crate::rag::cloud::build_metered(
            corpus.database.clone(),
            WORKSPACE.into(),
            target,
            corpus.grants.clone(),
            cancel,
            timeout,
            meter,
        )
        .await
    } else if let Some((model, digest)) = &arm.local_embedding {
        let provider = provider.ok_or("本地 Ollama 服务不可用。")?;
        let status = crate::rag::build_local_index(
            corpus.database.clone(),
            WORKSPACE.into(),
            provider,
            crate::rag::Build {
                model: model.clone(),
                digest: digest.clone(),
                confirmed: true,
            },
            cancel,
            meter,
        )
        .await?;
        corpus
            .database
            .connection()?
            .execute(
                "UPDATE rag_workspace_settings SET enabled=1 WHERE workspace_id=?1",
                [WORKSPACE],
            )
            .map_err(error)?;
        Ok(status["index"].clone())
    } else {
        Ok(Value::Null)
    }
}

pub(super) async fn case(
    corpus: &Corpus,
    arm: &Arm,
    targets: &CloudTargets,
    provider: Option<&Ollama>,
    case: &Case,
    date: &str,
    run_id: &str,
    meter: &Meter,
    mut cancel: watch::Receiver<bool>,
) -> AppResult<Value> {
    let clock = Instant::now();
    let mut sampler = Sampler::new();
    let before = arm.uses_local().then(|| sampler.system());
    let question = case.question.as_str();
    let db = &corpus.database;
    let conversation = db.create_conversation(WORKSPACE)?.id;
    let enabled = arm.policy != Policy::Off;
    db.set_conversation_retrieval(WORKSPACE, &conversation, enabled)?;
    let scope = (arm.policy == Policy::VersionAware)
        .then(|| knowledge_scope::Request::FactDate { date: date.into() });
    let graph = if arm.policy == Policy::Graph {
        GraphMode::Always
    } else {
        GraphMode::Disabled
    };
    let timeout = Duration::from_secs(arm.settings.timeout_seconds);
    let chat = || {
        targets.generation.as_ref().map(|target| CloudChat {
            target,
            share_selection: false,
            granted_versions: &corpus.grants,
            public_fixture: true,
        })
    };
    let route = {
        let c = db.connection()?;
        retrieval::route(
            &c,
            WORKSPACE,
            question,
            enabled,
            scope.is_some(),
            graph,
            false,
        )?
        .with_runtime(&c, WORKSPACE, Some(&arm.profile))?
    };
    let semantic = if let Some(profile) = route.profile.clone() {
        let query = async {
            if let Some(target) = &targets.embedding {
                Ok(crate::rag::cloud::query(
                    target,
                    profile,
                    question.into(),
                    now(),
                    timeout,
                    meter,
                )
                .await)
            } else {
                let provider = provider.ok_or("本地 Ollama 服务不可用。")?;
                Ok::<_, String>(
                    crate::rag::query(provider, profile, question.into(), now(), meter).await,
                )
            }
        };
        Some(tokio::select! {
            biased;
            _ = cancel.wait_for(|v|*v) => return Err("已停止在查找阶段。".into()),
            query = query => query?,
        })
    } else {
        None
    };
    let ranking = if let Some(target) = &targets.reranking {
        let inputs = {
            let c = db.connection()?;
            let runtime = ChatRuntime {
                profile: &arm.profile,
                cloud: chat(),
                ranking: None,
            };
            let candidates = crate::db::chat_retrieval(
                &c,
                WORKSPACE,
                question,
                &route,
                route.semantic(semantic.as_ref()),
                None,
                Some(&runtime),
            )?;
            ranking::permitted_inputs(&c, WORKSPACE, target, &candidates, &corpus.grants)?
        };
        Some(tokio::select! {
            biased;
            _ = cancel.wait_for(|v|*v) => return Err("已停止在重排阶段。".into()),
            ranking = ranking::rerank(target,question,inputs,timeout,meter) => ranking,
        })
    } else {
        None
    };
    if *cancel.borrow() {
        return Err("已停止，未发送回答请求。".into());
    }
    if let Some(expected) = &arm.local_generation {
        let tags = provider
            .ok_or("本地 Ollama 服务不可用。")?
            .get("/api/tags")
            .await?;
        if !tags["models"].as_array().into_iter().flatten().any(|row| {
            row["name"] == arm.settings.model
                && row["digest"]
                    .as_str()
                    .is_some_and(|digest| digest.trim_start_matches("sha256:") == expected)
        }) {
            return Err("回答模型的版本已变化，请重新检查实验计划。".into());
        }
    }
    let runtime = ChatRuntime {
        profile: &arm.profile,
        cloud: chat(),
        ranking: ranking.as_ref(),
    };
    let prepared = db.prepare_chat_runtime(
        run_id,
        WORKSPACE,
        &conversation,
        question,
        &arm.settings,
        &[],
        false,
        semantic.as_ref(),
        scope.as_ref(),
        graph,
        Some(&runtime),
        Some(&json!({"reason":"fixed_experiment_arm","backup":null})),
        false,
    )?;
    let completion = generation::execute_rag_case(
        db.clone(),
        provider,
        targets.generation.as_ref(),
        prepared,
        &arm.settings,
        meter,
        cancel,
    )
    .await;
    let elapsed = clock.elapsed().as_millis() as u64;
    let resources = if let Some(before) = before {
        let after = sampler.system();
        json!({"scope":"whole_system_and_gpu","before":before,"after":after,"gpuAfter":sampler.gpu().await})
    } else {
        Value::Null
    };
    let detail = db.run_detail(WORKSPACE, run_id)?;
    let response = (completion.saved && completion.update.status == "completed")
        .then_some(completion.update.content.as_str());
    let status = match (completion.saved, completion.update.status.as_str()) {
        (true, "completed") => "completed",
        (true, "cancelled") => "cancelled",
        _ => "execution_error",
    };
    let mut result = match score::grade(case, &detail.context_manifest, response) {
        Ok(mut result) => {
            result["status"] = json!(status);
            result["error"] = json!(completion.update.error_message);
            result
        }
        Err(error) => {
            json!({"id":case.id,"question":case.question,"status":"scoring_error","error":error})
        }
    };
    result["answer"] = json!(completion.update.content);
    result["execution"] = json!(detail);
    result["elapsedMs"] = json!(elapsed);
    result["resources"] = resources;
    result["metrics"]["endToEndMs"] = json!(elapsed);
    Ok(result)
}
