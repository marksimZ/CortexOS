use crate::{
    db::Database,
    models::*,
    ollama::{ModelHealth, Ollama},
};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{State, ipc::Channel};

#[derive(Clone)]
pub struct AppState {
    pub database: AppResult<Arc<Database>>,
    pub(crate) provider: AppResult<Ollama>,
    pub(crate) runtime_cloud: Arc<tokio::sync::Mutex<()>>,
    pub(crate) tasks: Arc<crate::tasks::TaskSlot>,
    pub(crate) storage_scans: Arc<crate::tasks::TaskSlot>,
    pub(crate) opened_backup: Arc<Mutex<Option<crate::portability::OpenedBackup>>>,
    #[cfg(windows)]
    pub(crate) storage_preview: Arc<Mutex<Option<crate::storage::recovery::PreviewTicket>>>,
    pub closing: Arc<AtomicBool>,
    pub(crate) close_cleanup: Arc<AtomicBool>,
    pending_source: Arc<Mutex<Option<crate::sources::PendingSource>>>,
}

impl AppState {
    pub fn new(directory: &Path) -> Self {
        Self {
            database: Database::open(directory).map(Arc::new),
            pending_source: Arc::new(Mutex::new(None)),
            provider: Ollama::new(),
            runtime_cloud: Arc::new(tokio::sync::Mutex::new(())),
            tasks: Arc::new(crate::tasks::TaskSlot::default()),
            storage_scans: Arc::new(crate::tasks::TaskSlot::default()),
            opened_backup: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            storage_preview: Arc::new(Mutex::new(None)),
            closing: Arc::new(AtomicBool::new(false)),
            close_cleanup: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(crate) fn cancel_personalized_task(&self, workspace: &str, id: &str) -> AppResult<()> {
        self.tasks
            .cancel(id, workspace, crate::tasks::TaskKind::Benchmark)
    }
    pub(crate) async fn index_lease(&self, workspace: &str, request_id: Option<String>, status: Option<Channel<String>>) -> AppResult<crate::tasks::Lease> {
        if let Some(id) = request_id {
            uuid::Uuid::parse_str(&id).map_err(|_| "索引任务编号无效。")?;
            self.tasks.wait_task(&id, workspace, crate::tasks::TaskKind::RagIndex, &self.closing, |message| {
                if let Some(channel) = &status { let _ = channel.send(message.to_owned()); }
            }).await
        } else {
            self.claim_task(&new_id(), workspace, crate::tasks::TaskKind::RagIndex)
        }
    }
    pub(crate) fn cancel_rag_index(&self, workspace: &str, id: Option<&str>) -> AppResult<()> {
        if let Some(id) = id { return self.tasks.cancel(id, workspace, crate::tasks::TaskKind::RagIndex); }
        self.tasks.cancel_kind(workspace, crate::tasks::TaskKind::RagIndex)
    }
    pub fn has_active(&self) -> bool {
        self.tasks.has_active()
    }
    pub(crate) fn has_shutdown_work(&self) -> bool {
        self.has_active() || self.storage_scans.has_active()
    }
    pub fn cancel_active(&self) {
        self.tasks.cancel_active();
        self.storage_scans.cancel_active();
    }
    pub(crate) fn claim_task(
        &self,
        id: &str,
        workspace: &str,
        kind: crate::tasks::TaskKind,
    ) -> AppResult<crate::tasks::Lease> {
        if self.closing.load(Ordering::SeqCst) {
            return Err("正在关闭应用，请等待当前内容保存。".into());
        }
        let lease = self.tasks.claim(id, workspace, kind)?;
        if self.closing.load(Ordering::SeqCst) {
            return Err("正在关闭应用，请等待当前内容保存。".into());
        }
        Ok(lease)
    }
}

pub(crate) async fn blocking<T: Send + 'static>(
    database: Arc<Database>,
    operation: impl FnOnce(&Database) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || operation(&database))
        .await
        .map_err(|_| "本地数据任务意外结束，请保留草稿并重试。".to_string())?
}

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> AppResult<Bootstrap> {
    let mut result = blocking(state.database.clone()?, |db| db.bootstrap()).await?;
    result.active_task = state.tasks.summary()?;
    Ok(result)
}

