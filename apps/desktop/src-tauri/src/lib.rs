mod benchmark;
mod citations;
mod cloud;
mod commands;
mod db;
mod execution;
mod generation;
mod graph;
mod graph_query;
mod knowledge;
mod models;
mod ollama;
mod retrieval;
mod sources;
mod tasks;

use std::sync::atomic::Ordering;
use tauri::Manager;

pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(debug_assertions)]
    let context = {
        let mut context = context;
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
        context
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let mut directory = app.path().app_data_dir()?;
            #[cfg(debug_assertions)]
            if let Ok(value) = std::env::var("CORTEXOS_TEST_DATA_DIR") {
                directory = std::path::PathBuf::from(value);
            }
            app.manage(commands::AppState::new(&directory));
            app.manage(cloud::CloudState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::commit_source,
            commands::list_sources,
            commands::get_source,
            commands::revoke_source,
            commands::save_source_knowledge,
            commands::bootstrap,
            commands::create_workspace,
            commands::list_conversations,
            commands::create_conversation,
            commands::rename_conversation,
            commands::load_messages,
            commands::check_ollama,
            commands::save_settings,
            commands::get_run,
            commands::list_runs,
            commands::load_execution_graph,
            commands::save_execution_view,
            commands::preview_context,
            commands::set_conversation_retrieval,
            commands::cancel_run,
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
                if state.has_active() || state.close_cleanup.load(Ordering::SeqCst) || (!was_closing && pending_copy) {
                    api.prevent_close();
                    if !was_closing {
                        state.close_cleanup.store(true, Ordering::SeqCst);
                        state.cancel_active();
                        let window = window.clone();
                        tauri::async_runtime::spawn(async move {
                            while state.has_active() {
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
