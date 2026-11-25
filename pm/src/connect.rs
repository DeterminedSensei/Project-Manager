//! Registers pm.exe as an MCP server with AI assistants.
//!
//! - Claude Desktop: adds an entry to `claude_desktop_config.json`. Both the normal
//!   location (%APPDATA%\Claude) and the Microsoft Store location
//!   (%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude) are handled.
//!   A backup is written before every change and all other settings are kept.
//! - Claude Code: runs `claude mcp add/remove --scope user` (the supported way).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::library::same_path;

pub const SERVER_KEY: &str = "project-manager";
const DESKTOP_CONFIG: &str = "claude_desktop_config.json";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    /// "claude-desktop" or "claude-code"
    pub id: String,
    pub name: String,
    pub installed: bool,
    /// Registered with the current pm.exe path.
    pub connected: bool,
    /// Registered, but with a different pm.exe path.
    pub outdated: bool,
    pub detail: String,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().trim_start_matches(r"\\?\").to_string()
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Copies `file` to `<name>.backup-<time>.json` next to it.
fn backup(file: &Path) -> Result<PathBuf, String> {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let target = file.with_file_name(format!("{stem}.backup-{}.json", timestamp()));
    fs::copy(file, &target).map_err(|e| format!("Could not back up {}: {e}", file.display()))?;
    Ok(target)
}

/// `Ok(None)` if the file does not exist; an error if it exists but is not valid JSON.
fn read_json(file: &Path) -> Result<Option<Value>, String> {
    if !file.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(Some(json!({})));
    }
    serde_json::from_str(text).map(Some).map_err(|e| {
        format!(
            "{} is not valid JSON ({e}). Nothing was changed; fix or remove that file first.",
            file.display()
        )
    })
}

fn write_json(file: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(file, text).map_err(|e| format!("{}: {e}", file.display()))
}

fn registered_command(cfg: &Value) -> Option<String> {
    cfg.get("mcpServers")?
        .get(SERVER_KEY)?
        .get("command")?
        .as_str()
        .map(|s| s.to_string())
}

fn status_from(
    id: &str,
    name: &str,
    installed: bool,
    commands: &[String],
    pm: Option<&Path>,
    where_: &str,
) -> ClientStatus {
    let pm_s = pm.map(path_string);
    let connected = match &pm_s {
        Some(p) => commands.iter().any(|c| same_path(c, p)),
        None => false,
    };
    let outdated = !connected && !commands.is_empty();
    let detail = if !installed {
        format!("{name} was not found on this PC.")
    } else if connected {
        format!("Connected ({where_}).")
    } else if outdated {
        format!("Registered with another pm.exe: {}", commands.join(", "))
    } else {
        "Not connected.".to_string()
    };
    ClientStatus {
        id: id.into(),
        name: name.into(),
        installed,
        connected,
        outdated,
        detail,
    }
}

// ---------- Claude Desktop ----------

/// Config files Claude Desktop may read, for every installation that exists.
pub fn desktop_config_paths() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(appdata) = env_path("APPDATA") {
        let dir = appdata.join("Claude");
        if dir.is_dir() {
            v.push(dir.join(DESKTOP_CONFIG));
        }
    }
    if let Some(local) = env_path("LOCALAPPDATA") {
        if let Ok(entries) = fs::read_dir(local.join("Packages")) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("claude_") && e.path().is_dir() {
                    v.push(
                        e.path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude")
                            .join(DESKTOP_CONFIG),
                    );
                }
            }
        }
    }
    v
}

pub fn claude_desktop_status(pm: Option<&Path>) -> ClientStatus {
    let paths = desktop_config_paths();
    let commands: Vec<String> = paths
        .iter()
        .filter_map(|p| read_json(p).ok().flatten())
        .filter_map(|cfg| registered_command(&cfg))
        .collect();
    status_from(
        "claude-desktop",
        "Claude Desktop",
        !paths.is_empty(),
        &commands,
        pm,
        "restart Claude Desktop after changes",
    )
}

/// Adds the given server to an existing config value; errors if the structure is unexpected.
pub fn add_server_entry(cfg: &mut Value, command: &str) -> Result<(), String> {
    if !cfg.is_object() {
        return Err("The config file does not contain a JSON object".into());
    }
    let root = cfg.as_object_mut().expect("checked above");
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or("\"mcpServers\" in the config file is not an object")?;
    servers.insert(
        SERVER_KEY.to_string(),
        json!({ "command": command, "args": ["mcp"] }),
    );
    Ok(())
}

pub fn remove_server_entry(cfg: &mut Value) -> bool {
    cfg.get_mut("mcpServers")
        .and_then(|s| s.as_object_mut())
        .map(|s| s.shift_remove(SERVER_KEY).is_some())
        .unwrap_or(false)
}

