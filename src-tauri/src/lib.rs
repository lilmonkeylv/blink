mod commands;
mod error;
mod launch;
mod state;
mod view;
mod watch;
mod wire;

use tauri::Manager;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::state::AppState;

fn init_logging(log_dir: Option<&std::path::Path>) {
    let filter = tracing_subscriber::EnvFilter::try_from_env("BLINK_LOG").unwrap_or_else(|_| "info,blink=debug".into());
    let file = log_dir.and_then(|dir| {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::OpenOptions::new().create(true).append(true).open(dir.join("blink.log")).ok()
    });
    let file_layer =
        file.map(|f| tracing_subscriber::fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(f)));
    tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer()).with(file_layer).init();
}

pub fn run() {
    let paths = blink_store::AppPaths::system().ok();
    init_logging(paths.as_ref().map(|p| p.log_dir.as_path()));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            launch::handle_args(app, &argv, Some(cwd.as_ref()));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::updater::PendingUpdate::default())
        .setup(|app| {
            let state = AppState::initialize().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            state.engine.pools().background().spawn(blink_diff::warm_up);
            app.manage(state);
            let handle = app.handle().clone();
            launch::handle_args(&handle, &std::env::args().collect::<Vec<_>>(), None);
            launch::register_deep_links(&handle);
            commands::sync::spawn_background_fetch(handle.clone());
            commands::github::spawn_checks_poller(handle.clone());
            commands::updater::spawn_update_checks(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::repo::bootstrap,
            commands::repo::shared_state,
            commands::repo::add_repository,
            commands::repo::init_repository,
            commands::repo::create_repository,
            commands::repo::remove_repository,
            commands::repo::open_repository,
            commands::repo::refresh_repository,
            commands::repo::reveal_in_file_manager,
            commands::repo::open_on_github,
            commands::repo::remotes,
            commands::repo::repo_settings,
            commands::repo::save_repo_settings,
            commands::repo::import_repositories,
            commands::repo::desktop_repositories,
            commands::window::open_window,
            commands::changes::file_diff,
            commands::changes::commit,
            commands::changes::discard_files,
            commands::changes::discard_lines,
            commands::changes::ignore_path,
            commands::changes::undo_last_commit,
            commands::changes::git_identity,
            commands::changes::set_git_identity,
            commands::history::history,
            commands::history::commit_detail,
            commands::history::commit_file_diff,
            commands::branches::branches,
            commands::branches::checkout_branch,
            commands::branches::create_branch,
            commands::branches::rename_branch,
            commands::branches::delete_branch,
            commands::branches::merge_branch,
            commands::branches::abort_merge,
            commands::branches::resolve_conflict,
            commands::branches::stash_changes,
            commands::branches::stash_list,
            commands::branches::stash_pop,
            commands::branches::stash_drop,
            commands::sync::sync,
            commands::rewrite::revert_commit,
            commands::rewrite::cherry_pick,
            commands::rewrite::reset_to_commit,
            commands::rewrite::checkout_commit,
            commands::rewrite::tags,
            commands::rewrite::create_tag,
            commands::rewrite::delete_tag,
            commands::rewrite::rebase_branch,
            commands::rewrite::rebase_interactive,
            commands::rewrite::squash_commits,
            commands::rewrite::continue_operation,
            commands::rewrite::abort_operation,
            commands::rewrite::skip_operation,
            commands::rewrite::undo_history,
            commands::rewrite::undo,
            commands::inspect::blame,
            commands::inspect::file_history,
            commands::inspect::search_history,
            commands::inspect::reflog,
            commands::inspect::history_graph,
            commands::inspect::range_files,
            commands::inspect::range_file_diff,
            commands::inspect::compare_branch,
            commands::inspect::worktrees,
            commands::inspect::add_worktree,
            commands::inspect::remove_worktree,
            commands::inspect::commit_patch,
            commands::inspect::apply_patch,
            commands::inspect::image_pair,
            commands::inspect::read_worktree_file,
            commands::inspect::write_resolved_file,
            commands::inspect::conflict_versions,
            commands::github::github_repositories,
            commands::github::github_orgs,
            commands::github::gitignore_templates,
            commands::github::clone_repository,
            commands::github::publish_repository,
            commands::github::pull_requests,
            commands::github::checkout_pull_request,
            commands::github::create_pull_request,
            commands::github::branch_checks,
            commands::github::rerun_failed_checks,
            commands::github::fork_info,
            commands::github::add_upstream,
            commands::integrations::detect_integrations,
            commands::integrations::open_in_editor,
            commands::integrations::open_in_shell,
            commands::auth::sign_in,
            commands::auth::device_start,
            commands::auth::device_complete,
            commands::auth::sign_out,
            commands::auth::revoke_url,
            commands::auth::enterprise_accounts,
            commands::auth::enterprise_sign_in,
            commands::auth::enterprise_sign_out,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::open_logs_folder,
            commands::settings::open_devtools,
            commands::updater::check_for_update,
            commands::updater::install_update,
            commands::notes::whats_new,
            commands::notes::release_notes,
            commands::notes::open_external,
            launch::take_pending_launch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Blink");
}
