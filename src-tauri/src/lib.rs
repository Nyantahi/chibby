pub mod agent;
pub mod ai;
#[cfg(feature = "gui")]
pub mod commands;
pub mod engine;
pub mod state;

#[cfg(feature = "gui")]
use commands::agent_commands;
#[cfg(feature = "gui")]
use commands::artifact_commands;
#[cfg(feature = "gui")]
use commands::env_commands;
#[cfg(feature = "gui")]
use commands::gate_commands;
#[cfg(feature = "gui")]
use commands::notify_commands;
#[cfg(feature = "gui")]
use commands::pipeline_commands;
#[cfg(feature = "gui")]
use commands::project_commands;
#[cfg(feature = "gui")]
use commands::run_commands;
#[cfg(feature = "gui")]
use commands::settings_commands;
#[cfg(feature = "gui")]
use commands::template_commands;
#[cfg(feature = "gui")]
use commands::updater_commands;
#[cfg(feature = "gui")]
use commands::version_commands;

/// Entry point for the Tauri application.
#[cfg(feature = "gui")]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(state::create_pipeline_state())
        .manage(agent_commands::create_agent_state())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            project_commands::list_projects,
            project_commands::add_project,
            project_commands::remove_project,
            project_commands::get_git_info,
            // Pipeline commands
            pipeline_commands::detect_scripts,
            pipeline_commands::generate_pipeline,
            pipeline_commands::save_pipeline,
            pipeline_commands::load_pipeline,
            pipeline_commands::list_pipelines,
            pipeline_commands::load_pipeline_by_name,
            pipeline_commands::save_pipeline_by_name,
            pipeline_commands::validate_pipeline,
            pipeline_commands::get_github_workflows,
            pipeline_commands::workflows_to_pipeline_stages,
            pipeline_commands::get_recommendations,
            // Run commands
            run_commands::run_pipeline,
            run_commands::cancel_pipeline,
            run_commands::is_pipeline_running,
            run_commands::get_run_history,
            run_commands::get_all_runs,
            run_commands::get_run,
            // Phase 6: Retry, rollback, and history queries
            run_commands::retry_run,
            run_commands::rollback_to_run,
            run_commands::get_last_successful_run,
            run_commands::get_deployment_history,
            run_commands::delete_run,
            run_commands::clear_run_history,
            // Environment & secrets commands
            env_commands::load_environments,
            env_commands::save_environments,
            env_commands::load_secrets_config,
            env_commands::save_secrets_config,
            env_commands::set_secret,
            env_commands::delete_secret,
            env_commands::check_secrets_status,
            env_commands::test_ssh_connection,
            env_commands::run_preflight,
            // Version commands
            version_commands::detect_versions,
            version_commands::bump_version,
            version_commands::generate_changelog,
            // Artifact & signing commands
            artifact_commands::load_artifact_config,
            artifact_commands::save_artifact_config,
            artifact_commands::collect_artifacts,
            artifact_commands::list_artifact_manifests,
            artifact_commands::load_signing_config,
            artifact_commands::save_signing_config,
            artifact_commands::sign_artifact,
            artifact_commands::check_signing_tools,
            artifact_commands::load_cleanup_config,
            artifact_commands::save_cleanup_config,
            artifact_commands::run_cleanup,
            // Notification commands
            notify_commands::load_notify_config,
            notify_commands::save_notify_config,
            notify_commands::send_test_notification,
            // Updater commands (Phase 5.5)
            updater_commands::load_updater_config,
            updater_commands::save_updater_config,
            updater_commands::generate_update_keys,
            updater_commands::import_update_private_key,
            updater_commands::has_update_key,
            updater_commands::rotate_update_keys,
            updater_commands::delete_update_key,
            updater_commands::updater_preflight,
            updater_commands::sign_update_bundle,
            updater_commands::generate_latest_json,
            updater_commands::merge_latest_json,
            updater_commands::check_tauri_cli,
            updater_commands::publish_update,
            // Security & quality gate commands (Phase 5.8)
            gate_commands::load_gates_config,
            gate_commands::save_gates_config,
            gate_commands::run_gates,
            gate_commands::run_secret_scan,
            gate_commands::run_dependency_audit,
            gate_commands::run_commit_lint,
            gate_commands::create_secret_scan_baseline,
            // Template commands
            template_commands::get_templates,
            template_commands::get_template,
            template_commands::get_template_variables,
            template_commands::apply_template,
            template_commands::save_custom_template,
            template_commands::delete_custom_template,
            template_commands::export_template,
            template_commands::import_template,
            // Agent commands (Phase 8)
            agent_commands::get_agent_status,
            agent_commands::analyze_run,
            agent_commands::agent_chat,
            agent_commands::generate_pipeline_config,
            agent_commands::save_generated_pipeline,
            agent_commands::get_agent_memories,
            agent_commands::delete_agent_memory,
            agent_commands::rebuild_agent,
            // App settings commands
            settings_commands::load_app_settings,
            settings_commands::save_app_settings,
            settings_commands::set_app_api_key,
            settings_commands::delete_app_api_key,
            settings_commands::has_app_api_key,
            settings_commands::get_app_data_dir,
            settings_commands::get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Chibby");
}
