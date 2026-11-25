//! The list of registered projects (library.json in the app data folder) and
//! looking projects up by name or path.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project;

/// Must match `identifier` in src-tauri/tauri.conf.json: Tauri stores app data
/// in `%APPDATA%\<identifier>` on Windows.
pub const APP_IDENTIFIER: &str = "local.projectmanager.app";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Library {
    pub projects: Vec<String>,
}

/// The app data folder without needing a running Tauri app.
pub fn default_data_dir() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_IDENTIFIER));
    }
    // Non-Windows fallback (same place Tauri uses on Linux).
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg).join(APP_IDENTIFIER));
    }
    std::env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join(".local")
            .join("share")
            .join(APP_IDENTIFIER)
    })
}

pub fn load_library(data_dir: &Path) -> Library {
    fs::read_to_string(data_dir.join("library.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_library(data_dir: &Path, lib: &Library) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(lib).map_err(|e| e.to_string())?;
    fs::write(data_dir.join("library.json"), text).map_err(|e| e.to_string())
}

/// Windows paths are case-insensitive; also ignore trailing slashes and slash style.
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

pub fn is_registered(data_dir: &Path, path: &str) -> bool {
    load_library(data_dir)
        .projects
        .iter()
        .any(|p| same_path(p, path))
}

/// Registers a folder: creates `.projectmanager/` if needed and adds it to the library.
pub fn add_project(data_dir: &Path, path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    project::init_project(&p)?;
    let mut lib = load_library(data_dir);
    if !lib.projects.iter().any(|x| same_path(x, path)) {
        lib.projects.push(path.to_string());
        save_library(data_dir, &lib)?;
    }
    Ok(p)
}

/// Finds a registered project by folder path, project name or folder name
/// (case-insensitive).
pub fn find_project(data_dir: &Path, query: &str) -> Result<PathBuf, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("No project given".into());
    }
    let lib = load_library(data_dir);
    if let Some(p) = lib.projects.iter().find(|p| same_path(p, query)) {
        return Ok(PathBuf::from(p));
    }
    let mut matches = vec![];
    for p in &lib.projects {
        let path = Path::new(p);
        let folder = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name = project::read_meta(path).map(|m| m.name).unwrap_or_default();
        if name.eq_ignore_ascii_case(query) || folder.eq_ignore_ascii_case(query) {
            matches.push(p.clone());
        }
    }
    match matches.len() {
        1 => Ok(PathBuf::from(&matches[0])),
        0 => Err(format!(
            "No project named \"{query}\" in the Project Manager library. Use list_projects to see the registered projects."
        )),
        _ => Err(format!(
            "Several projects match \"{query}\"; use the folder path instead: {}",
            matches.join(", ")
        )),
    }
}

/// Walks up from `start` to the first folder that contains `.projectmanager/project.json`.
pub fn find_project_upwards(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if project::pm_dir(d).join("project.json").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}
