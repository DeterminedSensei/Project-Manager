//! Claude Code chat sessions of a project.
//!
//! Claude Code stores each session as `~/.claude/projects/<encoded-path>/<session-id>.jsonl`,
//! where `<encoded-path>` is the working directory with every non-alphanumeric character
//! replaced by `-`. The line format is internal to Claude Code and changes between
//! versions, so this module reads only a few common fields and skips anything it does
//! not understand.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::library::same_path;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    /// Name set with /rename, if any.
    pub name: Option<String>,
    /// Claude Code's own summary of the conversation, if any.
    pub summary: Option<String>,
    /// The first real prompt the user typed.
    pub first_prompt: Option<String>,
    pub started: Option<String>,
    pub last_activity: Option<String>,
    pub message_count: usize,
    pub git_branch: Option<String>,
    pub size_bytes: u64,
}

/// `~/.claude/projects` (or `$CLAUDE_CONFIG_DIR/projects`).
pub fn claude_projects_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("projects"));
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|h| PathBuf::from(h).join(".claude").join("projects"))
}

/// `X:\Programlul\Game` -> `X--Programlul-Game`
pub fn encode_project_path(project: &Path) -> String {
    let s = project.to_string_lossy();
    let s = s.trim_start_matches(r"\\?\").trim_end_matches(['\\', '/']);
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Reads the `cwd` of the first entry that has one (used as a fallback match).
fn first_cwd(file: &Path) -> Option<String> {
    let f = fs::File::open(file).ok()?;
    for line in BufReader::new(f).lines().take(50).map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                return Some(cwd.to_string());
            }
        }
    }
    None
}

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            let is_jsonl = p
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("jsonl"))
                .unwrap_or(false);
            if p.is_file() && is_jsonl {
                v.push(p);
            }
        }
    }
    v
}

/// The folder where Claude Code keeps this project's sessions, if it exists.
pub fn sessions_dir(project: &Path) -> Option<PathBuf> {
    let root = claude_projects_dir()?;
    let wanted = encode_project_path(project).to_lowercase();
    let entries: Vec<PathBuf> = fs::read_dir(&root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // 1. Folder name matches the encoded path (drive letter case may differ).
    if let Some(p) = entries.iter().find(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase() == wanted)
            .unwrap_or(false)
    }) {
        return Some(p.clone());
    }
    // 2. Fallback for long or unusual paths: a session in the folder was started in this project.
    let project_s = project.to_string_lossy().to_string();
    entries.into_iter().find(|dir| {
        jsonl_files(dir)
            .first()
            .and_then(|f| first_cwd(f))
            .map(|cwd| same_path(&cwd, &project_s))
            .unwrap_or(false)
    })
}

/// Text of a user message, or None if it is not something the user typed
/// (tool results, command output, system reminders).
fn user_text(entry: &Value) -> Option<String> {
    let content = entry.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            if parts
                .iter()
                .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
            {
                return None;
            }
            parts
                .iter()
                .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => return None,
    };
    let t = text.trim();
    if t.is_empty() || t.starts_with('<') || t.starts_with("Caveat:") {
        return None;
    }
    Some(t.to_string())
}

fn shorten(s: &str, max: usize) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        let cut: String = one_line.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    }
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// Reads one transcript file. Lines that fail to parse are skipped.
pub fn read_session(file: &Path) -> Option<SessionInfo> {
    let id = file.file_stem()?.to_string_lossy().to_string();
    let size_bytes = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    let f = fs::File::open(file).ok()?;
    let mut info = SessionInfo {
        id,
        name: None,
        summary: None,
        first_prompt: None,
        started: None,
        last_activity: None,
        message_count: 0,
        git_branch: None,
        size_bytes,
    };
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(t) = str_field(&v, "customTitle") {
            info.name = Some(t);
        }
        let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if kind == "summary" {
            if let Some(s) = str_field(&v, "summary") {
                info.summary = Some(s);
            }
            continue;
        }
        if kind != "user" && kind != "assistant" {
            continue;
        }
        let sidechain = v
            .get("isSidechain")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        let meta = v.get("isMeta").and_then(|b| b.as_bool()).unwrap_or(false);
        if sidechain || meta {
            continue;
        }
        if let Some(ts) = str_field(&v, "timestamp") {
            if info.started.is_none() {
                info.started = Some(ts.clone());
            }
            info.last_activity = Some(ts);
        }
        if info.git_branch.is_none() {
            info.git_branch = str_field(&v, "gitBranch").filter(|b| !b.is_empty());
        }
        if kind == "user" {
            if let Some(t) = user_text(&v) {
                info.message_count += 1;
                if info.first_prompt.is_none() {
                    info.first_prompt = Some(shorten(&t, 160));
                }
            }
        } else {
            info.message_count += 1;
        }
    }
    Some(info)
}

/// All sessions of a project that contain at least one real prompt, newest first.
pub fn list_sessions(project: &Path) -> Vec<SessionInfo> {
    let Some(dir) = sessions_dir(project) else {
        return vec![];
    };
    let mut v: Vec<SessionInfo> = jsonl_files(&dir)
        .iter()
        .filter_map(|f| read_session(f))
        .filter(|s| s.first_prompt.is_some())
        .collect();
    v.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    v
}

/// Number of transcript files (cheap; used to decide whether "Continue" makes sense).
pub fn session_count(project: &Path) -> usize {
    sessions_dir(project)
        .map(|d| jsonl_files(&d).len())
        .unwrap_or(0)
}

/// Session ids are UUIDs; reject anything that could be a path.
pub fn valid_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Deletes the transcript and its folder of subagent transcripts. Cannot be undone.
pub fn delete_session(project: &Path, id: &str) -> Result<(), String> {
    if !valid_session_id(id) {
        return Err("Invalid session id".into());
    }
    let dir = sessions_dir(project).ok_or("No Claude Code sessions found for this project")?;
    let file = dir.join(format!("{id}.jsonl"));
    if !file.is_file() {
        return Err("Session not found".into());
    }
    fs::remove_file(&file).map_err(|e| format!("Could not delete {}: {e}", file.display()))?;
    let side = dir.join(id);
    if side.is_dir() {
        fs::remove_dir_all(&side)
            .map_err(|e| format!("Could not delete {}: {e}", side.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_paths() {
        assert_eq!(
            encode_project_path(Path::new(r"X:\Programlul\ClaudeProjects\ProjectManager")),
            "X--Programlul-ClaudeProjects-ProjectManager"
        );
        assert_eq!(
            encode_project_path(Path::new("/home/me/my app")),
            "-home-me-my-app"
        );
    }

    #[test]
    fn session_ids() {
        assert!(valid_session_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!valid_session_id("../x"));
        assert!(!valid_session_id(r"a\b"));
        assert!(!valid_session_id(""));
    }
}
