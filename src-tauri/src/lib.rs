mod icons;
mod launch;
mod settings;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;

use launch::{DetectedTools, LaunchOption};
use pm_core::connect::{self, ClientStatus};
use pm_core::library::same_path;
use pm_core::project::{
    self, CustomTab, NewWorklogEntry, ProjectMeta, ProjectSummary, WorklogEntry,
};
use pm_core::sessions::{self, SessionInfo};
use pm_core::templates::{self, TabPreset};
use settings::Settings;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDetail {
    summary: ProjectSummary,
    state: String,
    docs: Vec<String>,
    worklog: Vec<WorklogEntry>,
    launchers: Vec<LaunchOption>,
    /// AI tool folders found in the project, e.g. [".claude"].
    ai_folders: Vec<String>,
    /// Where the root AGENTS.md currently points (".projectmanager", ".claude", ...).
    pointer_target: Option<String>,
    /// Page file names of each extra tab, by tab id.
    tab_pages: HashMap<String, Vec<String>>,
}

/// Only allow commands on projects that are registered in the library.
fn registered(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    let lib = settings::load_library(app);
    if lib.projects.iter().any(|p| same_path(p, path)) {
        Ok(PathBuf::from(path))
    } else {
        Err("This project is not in the library".into())
    }
}

// ---------- library ----------

#[tauri::command]
async fn list_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let lib = settings::load_library(&app);
    let mut v: Vec<ProjectSummary> = lib
        .projects
        .iter()
        .map(|p| project::summary(Path::new(p)))
        .collect();
    v.sort_by(|a, b| a.meta.name.to_lowercase().cmp(&b.meta.name.to_lowercase()));
    Ok(v)
}

#[tauri::command]
async fn add_project(app: AppHandle, path: String) -> Result<ProjectSummary, String> {
    let p = PathBuf::from(&path);
    project::init_project(&p)?;
    let mut lib = settings::load_library(&app);
    if !lib.projects.iter().any(|x| same_path(x, &path)) {
        lib.projects.push(path.clone());
        settings::save_library(&app, &lib)?;
    }
    Ok(project::summary(&p))
}

/// Removes a project from the library only. Files on disk are not touched.
#[tauri::command]
fn remove_project(app: AppHandle, path: String) -> Result<(), String> {
    let mut lib = settings::load_library(&app);
    lib.projects.retain(|x| !same_path(x, &path));
    settings::save_library(&app, &lib)
}

#[tauri::command]
async fn get_project(app: AppHandle, path: String) -> Result<ProjectDetail, String> {
    let p = registered(&app, &path)?;
    // Bring the generated AI rules up to date (migrates projects from older versions).
    let _ = project::sync_agents_md(&p);
    let s = settings::load_settings(&app);
    let meta = project::read_meta(&p)?;
    let tab_pages = meta
        .tabs
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                project::list_pages(&p, &t.id).unwrap_or_default(),
            )
        })
        .collect();
    Ok(ProjectDetail {
        summary: project::summary(&p),
        state: project::read_state(&p),
        docs: project::list_docs(&p)?,
        worklog: project::list_worklog(&p)?,
        launchers: launch::launch_options(&p, &s),
        ai_folders: project::detect_ai_folders(&p),
        pointer_target: project::current_pointer_target(&p),
        tab_pages,
    })
}

#[tauri::command]
fn update_meta(app: AppHandle, path: String, meta: ProjectMeta) -> Result<(), String> {
    let p = registered(&app, &path)?;
    project::write_meta(&p, &meta)?;
    project::sync_agents_md(&p)?;
    Ok(())
}

#[tauri::command]
fn save_state(app: AppHandle, path: String, content: String) -> Result<(), String> {
    let p = registered(&app, &path)?;
    project::save_state(&p, &content)
}

// ---------- docs ----------

#[tauri::command]
fn read_page(
    app: AppHandle,
    path: String,
    section: String,
    name: String,
) -> Result<String, String> {
    let p = registered(&app, &path)?;
    project::read_page(&p, &section, &name)
}

#[tauri::command]
fn save_page(
    app: AppHandle,
    path: String,
    section: String,
    name: String,
    content: String,
) -> Result<(), String> {
    let p = registered(&app, &path)?;
    project::save_page(&p, &section, &name, &content)
}

#[tauri::command]
fn create_page(
    app: AppHandle,
    path: String,
    section: String,
    title: String,
) -> Result<String, String> {
    let p = registered(&app, &path)?;
    project::create_page(&p, &section, &title)
}

// ---------- extra tabs ----------

#[tauri::command]
fn tab_presets() -> Vec<TabPreset> {
    templates::tab_presets()
}

