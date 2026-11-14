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
    tasks: Arc<crate::tasks::TaskSlot>,
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
            tasks: Arc::new(crate::tasks::TaskSlot::default()),
            closing: Arc::new(AtomicBool::new(false)),
            close_cleanup: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(crate) fn cancel_personalized_task(&self, workspace: &str, id: &str) -> AppResult<()> {
        self.tasks
            .cancel(id, workspace, crate::tasks::TaskKind::Benchmark)
    }
    pub fn has_active(&self) -> bool {
        self.tasks.has_active()
    }
    pub fn cancel_active(&self) {
        self.tasks.cancel_active();
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
    on_update: Channel<StreamUpdate>,
) -> AppResult<RunSummary> {
    let database = state.database.clone()?;
    let provider = state.provider.clone()?;
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, crate::tasks::TaskKind::Chat)?;
    let settings = blocking(database.clone(), |db| db.settings()).await?;
    let prepare_id = id.clone();
    let model_settings = settings.clone();
    let prepared = blocking(database.clone(), move |db| {
        db.prepare_context_run(
            &prepare_id,
            &workspace_id,
            &conversation_id,
            &content,
            &model_settings,
            &context_selection.unwrap_or_default(),
            false,
        )
    })
    .await?;
    let result = prepared.run.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = crate::generation::execute(
            database,
            provider,
            prepared,
            settings,
            crate::generation::InspectionPolicy::Chat,
            lease.receiver(),
            |u| {
                let _ = on_update.send(u);
            },
        )
        .await;
        drop(lease);
        let _ = on_update.send(outcome.update);
    });
    Ok(result)
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
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| db.save_knowledge(input)).await
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
) -> AppResult<serde_json::Value> {
    blocking(state.database.clone()?, move |db| {
        let settings = db.settings()?;
        Ok(db
            .prepare_context_run(
                &new_id(),
                &workspace_id,
                &conversation_id.unwrap_or_default(),
                &content,
                &settings,
                &context_selection,
                true,
            )?
            .manifest)
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
                        .add_filter("UTF-8 文本", &["txt", "md"])
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
) -> AppResult<String> {
    blocking(state.database.clone()?, move |db| {
        db.save_knowledge_with_evidence(input, Some(source))
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