#[tauri::command]
pub async fn create_workspace(state: State<'_, AppState>, name: String) -> AppResult<Workspace> {
    blocking(state.database.clone()?, move |db| {
        db.create_workspace(&name)
    })
    .await
}

#[tauri::command]
pub async fn list_conversations(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<Conversation>> {
    blocking(state.database.clone()?, move |db| {
        db.conversations(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Conversation> {
    blocking(state.database.clone()?, move |db| {
        db.create_conversation(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn rename_conversation(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: String,
    title: String,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        db.rename_conversation(&workspace_id, &conversation_id, &title)
    })
    .await
}

#[tauri::command]
pub async fn load_messages(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: String,
) -> AppResult<Vec<Message>> {
    blocking(state.database.clone()?, move |db| {
        db.messages(&workspace_id, &conversation_id)
    })
    .await
}

#[tauri::command]
pub async fn check_ollama(state: State<'_, AppState>) -> AppResult<ModelHealth> {
    state
        .provider
        .as_ref()
        .map_err(Clone::clone)?
        .health()
        .await
}

#[tauri::command]
pub async fn list_benchmark_datasets(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    workspace_id: String,
) -> AppResult<Vec<crate::benchmark::dataset::DatasetSummary>> {
    let allowed = cloud.guard().is_ok();
    blocking(state.database.clone()?, move |db| {
        db.benchmark_datasets(&workspace_id).map(|data| {
            data.into_iter()
                .filter(|d| d.id == "cortex-core-60" || allowed)
                .collect()
        })
    })
    .await
}

#[tauri::command]
pub async fn get_benchmark_case(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    workspace_id: String,
    dataset_id: String,
    version: u32,
    case_id: String,
) -> AppResult<crate::benchmark::dataset::Case> {
    if dataset_id != "cortex-core-60" {
        cloud.guard()?;
    }
    blocking(state.database.clone()?, move |db| {
        db.benchmark_case(&workspace_id, &dataset_id, version, &case_id)
    })
    .await
}

#[tauri::command]
pub async fn list_benchmark_models(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<crate::benchmark::preflight::Catalog> {
    blocking(state.database.clone()?, move |db| {
        crate::benchmark::dataset::require_workspace(&*db.connection()?, &workspace_id)
    })
    .await?;
    state
        .provider
        .as_ref()
        .map_err(Clone::clone)?
        .benchmark_models()
        .await
}

#[tauri::command]
pub async fn inspect_benchmark_model(
    state: State<'_, AppState>,
    workspace_id: String,
    request: crate::benchmark::preflight::ModelRequest,
) -> AppResult<crate::benchmark::preflight::Preflight> {
    blocking(state.database.clone()?, move |db| {
        crate::benchmark::dataset::require_workspace(&*db.connection()?, &workspace_id)
    })
    .await?;
    state
        .provider
        .as_ref()
        .map_err(Clone::clone)?
        .benchmark_inspect(request)
        .await
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: ModelSettings,
) -> AppResult<ModelSettings> {
    blocking(state.database.clone()?, move |db| {
        db.save_settings(&settings)
    })
    .await
}

#[tauri::command]
pub async fn start_benchmark(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    workspace_id: String,
    input: crate::benchmark::repository::StartInput,
) -> AppResult<crate::benchmark::repository::Summary> {
    if input.personalized_dataset.is_some() {
        cloud.guard()?;
        if input.model.expected_digest.is_none() {
            return Err("本地评测需要实际模型指纹。".into());
        }
    }
    crate::benchmark::runner::start(state.inner().clone(), workspace_id, input).await
}

#[tauri::command]
pub async fn get_benchmark_run(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    workspace_id: String,
    experiment_id: String,
) -> AppResult<crate::benchmark::repository::Detail> {
    let detail = blocking(state.database.clone()?, move |db| {
        db.benchmark_detail(&workspace_id, &experiment_id)
    })
    .await?;
    if !detail.summary.development_only {
        cloud.guard()?;
    }
    Ok(detail)
}

#[tauri::command]
pub async fn cancel_benchmark(
    state: State<'_, AppState>,
    workspace_id: String,
    experiment_id: String,
) -> AppResult<()> {
    let scope = workspace_id.clone();
    blocking(state.database.clone()?, move |db| {
        crate::benchmark::dataset::require_workspace(&*db.connection()?, &scope)
    })
    .await?;
    state.tasks.cancel(
        &experiment_id,
        &workspace_id,
        crate::tasks::TaskKind::Benchmark,
    )
}

#[tauri::command]
pub async fn get_run(
    state: State<'_, AppState>,
    workspace_id: String,
    run_id: String,
) -> AppResult<RunDetail> {
    blocking(state.database.clone()?, move |db| {
        db.run_detail(&workspace_id, &run_id)
    })
    .await
}

#[tauri::command]
pub async fn get_chat_run_status(
    state: State<'_, AppState>,
    workspace_id: String,
    run_id: String,
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| {
        db.chat_run_status(&workspace_id, &run_id)
    }).await
}

#[tauri::command]
pub async fn cancel_run(
    state: State<'_, AppState>,
    workspace_id: String,
    run_id: String,
) -> AppResult<()> {
    state
        .tasks
        .cancel(&run_id, &workspace_id, crate::tasks::TaskKind::Chat)
}

#[tauri::command]
pub async fn start_chat(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: String,
    content: String,
    context_selection: Option<Vec<ContextSelection>>,
    knowledge_scope: Option<crate::knowledge_scope::Request>,
    use_graph: Option<bool>,
    cloud_consent: Option<crate::runtime_profiles::CloudConsent>,
    use_knowledge_route: Option<bool>,
    generation_target: Option<crate::runtime_profiles::Target>,
    on_preparing: Channel<RunPreparing>,
    on_update: Channel<StreamUpdate>,
) -> AppResult<RunSummary> {
    let database = state.database.clone()?;
    use crate::runtime_profiles::{ChatRuntime, CloudChat, CloudTargets, Target, ranking};
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, crate::tasks::TaskKind::Chat)?;
    let _ = on_preparing.send(RunPreparing { id:id.clone(), workspace_id:workspace_id.clone(), conversation_id:conversation_id.clone() });
    let (ws, chat) = (workspace_id.clone(),conversation_id.clone());
    let manual_selection = generation_target.is_some();
    let manual_cloud_guard = if manual_selection {
        Some(state.runtime_cloud.clone().try_lock_owned().map_err(|_| "正在处理另一项云端设置。")?)
    } else {
        None
    };
    let explicit_context = knowledge_scope.is_some() || use_graph==Some(true)
        || context_selection.as_ref().is_some_and(|items|!items.is_empty());
    let (mut settings, decision) = blocking(database.clone(), move |db| {
        let settings = db.settings()?;
        let c = db.connection()?;
        let knowledge = crate::retrieval::enabled(&c,&ws,&chat)? || explicit_context
            || use_knowledge_route == Some(true);
        let decision = crate::runtime_profiles::routing::select_generation(&c,&ws,knowledge,&settings,generation_target)?;
        Ok((settings,decision))
    }).await?;
    let crate::runtime_profiles::routing::Selection { profile:runtime, decision:model_route, local_backup } = decision;
    let (consent, cloud_guard) = if let Some(profile) = runtime.as_ref().filter(|profile| profile.has_cloud_target()) {
        let sync_default = blocking(database.clone(), {
            let ws = workspace_id.clone();
            move |db| Ok(crate::cloud::privacy::Rules::read(&*db.connection()?, &ws)?.sync_memory_and_history)
        }).await?;
        let consent = if manual_selection || cloud_consent.is_some() {
            profile.cloud_consent(cloud_consent)?
        } else if sync_default {
            crate::runtime_profiles::CloudConsent::implicit(profile)
        } else {
            profile.cloud_consent(None)?
        };
        let guard = match manual_cloud_guard {
            Some(guard) => guard,
            None => state.runtime_cloud.clone().try_lock_owned().map_err(|_| "正在处理另一项云端设置。")?,
        };
        (Some(consent),Some(guard))
    } else {
        drop(manual_cloud_guard);
        if cloud_consent.is_some() { return Err("运行方案已变化，请重新确认发送。".into()); }
        (None,None)
    };
    let workspace = workspace_id.clone();
    let (runtime, targets) = blocking(database.clone(), move |db| {
        let targets = CloudTargets::resolve(&db,&workspace,runtime.as_ref())?;
        Ok((runtime,targets))
    }).await?;
    let provider = if let Some(target) = &targets.generation {
        settings.model = target.model.clone();
        state.provider.clone().ok()
    } else {
        if let Some(Target::Local { model:Some(model) }) = runtime.as_ref().map(|profile| &profile.input.generation) {
            settings.model = model.clone();
        }
        Some(state.provider.clone()?)
    };
    crate::db::chat_budget(&settings, content.trim())?;
    let requested_at = now();
    let workspace = workspace_id.clone();
    let conversation = conversation_id.clone();
    let text = content.trim().to_owned();
    let scoped = knowledge_scope.is_some();
    let force_graph = use_graph.unwrap_or_default();
    let selection = context_selection.unwrap_or_default();
    let manual = !selection.is_empty();
    let cloud_lookup = targets.generation.as_ref().map(|_| {
        consent.as_ref().is_some_and(|v| !v.granted_versions.is_empty())
    });
    let (route, runtime) = blocking(database.clone(), move |db| {
        let c = db.connection()?;
        let sync_all = crate::cloud::privacy::Rules::read(&c, &workspace)?.sync_memory_and_history;
        let conv_enabled = crate::retrieval::enabled(&c, &workspace, &conversation)?;
        let enabled = match cloud_lookup {
            Some(_) if sync_all => conv_enabled,
            Some(explicit) => explicit,
            None => conv_enabled,
        };
        let graph_mode = if cloud_lookup.is_some() && !sync_all {crate::retrieval::GraphMode::Disabled} else {force_graph.into()};
        let route = crate::retrieval::route(&c,&workspace,&text,enabled,scoped && cloud_lookup.is_none(),graph_mode,manual)?
            .with_runtime(&c,&workspace,runtime.as_ref())?;
        Ok((route,runtime))
    }).await?;
    let mut cancel = lease.receiver();
    let timeout = Duration::from_secs(settings.timeout_seconds);
    let meter = crate::runtime_usage::Meter::new(database.clone(),workspace_id.clone(),id.clone(),"chat");
    let semantic = if let Some(index) = route.profile.clone() {
        let question = content.trim().to_owned();
        let query = async {
            if let Some(cloud) = &targets.embedding {
                Some(crate::rag::cloud::query(cloud,index,question,requested_at,timeout,&meter).await)
            } else if let Some(local) = &provider {
                Some(crate::rag::query(local,index,question,requested_at,&meter).await)
            } else { None }
        };
        tokio::select! {
            biased;
            _ = cancel.wait_for(|v| *v) => return Err("已停止查找，问题尚未发送。".into()),
            result = query => result,
        }
    } else { None };
    let ranking = if targets.reranking.is_some() && route.searches() {
        let workspace = workspace_id.clone();
        let text = content.trim().to_owned();
        let (inputs, targets, runtime, consent, semantic, knowledge_scope) = blocking(database.clone(), move |db| {
            let c = db.connection()?;
            let ranked = targets.reranking.as_ref().unwrap();
            let allowed = consent.as_ref().map_or(&[][..],|value|&value.ranking_versions);
            let inputs = if allowed.is_empty() { vec![] } else {
                let cloud = targets.generation.as_ref().map(|target|CloudChat {
                    target, share_selection:false, granted_versions:&consent.as_ref().unwrap().granted_versions, public_fixture:false,
                });
                let scope = if cloud.is_none() {
                    knowledge_scope.as_ref().map(|scope|crate::knowledge_scope::resolve(&c,&workspace,scope)).transpose()?
                } else { None };
                let context = runtime.as_ref().map(|profile|ChatRuntime {profile,cloud,ranking:None});
                let report = crate::db::chat_retrieval(&c,&workspace,&text,&route,route.semantic(semantic.as_ref()),scope.as_ref(),context.as_ref())?;
                ranking::permitted_inputs(&c,&workspace,ranked,&report,allowed)?
            };
            Ok((inputs, targets, runtime, consent, semantic, knowledge_scope))
        }).await?;
        let result = tokio::select! {
            biased;
            _ = cancel.wait_for(|v| *v) => return Err("已停止重排，问题尚未发送给回答模型。".into()),
            result = ranking::rerank(targets.reranking.as_ref().unwrap(),content.trim(),inputs,timeout,&meter) => result,
        };
        (Some(result),targets,runtime,consent,semantic,knowledge_scope)
    } else {
        (None,targets,runtime,consent,semantic,knowledge_scope)
    };
    let (ranking, targets, runtime, consent, semantic, knowledge_scope) = ranking;
    if *cancel.borrow() {
        return Err("已停止准备回答，问题尚未发送给回答模型。".into());
    }
    let prepare_id = id.clone();
    let model_settings = settings.clone();
    let (prepared, cloud) = blocking(database.clone(), move |db| {
        let runtime = runtime.as_ref().map(|profile| ChatRuntime {
            profile,
            cloud: targets.generation.as_ref().map(|target| CloudChat {
                target, share_selection: consent.as_ref().unwrap().share_selection,
                granted_versions:&consent.as_ref().unwrap().granted_versions, public_fixture:false,
            }),
            ranking:ranking.as_ref(),
        });
        let prepared = db.prepare_chat_runtime(
            &prepare_id,
            &workspace_id,
            &conversation_id,
            &content,
            &model_settings,
            &selection,
            false,
            semantic.as_ref(),
            knowledge_scope.as_ref(),
            force_graph,
            runtime.as_ref(),
            Some(&model_route),
            true,
        )?;
        Ok((prepared, targets.generation))
    })
    .await?;
    let result = prepared.run.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = crate::generation::execute_chat(database,provider,cloud,prepared,settings,local_backup,
            lease.receiver(),|u|{let _=on_update.send(u);}).await;
        drop(cloud_guard);
        drop(lease);
        let _ = on_update.send(outcome.update);
    });
    Ok(result)
}

#[tauri::command]
pub async fn list_context_options(
    state: State<'_, AppState>,
    workspace_id: String,
    day: String,
    after: Option<crate::knowledge::ContextOptionCursor>,
) -> AppResult<crate::knowledge::ContextOptionsPage> {
    blocking(state.database.clone()?, move |db| {
        db.context_options(&workspace_id, &day, after.as_ref())
    })
    .await
}

#[tauri::command]
pub async fn list_knowledge(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<crate::knowledge::KnowledgeNode>> {
    blocking(state.database.clone()?, move |db| {
        db.knowledge_list(&workspace_id)
    })
    .await
}
#[tauri::command]
pub async fn get_knowledge(
    state: State<'_, AppState>,
    workspace_id: String,
    node_id: String,
) -> AppResult<crate::knowledge::KnowledgeDetail> {
    blocking(state.database.clone()?, move |db| {
        db.knowledge_detail(&workspace_id, &node_id)
    })
    .await
}
#[tauri::command]
pub async fn save_knowledge(
    state: State<'_, AppState>,
    input: crate::knowledge::SaveKnowledge,
    validity: Option<crate::knowledge_scope::Validity>,
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| match validity {
        Some(value)=>db.save_version(input,None,Some(value)),
        None=>db.save_knowledge(input),
    }).await
}
#[tauri::command]
pub async fn approve_knowledge(
    state: State<'_, AppState>,
    workspace_id: String,
    node_id: String,
    expected_revision: i64,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        db.approve_knowledge(&workspace_id, &node_id, expected_revision)
    })
    .await
}

#[tauri::command]
pub async fn load_knowledge_graph(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<crate::graph::KnowledgeGraph> {
    blocking(state.database.clone()?, move |db| {
        db.knowledge_graph(&workspace_id)
    })
    .await
}
#[tauri::command]
pub async fn save_knowledge_edge(
    state: State<'_, AppState>,
    input: crate::graph::SaveEdge,
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| {
        db.save_knowledge_edge(input)
    })
    .await
}

#[tauri::command]
pub async fn query_graph_nodes(
    state: State<'_, AppState>,
    input: crate::graph_query::NodeQuery,
) -> AppResult<crate::graph_query::NodePage> {
    blocking(state.database.clone()?, move |db| db.graph_node_page(input)).await
}
#[tauri::command]
pub async fn load_graph_node(
    state: State<'_, AppState>,
    workspace_id: String,
    node_id: String,
) -> AppResult<crate::graph_query::GraphNode> {
    blocking(state.database.clone()?, move |db| {
        db.graph_node(&workspace_id, &node_id)
    })
    .await
}
#[tauri::command]
pub async fn load_graph_links(
    state: State<'_, AppState>,
    workspace_id: String,
    node_ids: Vec<String>,
    expected_snapshot: String,
) -> AppResult<Vec<crate::graph::GraphEdge>> {
    blocking(state.database.clone()?, move |db| {
        db.graph_links(&workspace_id, node_ids, &expected_snapshot)
    })
    .await
}
#[tauri::command]
pub async fn query_graph_relations(
    state: State<'_, AppState>,
    workspace_id: String,
    node_id: String,
    offset: i64,
    limit: i64,
    expected_snapshot: Option<String>,
) -> AppResult<crate::graph_query::RelationPage> {
    blocking(state.database.clone()?, move |db| {
        db.graph_relations(
            &workspace_id,
            &node_id,
            offset,
            limit,
            expected_snapshot.as_deref(),
        )
    })
    .await
}
#[tauri::command]
pub async fn load_graph_edge_history(
    state: State<'_, AppState>,
    workspace_id: String,
    edge_id: String,
    before_version: Option<i64>,
    limit: i64,
) -> AppResult<crate::graph_query::HistoryPage> {
    blocking(state.database.clone()?, move |db| {
        db.graph_history(&workspace_id, &edge_id, before_version, limit)
    })
    .await
}
#[tauri::command]
pub async fn save_graph_view(
    state: State<'_, AppState>,
    input: crate::graph::SaveGraphView,
) -> AppResult<i64> {
    blocking(state.database.clone()?, move |db| db.save_graph_view(input)).await
}

#[tauri::command]
pub async fn preview_context(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: Option<String>,
    content: String,
    context_selection: Vec<ContextSelection>,
    knowledge_scope: Option<crate::knowledge_scope::Request>,
    use_graph: Option<bool>,
    generation_target: Option<crate::runtime_profiles::Target>,
) -> AppResult<serde_json::Value> {
    blocking(state.database.clone()?, move |db| {
        use crate::runtime_profiles::{ChatRuntime, CloudChat, Target};
        let mut settings = db.settings()?;
        let decision = {
            let c = db.connection()?;
            let automatic = match conversation_id.as_deref() {
                Some(id) if !id.is_empty() => crate::retrieval::enabled(&c,&workspace_id,id)?,
                _ => true,
            };
            let knowledge = automatic || !context_selection.is_empty() || knowledge_scope.is_some() || use_graph==Some(true);
            crate::runtime_profiles::routing::select_generation(&c,&workspace_id,knowledge,&settings,generation_target)?
        };
        let profile = decision.profile;
        let cloud = match profile.as_ref().map(|item| &item.input.generation) {
            Some(Target::Local { model: Some(model) }) => {
                settings.model = model.clone();
                None
            }
            Some(Target::Cloud { connection_id, model }) => {
                settings.model = model.clone();
                Some(crate::cloud::runtime_connections::resolve_target(&db, &workspace_id, connection_id, model,
                    crate::cloud::runtime_connections::Capability::Generation)?)
            }
            _ => None,
        };
        let runtime = profile.as_ref().map(|profile| ChatRuntime {
            profile,
            cloud: cloud.as_ref().map(|target| CloudChat { target, share_selection: true, granted_versions: &[], public_fixture:false }),
            ranking: None,
        });
        Ok(db.prepare_chat_runtime(
            &new_id(),
            &workspace_id,
            &conversation_id.unwrap_or_default(),
            &content,
            &settings,
            &context_selection,
            true,
            None,
            knowledge_scope.as_ref(),
            use_graph.unwrap_or_default(),
            runtime.as_ref(),
            Some(&decision.decision),
            true,
        )?.manifest)
    })
    .await
}

#[tauri::command]
pub async fn list_runs(
    state: State<'_, AppState>,
    workspace_id: String,
    offset: Option<u32>,
) -> AppResult<Vec<RunSummary>> {
    blocking(state.database.clone()?, move |db| {
        db.list_runs(&workspace_id, offset.unwrap_or(0))
    })
    .await
}
#[tauri::command]
pub async fn load_execution_graph(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    workspace_id: String,
    run_id: String,
) -> AppResult<crate::execution::ExecutionGraph> {
    let purpose = state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .execution_purpose(&workspace_id, &run_id)?;
    if state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .personalized_execution(&workspace_id, &run_id)?
    {
        cloud.guard()?;
    }
    blocking(state.database.clone()?, move |db| {
        if matches!(purpose, ExecutionPurpose::Chat) {
            db.execution_graph(&workspace_id, &run_id)
        } else {
            db.recorded_execution_graph(&workspace_id, &run_id, purpose)
        }
    })
    .await
}
#[tauri::command]
pub async fn save_execution_view(
    state: State<'_, AppState>,
    cloud: State<'_, crate::cloud::CloudState>,
    input: crate::execution::SaveExecutionView,
) -> AppResult<i64> {
    let purpose = state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .execution_purpose(&input.workspace_id, &input.run_id)?;
    if state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .personalized_execution(&input.workspace_id, &input.run_id)?
    {
        cloud.guard()?;
    }
    blocking(state.database.clone()?, move |db| {
        if matches!(purpose, ExecutionPurpose::Chat) {
            db.save_execution_view(input)
        } else {
            db.save_recorded_execution_view(input, purpose)
        }
    })
    .await
}

#[tauri::command]
pub async fn set_conversation_retrieval(
    state: State<'_, AppState>,
    workspace_id: String,
    conversation_id: String,
    enabled: bool,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        db.set_conversation_retrieval(&workspace_id, &conversation_id, enabled)
    })
    .await
}