#[tauri::command]
fn add_tab(
    app: AppHandle,
    path: String,
    title: String,
    preset: String,
    instructions: String,
) -> Result<CustomTab, String> {
    let p = registered(&app, &path)?;
    project::add_tab(&p, &title, &preset, &instructions)
}

#[tauri::command]
fn update_tab(
    app: AppHandle,
    path: String,
    id: String,
    title: String,
    instructions: String,
) -> Result<CustomTab, String> {
    let p = registered(&app, &path)?;
    project::update_tab(&p, &id, &title, &instructions)
}

#[tauri::command]
fn remove_tab(app: AppHandle, path: String, id: String) -> Result<(), String> {
    let p = registered(&app, &path)?;
    project::remove_tab(&p, &id)
}

// ---------- worklog ----------

#[tauri::command]
fn add_worklog_entry(
    app: AppHandle,
    path: String,
    entry: NewWorklogEntry,
) -> Result<WorklogEntry, String> {
    let p = registered(&app, &path)?;
    project::add_worklog_entry(&p, entry)
}

// ---------- cover / AI ----------

#[tauri::command]
async fn set_cover(app: AppHandle, path: String, source: String) -> Result<String, String> {
    let p = registered(&app, &path)?;
    project::set_cover(&p, Path::new(&source))
}

#[tauri::command]
fn install_ai_pointer(
    app: AppHandle,
    path: String,
    target: Option<String>,
) -> Result<Vec<String>, String> {
    let p = registered(&app, &path)?;
    project::install_ai_pointer(&p, target.as_deref())
}

// ---------- launching / settings ----------

#[tauri::command]
fn launch_tool(app: AppHandle, path: String, tool: String) -> Result<(), String> {
    let p = registered(&app, &path)?;
    let s = settings::load_settings(&app);
    launch::launch(&p, &tool, &s)
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    settings::load_settings(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save_settings(&app, &settings)
}

#[tauri::command]
fn detect_tools(app: AppHandle) -> DetectedTools {
    launch::detect_tools(&settings::load_settings(&app))
}

// ---------- Claude Code sessions ----------

#[tauri::command]
async fn list_sessions(app: AppHandle, path: String) -> Result<Vec<SessionInfo>, String> {
    let p = registered(&app, &path)?;
    Ok(sessions::list_sessions(&p))
}

#[tauri::command]
fn resume_session(app: AppHandle, path: String, id: String) -> Result<(), String> {
    let p = registered(&app, &path)?;
    if !sessions::valid_session_id(&id) {
        return Err("Invalid session id".into());
    }
    launch::start_claude(&p, &["--resume", id.as_str()])
}

#[tauri::command]
fn delete_session(app: AppHandle, path: String, id: String) -> Result<(), String> {
    let p = registered(&app, &path)?;
    sessions::delete_session(&p, &id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiToolsInfo {
    /// Full path of pm.exe, if found.
    pm_path: Option<String>,
}

fn clean_path(p: PathBuf) -> String {
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    p.to_string_lossy().trim_start_matches(r"\\?\").to_string()
}

/// Finds pm.exe: next to this app (installed version), or in the pm crate's
/// build folder (development).
fn find_pm() -> Option<PathBuf> {
    let exe_name = if cfg!(windows) { "pm.exe" } else { "pm" };
    let mut candidates = vec![];
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            candidates.push(dir.join(exe_name));
        }
    }
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("pm")
        .join("target");
    candidates.push(dev.join("release").join(exe_name));
    candidates.push(dev.join("debug").join(exe_name));
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| PathBuf::from(clean_path(p)))
}

#[tauri::command]
fn ai_tools_info() -> AiToolsInfo {
    AiToolsInfo {
        pm_path: find_pm().map(|p| p.to_string_lossy().to_string()),
    }
}

#[tauri::command]
async fn ai_clients() -> Vec<ClientStatus> {
    connect::all_status(find_pm().as_deref())
}

#[tauri::command]
async fn connect_client(client: String) -> Result<String, String> {
    let pm = find_pm().ok_or("pm.exe has not been built yet. Run: npm run build:pm")?;
    connect::connect(&client, &pm)
}

#[tauri::command]
async fn disconnect_client(client: String) -> Result<String, String> {
    connect::disconnect(&client)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_projects,
            add_project,
            remove_project,
            get_project,
            update_meta,
            save_state,
            read_page,
            save_page,
            create_page,
            tab_presets,
            add_tab,
            update_tab,
            remove_tab,
            add_worklog_entry,
            set_cover,
            install_ai_pointer,
            launch_tool,
            get_settings,
            save_settings,
            detect_tools,
            ai_tools_info,
            ai_clients,
            connect_client,
            disconnect_client,
            list_sessions,
            resume_session,
            delete_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running Project Manager");
}