pub fn connect_claude_desktop(pm: &Path) -> Result<String, String> {
    let paths = desktop_config_paths();
    if paths.is_empty() {
        return Err("Claude Desktop was not found on this PC.".into());
    }
    // Read everything first so an invalid file stops us before anything is written.
    let mut existing = vec![];
    for p in &paths {
        existing.push(read_json(p)?);
    }
    // A missing file is started from another location's settings so nothing is hidden.
    let seed = existing
        .iter()
        .flatten()
        .next()
        .cloned()
        .unwrap_or_else(|| json!({}));
    let command = path_string(pm);
    let mut backups = vec![];
    for (p, cfg) in paths.iter().zip(existing) {
        let had_file = cfg.is_some();
        let mut cfg = cfg.unwrap_or_else(|| seed.clone());
        add_server_entry(&mut cfg, &command)?;
        if had_file {
            backups.push(backup(p)?);
        }
        write_json(p, &cfg)?;
    }
    let mut msg = String::from("Connected to Claude Desktop. Quit Claude Desktop (tray icon → Quit) and start it again to load the tools.");
    if !backups.is_empty() {
        let names: Vec<String> = backups.iter().map(|b| path_string(b)).collect();
        msg.push_str(&format!(" Backup: {}", names.join(", ")));
    }
    Ok(msg)
}

pub fn disconnect_claude_desktop() -> Result<String, String> {
    let mut changed = 0;
    for p in desktop_config_paths() {
        if let Some(mut cfg) = read_json(&p)? {
            if registered_command(&cfg).is_some() {
                backup(&p)?;
                remove_server_entry(&mut cfg);
                write_json(&p, &cfg)?;
                changed += 1;
            }
        }
    }
    Ok(if changed > 0 {
        "Disconnected from Claude Desktop. Restart Claude Desktop to apply.".into()
    } else {
        "Claude Desktop was not connected.".into()
    })
}

// ---------- Claude Code ----------

/// Finds the `claude` command: the native installer's location, then PATH.
pub fn find_claude_cli() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Some(home) = env_path("USERPROFILE") {
            let p = home.join(".local").join("bin").join("claude.exe");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for n in names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn claude_json_path() -> Option<PathBuf> {
    env_path("USERPROFILE")
        .or_else(|| env_path("HOME"))
        .map(|h| h.join(".claude.json"))
}

pub fn claude_code_status(pm: Option<&Path>) -> ClientStatus {
    let installed = find_claude_cli().is_some();
    // Read-only look at Claude Code's user config to see what is registered.
    let commands: Vec<String> = claude_json_path()
        .and_then(|p| read_json(&p).ok().flatten())
        .and_then(|cfg| registered_command(&cfg))
        .into_iter()
        .collect();
    status_from(
        "claude-code",
        "Claude Code",
        installed,
        &commands,
        pm,
        "available in all folders",
    )
}

fn run_claude(args: &[&str]) -> Result<String, String> {
    let exe =
        find_claude_cli().ok_or("Claude Code (the `claude` command) was not found on this PC.")?;
    let mut cmd = Command::new(&exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Could not run {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

pub fn connect_claude_code(pm: &Path) -> Result<String, String> {
    let command = path_string(pm);
    // Remove an older registration first; "add" fails if the name already exists.
    let _ = run_claude(&["mcp", "remove", "--scope", "user", SERVER_KEY]);
    run_claude(&[
        "mcp",
        "add",
        "--scope",
        "user",
        SERVER_KEY,
        "--",
        command.as_str(),
        "mcp",
    ])
    .map_err(|e| format!("claude mcp add failed: {e}"))?;
    Ok("Connected to Claude Code (user scope, all folders). New Claude Code sessions will have the tools.".into())
}

pub fn disconnect_claude_code() -> Result<String, String> {
    run_claude(&["mcp", "remove", "--scope", "user", SERVER_KEY])
        .map_err(|e| format!("claude mcp remove failed: {e}"))?;
    Ok("Disconnected from Claude Code.".into())
}

// ---------- dispatch ----------

pub fn all_status(pm: Option<&Path>) -> Vec<ClientStatus> {
    vec![claude_desktop_status(pm), claude_code_status(pm)]
}

pub fn connect(client: &str, pm: &Path) -> Result<String, String> {
    match client {
        "claude-desktop" => connect_claude_desktop(pm),
        "claude-code" => connect_claude_code(pm),
        other => Err(format!("Unknown client: {other}")),
    }
}

pub fn disconnect(client: &str) -> Result<String, String> {
    match client {
        "claude-desktop" => disconnect_claude_desktop(),
        "claude-code" => disconnect_claude_code(),
        other => Err(format!("Unknown client: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_removes_entry_keeping_other_settings() {
        let mut cfg =
            json!({ "preferences": { "x": 1 }, "mcpServers": { "other": { "command": "a" } } });
        add_server_entry(&mut cfg, "C:\\pm.exe").unwrap();
        assert_eq!(registered_command(&cfg).unwrap(), "C:\\pm.exe");
        assert_eq!(cfg["preferences"]["x"], 1);
        assert!(cfg["mcpServers"]["other"].is_object());
        assert!(remove_server_entry(&mut cfg));
        assert!(registered_command(&cfg).is_none());
        assert!(cfg["mcpServers"]["other"].is_object());
    }

    #[test]
    fn rejects_bad_structure() {
        let mut cfg = json!({ "mcpServers": 5 });
        assert!(add_server_entry(&mut cfg, "x").is_err());
        let mut cfg = json!([1, 2]);
        assert!(add_server_entry(&mut cfg, "x").is_err());
    }
}
