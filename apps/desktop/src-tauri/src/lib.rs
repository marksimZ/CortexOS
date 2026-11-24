mod benchmark;
mod citations;
mod cloud;
mod commands;
mod db;
mod diagnostics;
mod task_center;
mod execution;
mod generation;
mod graph;
mod graph_query;
mod http_transport;
mod knowledge;
mod knowledge_conflicts;
mod knowledge_graph_retrieval;
mod knowledge_scope;
mod models;
mod model_runtime;
mod runtime_profiles;
mod runtime_usage;
mod local_runtime;
mod memory;
mod ollama;
mod personal_assistant;
mod portability;
mod retrieval;
mod rag;
mod reminders;
mod source_chunks;
mod sources;
mod storage;
mod tasks;
mod web_search;
#[cfg(windows)]
mod updates;

use std::sync::{OnceLock, atomic::Ordering};
use tauri::Manager;

static APP_VERSION: OnceLock<String> = OnceLock::new();

pub(crate) fn app_version() -> &'static str {
    APP_VERSION.get().map(String::as_str).unwrap_or(env!("CARGO_PKG_VERSION"))
}

pub fn run() {
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    let _ = APP_VERSION.set(context.package_info().version.to_string());
    #[cfg(debug_assertions)]
    {
        if let Some(port) = std::env::var("CORTEXOS_TEST_CDP_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|port| *port != 0)
        {
            if let Some(window) = context.config_mut().app.windows.first_mut() {
                // ponytail: preserve wry 0.57 defaults; review these flags when upgrading wry.
                window.additional_browser_args = Some(format!(
                    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
                ));
            }
        }
    }
    #[cfg(windows)]
    let primary_window = context.config_mut().app.windows.iter_mut()
        .find(|window| window.label == "main" && window.create)
        .map(|window| {
            window.create = false;
            window.clone()
        });
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::UpdateState::default());
    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let default_directory = app.path().app_data_dir()?;
            let mut directory = default_directory.clone();
            #[cfg(debug_assertions)]
            if let Ok(value) = std::env::var("CORTEXOS_TEST_DATA_DIR") {
                directory = std::path::PathBuf::from(value);
            }
            let state = commands::AppState::new(&directory);
            app.manage(web_search::WebSearch::new(&default_directory));
            #[cfg(windows)]
            if primary_window.is_some() && state.database.is_ok() {
                storage::recovery::before_webview(app.handle(), &directory);
            }
            app.manage(state);
            app.manage(cloud::CloudState::new());
            #[cfg(windows)]
            if let Some(window) = &primary_window {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window)?.build()?;
            }
            reminders::start_scheduler(app.handle(), app.state::<commands::AppState>().inner());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            #[cfg(windows)]
            updates::update_channels,
            #[cfg(windows)]
            updates::check_update,
            #[cfg(windows)]
            updates::download_update,
            #[cfg(windows)]
            updates::install_update,
            diagnostics::read_diagnostics,
            diagnostics::export_diagnostics,
            task_center::read_live_tasks,
            task_center::read_task_history,
            task_center::cancel_waiting_task,
            reminders::list_reminders,
            reminders::save_reminder,
            reminders::set_reminder_state,
            personal_assistant::read_assistant,
            personal_assistant::save_assistant_draft,
            personal_assistant::publish_assistant,
            personal_assistant::activate_assistant,
            memory::get_global_persona,
            memory::save_global_persona,
            memory::list_memory_candidates,
            memory::resolve_memory_candidate,
            memory::promote_rule_from_chat,
            memory::generate_weekly_digest_draft,
            web_search::read_web_search_status,
            web_search::save_web_search_key,
            web_search::search_web,
            web_search::fetch_web_page,
            portability::backup_preview,
            portability::backup_save,
            portability::backup_open,
            portability::restore::restore_stage,
            portability::restore::restore_cancel,
            portability::restore::restore_on_start,
            portability::restore::restore_ack,
            portability::backup_history,
            portability::knowledge_export::export_knowledge,
            storage::storage_inventory,
            storage::cancel_storage_inventory,
            #[cfg(windows)]
            storage::recovery::storage_recovery_history,
            #[cfg(windows)]
            storage::recovery::storage_recovery_request,
            runtime_usage::report::model_runtime_usage,
            runtime_usage::report::model_runtime_resources,
            cloud::privacy::model_cloud_privacy_rules,
            cloud::privacy::set_knowledge_local_only,
            runtime_profiles::routing::model_chat_routing,
            runtime_profiles::routing::chat_runtime_route,
            runtime_usage::model_runtime_policy,
            rag::cloud::get_cloud_rag_index,
            rag::cloud::build_cloud_rag_index,
            cloud::privacy::list_model_cloud_knowledge,
            cloud::privacy::set_model_cloud_knowledge,
            runtime_profiles::list_model_runtime_profiles,
            runtime_profiles::save_model_runtime_profile,
            runtime_profiles::activate_model_runtime_profile,
            runtime_profiles::delete_model_runtime_profile,
            cloud::runtime_connections::list_runtime_cloud_connections,
            cloud::runtime_connections::bind_runtime_cloud_key,
            cloud::runtime_connections::add_runtime_cloud_connection,
            cloud::runtime_connections::remove_runtime_cloud_connection,
            cloud::runtime_connections::refresh_runtime_cloud_catalog,
            cloud::runtime_connections::probe_runtime_cloud_model,
            cloud::runtime_connections::set_runtime_cloud_model_limits,
            local_runtime::local_model_preferences,
            local_runtime::inspect_local_model_runtime,
            local_runtime::release_local_runtime_model,
            benchmark::rag_evaluation::get_rag_evaluation,
            benchmark::rag_evaluation::run_rag_evaluation,
            benchmark::rag_evaluation::runtime::preview_rag_runtime_experiment,
            benchmark::rag_evaluation::runtime::run_rag_runtime_experiment,
            benchmark::rag_evaluation::runtime::cancel_rag_runtime_experiment,
            benchmark::rag_evaluation::runtime::get_rag_runtime_experiment,
            benchmark::rag_evaluation::runtime::export_rag_runtime_experiment,
            rag::rag_index_status,
            rag::rag_embedding_models,
            rag::rag_build_index,
            rag::rag_cancel_index,
            rag::rag_set_index_options,
            knowledge_scope::list_knowledge_snapshots,
            knowledge_scope::capture_knowledge_snapshot,
            knowledge_scope::list_knowledge_snapshot_members,
            knowledge_scope::create_knowledge_branch,
            knowledge_graph_retrieval::get_knowledge_clusters,
            benchmark::harness::workspace::get_harness_workspace,
            benchmark::harness::runtime::get_harness_codex_images,
            benchmark::harness::runtime::aider::get_harness_aider_images,
            benchmark::harness::runtime::aider::get_harness_aider_runtime,
            benchmark::harness::runtime::aider::check_harness_aider_runtime,
            benchmark::harness::runtime::aider::cleanup_harness_aider_runtime,
            benchmark::harness::runtime::claude::get_harness_claude_images,
            benchmark::harness::runtime::claude::get_harness_claude_runtime,
            benchmark::harness::runtime::claude::check_harness_claude_runtime,
            benchmark::harness::runtime::claude::cleanup_harness_claude_runtime,
            benchmark::harness::runtime::get_harness_codex_runtime,
            benchmark::harness::session::claude::get_harness_claude_session,
            benchmark::harness::session::aider::get_harness_aider_session,
            benchmark::harness::session::experiment::list_harness_experiments,
            benchmark::harness::session::experiment::start_harness_experiment,
            benchmark::harness::session::experiment::stop_harness_experiment,
            benchmark::harness::session::experiment::get_harness_experiment,
            benchmark::harness::session::experiment::get_harness_experiment_trace,
            benchmark::harness::session::experiment::export_harness_experiment,
            benchmark::harness::session::claude::start_harness_claude_session,
            benchmark::harness::session::aider::start_harness_aider_session,
            benchmark::harness::session::claude::cleanup_harness_claude_session,
            benchmark::harness::session::aider::cleanup_harness_aider_session,
            benchmark::harness::session::get_harness_codex_session,
            benchmark::harness::session::start_harness_codex_session,
            benchmark::harness::session::cleanup_harness_codex_session,
            benchmark::harness::runtime::check_harness_codex_runtime,
            benchmark::harness::runtime::cleanup_harness_codex_runtime,
            benchmark::harness::workspace::prepare_harness_workspace,
            benchmark::harness::workspace::submit_harness_workspace,
            benchmark::harness::workspace::close_harness_workspace,
            benchmark::harness::preflight::get_harness_execution_preflight,
            benchmark::harness::preflight::check_harness_execution_preflight,
            benchmark::harness::programming::validation::get_harness_python_environment,
            benchmark::harness::programming::validation::get_harness_programming_validation,
            benchmark::harness::programming::validation::start_harness_programming_validation,
            benchmark::harness::programming::validation::stop_harness_programming_validation,
            benchmark::harness::programming::validation::cleanup_harness_programming_validation,
            benchmark::harness::programming::validation::publish_harness_programming_task,
            benchmark::harness::programming::validation::get_harness_programming_taskset,
            benchmark::harness::programming::get_harness_programming_drafts,
            benchmark::harness::programming::save_harness_programming_draft,
            benchmark::harness::tasks::get_harness_tasks,
            benchmark::harness::tasks::scenarios::run_harness_readonly_task,
            benchmark::harness::tasks::scenarios::get_harness_readonly_runs,
            benchmark::harness::tasks::create_harness_tasks,
            benchmark::harness::tasks::revise_harness_task,
            benchmark::harness::tasks::publish_harness_tasks,
            benchmark::harness::tasks::get_harness_taskset,
            benchmark::harness::tasks::get_harness_task_project,
            benchmark::harness::tasks::import_harness_repository,
            benchmark::harness::get_harness_fit,
            benchmark::harness::analyze_harness_fit,
            benchmark::harness::save_harness_fit,
            benchmark::harness::get_harness_document,
            benchmark::harness::import_harness_document,
            benchmark::standard::list_standard_benchmark_cases,
            benchmark::standard::plan_standard_benchmark,
            benchmark::report_read::get_benchmark_overview,
            benchmark::report_read::get_benchmark_result_page,
            benchmark::report_read::get_benchmark_result_detail,
            benchmark::report_read::get_benchmark_result_location,
            benchmark::standard::get_standard_benchmark,
            benchmark::standard::list_standard_benchmarks,
            benchmark::standard::start_standard_benchmark,
            benchmark::standard::stop_standard_benchmark,
            benchmark::personalized::repository::import_benchmark_repository,
            benchmark::personalized::repository::list_benchmark_repositories,
            benchmark::personalized::repository::get_benchmark_repository,
            benchmark::personalized::generation::plan_benchmark_generation,
            benchmark::personalized::generation::get_benchmark_generation,
            benchmark::personalized::generation::list_benchmark_generations,
            benchmark::personalized::generation::start_benchmark_generation,
            benchmark::personalized::generation::stop_personalized_task,
            benchmark::personalized::generation::revise_benchmark_candidate,
            benchmark::personalized::generation::publish_benchmark_dataset,
            benchmark::personalized::batch::plan_personalized_batch,
            benchmark::personalized::batch::get_personalized_batch,
            benchmark::personalized::batch::list_personalized_batches,
            benchmark::personalized::batch::start_personalized_batch,
            benchmark::personalized::report::export_personalized_report,
            benchmark::report_transfer::export_benchmark_report,
            benchmark::regrade::preview_benchmark_regrade,
            benchmark::regrade::run_benchmark_regrade,
            benchmark::regrade::cancel_benchmark_regrade,
            benchmark::regrade::get_benchmark_regrades,
            benchmark::profile::get_benchmark_profile,
            benchmark::profile::save_benchmark_profile,
            cloud::cloud_connection,
            cloud::verify_cloud_connection,
            cloud::cancel_cloud_verification,
            cloud::remove_cloud_connection,
            cloud::personalized_access,
            commands::start_benchmark,
            commands::get_benchmark_run,
            commands::cancel_benchmark,
            commands::list_benchmark_datasets,
            commands::get_benchmark_case,
            commands::list_benchmark_models,
            commands::inspect_benchmark_model,
            commands::discard_source,
            commands::choose_source,
            commands::import_source_folder,
            commands::commit_source,
            commands::list_sources,
            commands::get_source,
            commands::revoke_source,
            commands::save_source_knowledge,
            commands::list_watched_folders,
            commands::sync_watched_folder,
            commands::remove_watched_folder,
            commands::sync_workspace_watched_folders,
            commands::bootstrap,
            commands::create_workspace,
            commands::list_conversations,
            commands::create_conversation,
            commands::rename_conversation,
            commands::load_messages,
            commands::check_ollama,
            commands::save_settings,
            commands::get_run,
            commands::get_chat_run_status,
            commands::list_runs,
            commands::load_execution_graph,
            commands::save_execution_view,
            commands::preview_context,
            commands::set_conversation_retrieval,
            commands::cancel_run,
            commands::list_context_options,
            commands::list_knowledge,
            commands::get_knowledge,
            commands::save_knowledge,
            commands::approve_knowledge,
            commands::load_knowledge_graph,
            commands::query_graph_nodes,
            commands::load_graph_node,
            commands::load_graph_links,
            commands::query_graph_relations,
            commands::load_graph_edge_history,
            commands::save_knowledge_edge,
            commands::save_graph_view,
            commands::start_chat
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<commands::AppState>().inner().clone();
                let was_closing = state.closing.swap(true, Ordering::SeqCst);
                let pending_copy = state.database.as_ref().ok().is_some_and(|db| db.pending_workspace_jobs().is_ok_and(|jobs| !jobs.is_empty()) || db.pending_codex_runtime_jobs().is_ok_and(|jobs| !jobs.is_empty()) || db.pending_aider_runtime_jobs().is_ok_and(|jobs| !jobs.is_empty()) || db.pending_claude_runtime_jobs().is_ok_and(|jobs| !jobs.is_empty()) || db.pending_sessions(benchmark::python::session::Kind::Aider).is_ok_and(|jobs| !jobs.is_empty()) || db.pending_sessions(benchmark::python::session::Kind::Claude).is_ok_and(|jobs| !jobs.is_empty()) || db.pending_sessions(benchmark::python::session::Kind::Codex).is_ok_and(|jobs| !jobs.is_empty()));
                if state.has_shutdown_work() || state.close_cleanup.load(Ordering::SeqCst) || (!was_closing && pending_copy) {
                    api.prevent_close();
                    if !was_closing {
                        state.close_cleanup.store(true, Ordering::SeqCst);
                        state.cancel_active();
                        let window = window.clone();
                        tauri::async_runtime::spawn(async move {
                            while state.has_shutdown_work() {
                                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                            }
                            if let Ok(db) = state.database.clone() {
                                let _ = tauri::async_runtime::spawn_blocking(move || { let _ = benchmark::harness::workspace::cleanup_on_close(&db); let _ = benchmark::harness::runtime::cleanup_on_close(&db); benchmark::harness::session::cleanup_on_close(&db) }).await;
                            }
                            state.close_cleanup.store(false, Ordering::SeqCst);
                            let _ = window.close();
                        });
                    }
                }
            }
        })
        .run(context)
        .expect("CortexOS desktop failed to start");
}