#[tauri::command]
pub async fn choose_source(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Option<crate::sources::SourcePreview>> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let database = state.database.clone()?;
    let pending = tauri::async_runtime::spawn_blocking(
        move || -> AppResult<Option<crate::sources::PendingSource>> {
            database.source_previous(&workspace_id, "")?;
            let mut chosen = None;
            #[cfg(debug_assertions)]
            if let (Ok(root), Ok(file)) = (
                std::env::var("CORTEXOS_TEST_DATA_DIR"),
                std::env::var("CORTEXOS_TEST_SOURCE_PATH"),
            ) {
                let root =
                    std::fs::canonicalize(root).map_err(|_| "测试数据目录不存在。".to_string())?;
                let selected_path = std::path::PathBuf::from(&file);
                let file =
                    std::fs::canonicalize(file).map_err(|_| "测试文件不存在。".to_string())?;
                if !file.starts_with(&root) {
                    return Err("测试选择文件必须在隔离测试数据目录内。".into());
                }
                chosen = Some(selected_path);
            }
            let chosen = match chosen {
                Some(file) => Some(file),
                None => {
                    let window = app.get_webview_window("main").ok_or("主窗口不存在。")?;
                    app.dialog()
                        .file()
                        .set_parent(&window)
                        .set_title("选择资料文件")
                        .add_filter("文档与代码", crate::sources::SUPPORTED_EXTENSIONS)
                        .blocking_pick_file()
                        .map(|file| file.into_path().map_err(|_| "请选择本地文件。".to_string()))
                        .transpose()?
                }
            };
            chosen
                .map(|path| crate::sources::read_selected(&path, &workspace_id, &database))
                .transpose()
        },
    )
    .await
    .map_err(|_| "文件选择未完成，请重试。".to_string())??;
    let preview = pending.as_ref().map(|p| p.preview.clone());
    *state
        .pending_source
        .lock()
        .map_err(|_| "文件预览忙，请重试。".to_string())? = pending;
    Ok(preview)
}
#[tauri::command]
pub async fn import_source_folder(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Option<crate::sources::FolderSyncReport>> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let database = state.database.clone()?;
    tauri::async_runtime::spawn_blocking(move || -> AppResult<Option<crate::sources::FolderSyncReport>> {
        database.source_previous(&workspace_id, "")?;
        let window = app.get_webview_window("main").ok_or("主窗口不存在。")?;
        let picked = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("选择要同步到当前工作区的本地文件夹")
            .blocking_pick_folder()
            .map(|folder| folder.into_path().map_err(|_| "请选择本地文件夹。".to_string()))
            .transpose()?;
        picked
            .map(|dir| crate::sources::sync_folder(&dir, &workspace_id, &database))
            .transpose()
    })
    .await
    .map_err(|_| "文件夹同步未完成，请重试。".to_string())?
}

