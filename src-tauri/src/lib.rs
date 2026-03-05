use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::watch;

use git_pre_conflict_core::{conflict, git, guide, AppError, ConflictReport};

/// State for the background watch task.
struct WatchState {
    /// Sender to signal the watch loop to stop.
    stop_tx: Option<watch::Sender<bool>>,
    /// Last conflict report from the watch loop.
    last_report: Option<ConflictReport>,
    /// Whether watching is active.
    is_watching: bool,
    /// Target branch being watched.
    target: Option<String>,
    /// Optional repo path (None = CWD).
    repo_path: Option<String>,
}

impl Default for WatchState {
    fn default() -> Self {
        Self {
            stop_tx: None,
            last_report: None,
            is_watching: false,
            target: None,
            repo_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub is_watching: bool,
    pub target: Option<String>,
    pub last_report: Option<ConflictReport>,
    pub repo_path: Option<String>,
}

/// Run a single conflict check.
fn do_check(
    repo_path: Option<&str>,
    target: &str,
    no_fetch: bool,
) -> Result<ConflictReport, AppError> {
    git::find_git_dir(repo_path)?;

    let current = git::current_branch(repo_path)?;

    if !no_fetch {
        // Best-effort fetch; continue on failure
        let _ = git::fetch_origin(repo_path, target);
    }

    let target_ref = git::resolve_target_ref(repo_path, target)?;
    let result = git::merge_tree(repo_path, &current, &target_ref)?;
    conflict::parse_merge_tree(&result, current, target_ref)
}

#[tauri::command]
fn check_conflicts(
    target: String,
    no_fetch: bool,
    repo_path: Option<String>,
) -> Result<ConflictReport, String> {
    do_check(repo_path.as_deref(), &target, no_fetch).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_current_branch(repo_path: Option<String>) -> Result<String, String> {
    git::current_branch(repo_path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_branches(repo_path: Option<String>) -> Result<Vec<String>, String> {
    git::list_branches(repo_path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick_directory(window: tauri::Window) -> Result<Option<String>, String> {
    let path = window
        .dialog()
        .file()
        .set_title("Select Git Repository")
        .blocking_pick_folder()
        .map(|p| p.to_string());
    Ok(path)
}

#[tauri::command]
async fn start_watch(
    target: String,
    interval_secs: u64,
    repo_path: Option<String>,
    state: State<'_, Arc<Mutex<WatchState>>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Stop any existing watch first
    {
        let mut ws = state.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = ws.stop_tx.take() {
            let _ = tx.send(true);
        }
    }

    let (stop_tx, mut stop_rx) = watch::channel(false);

    {
        let mut ws = state.lock().map_err(|e| e.to_string())?;
        ws.stop_tx = Some(stop_tx);
        ws.is_watching = true;
        ws.target = Some(target.clone());
        ws.repo_path = repo_path.clone();
    }

    let state_clone = Arc::clone(&state);
    let _app = app.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            // Run the check
            let report = do_check(repo_path.as_deref(), &target, false);

            // Update state with result
            if let Ok(mut ws) = state_clone.lock() {
                match &report {
                    Ok(r) => ws.last_report = Some(r.clone()),
                    Err(_) => {}
                }
            }

            // Send notification if conflicts found
            if let Ok(ref report) = report {
                if !report.is_clean() {
                    #[cfg(desktop)]
                    {
                        use tauri_plugin_notification::NotificationExt;
                        let body = format!(
                            "{} conflict{} merging {} into {}",
                            report.file_count(),
                            if report.file_count() == 1 { "" } else { "s" },
                            report.target_ref,
                            report.current_branch,
                        );
                        let _ = _app
                            .notification()
                            .builder()
                            .title("git-pre-conflict")
                            .body(body)
                            .show();
                    }
                }
            }

            // Wait for interval or stop signal
            let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs));
            tokio::pin!(sleep);

            tokio::select! {
                _ = &mut sleep => {},
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        break;
                    }
                }
            }
        }

        // Clean up state
        if let Ok(mut ws) = state_clone.lock() {
            ws.is_watching = false;
            ws.stop_tx = None;
            ws.target = None;
            ws.repo_path = None;
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_watch(state: State<'_, Arc<Mutex<WatchState>>>) -> Result<(), String> {
    let mut ws = state.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = ws.stop_tx.take() {
        let _ = tx.send(true);
    }
    ws.is_watching = false;
    Ok(())
}

#[tauri::command]
fn get_watch_status(state: State<'_, Arc<Mutex<WatchState>>>) -> Result<WatchStatus, String> {
    let ws = state.lock().map_err(|e| e.to_string())?;
    Ok(WatchStatus {
        is_watching: ws.is_watching,
        target: ws.target.clone(),
        last_report: ws.last_report.clone(),
        repo_path: ws.repo_path.clone(),
    })
}

#[tauri::command]
fn get_conflict_diff(
    repo_path: Option<String>,
    current_branch: String,
    target_ref: String,
    file_path: String,
) -> Result<String, String> {
    let rp = repo_path.as_deref();
    let base = git::merge_base(rp, &current_branch, &target_ref).map_err(|e| e.to_string())?;
    git::diff_file(rp, &base, &target_ref, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_resolution_guide(report: ConflictReport) -> Result<guide::ResolutionGuide, String> {
    Ok(guide::generate_resolution_guide(
        &report.current_branch,
        &report.target_ref,
        &report.conflicted_files,
    ))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Mutex::new(WatchState::default())))
        .invoke_handler(tauri::generate_handler![
            check_conflicts,
            get_current_branch,
            list_branches,
            pick_directory,
            start_watch,
            stop_watch,
            get_watch_status,
            get_conflict_diff,
            get_resolution_guide,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
