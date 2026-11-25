//! App-wide settings and the list of registered projects, stored as JSON in the
//! app data folder (%APPDATA%\local.projectmanager.app on Windows).
//! The library format itself lives in pm_core::library, shared with pm.exe.

use std::fs;
use std::path::PathBuf;

use pm_core::library::{self, Library};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Editor paths. An empty string means "auto-detect".
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub vscode: String,
    pub intellij: String,
    /// Folder that contains one sub-folder per Unity version (Unity Hub's "Editor" folder).
    pub unity_editors_dir: String,
    pub unreal_editor: String,
    pub godot: String,
}

pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn load_settings(app: &AppHandle) -> Settings {
    data_dir(app)
        .ok()
        .and_then(|d| fs::read_to_string(d.join("settings.json")).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_settings(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(data_dir(app)?.join("settings.json"), text).map_err(|e| e.to_string())
}

pub fn load_library(app: &AppHandle) -> Library {
    data_dir(app)
        .map(|d| library::load_library(&d))
        .unwrap_or_default()
}

pub fn save_library(app: &AppHandle, lib: &Library) -> Result<(), String> {
    library::save_library(&data_dir(app)?, lib)
}