#[tauri::command]
pub async fn commit_source(
    state: State<'_, AppState>,
    workspace_id: String,
    ticket: String,
) -> AppResult<crate::sources::Source> {
    let pending = {
        let mut slot = state
            .pending_source
            .lock()
            .map_err(|_| "文件预览忙，请重试。".to_string())?;
        if !slot.as_ref().is_some_and(|p| {
            p.workspace == workspace_id
                && p.preview.ticket == ticket
                && p.created.elapsed() < Duration::from_secs(600)
        }) {
            return Err("文件预览已失效，请重新添加文件。".into());
        }
        slot.take().unwrap()
    };
    let payload = pending.clone();
    match blocking(state.database.clone()?, move |db| db.import_source(payload)).await {
        Ok(saved) => Ok(saved),
        Err(error) => {
            if let Ok(mut slot) = state.pending_source.lock() {
                if slot.is_none() && pending.created.elapsed() < Duration::from_secs(600) {
                    *slot = Some(pending);
                }
            }
            Err(error)
        }
    }
}
#[tauri::command]
pub async fn list_sources(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<crate::sources::Source>> {
    blocking(state.database.clone()?, move |db| db.sources(&workspace_id)).await
}
#[tauri::command]
pub async fn get_source(
    state: State<'_, AppState>,
    workspace_id: String,
    source_id: String,
    version: i64,
) -> AppResult<crate::sources::Snapshot> {
    blocking(state.database.clone()?, move |db| {
        db.source_snapshot(&workspace_id, &source_id, version)
    })
    .await
}
#[tauri::command]
pub async fn revoke_source(
    state: State<'_, AppState>,
    workspace_id: String,
    source_id: String,
    expected_revision: i64,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        db.revoke_source(&workspace_id, &source_id, expected_revision)
    })
    .await
}
#[tauri::command]
pub async fn save_source_knowledge(
    state: State<'_, AppState>,
    input: crate::knowledge::SaveKnowledge,
    source: crate::sources::SourceRef,
    validity: Option<crate::knowledge_scope::Validity>,
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| {
        match validity {
            Some(value)=>db.save_version(input,Some(source),Some(value)),
            None=>db.save_knowledge_with_evidence(input, Some(source)),
        }
    })
    .await
}

#[tauri::command]
pub fn discard_source(
    state: State<'_, AppState>,
    workspace_id: String,
    ticket: String,
) -> AppResult<()> {
    let mut pending = state
        .pending_source
        .lock()
        .map_err(|_| "文件预览忙，请重试。".to_string())?;
    if pending
        .as_ref()
        .is_some_and(|p| p.workspace == workspace_id && p.preview.ticket == ticket)
    {
        *pending = None;
    }
    Ok(())
}
