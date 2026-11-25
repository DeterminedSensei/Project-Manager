//! A small MCP (Model Context Protocol) server over stdio.
//!
//! AI assistants (Claude Desktop, Claude Code and other MCP clients) start
//! `pm.exe mcp` and talk JSON-RPC 2.0 with it, one JSON message per line.
//! Only projects registered in the Project Manager library can be accessed.
//! Nothing but protocol messages is ever written to stdout.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{context, library, project};

const SERVER_NAME: &str = "project-manager";
const DEFAULT_PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "Project Manager keeps documentation, a current-state summary and an \
append-only work log for each of the user's software projects. Before working on a project, call \
get_project_context. After each meaningful unit of work (and before you finish), call \
add_worklog_entry. When the overall state of the project changed, rewrite it with update_state. \
Document new features and architecture with write_doc. Some projects have extra sections \
(for example a Tutorial) listed in get_project_context: keep them up to date as their instructions \
say, using list_docs, read_doc and write_doc with the section argument. Worklog entries get the \
correct time automatically; never write times yourself.";

pub fn run(data_dir: PathBuf) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = handle_message(&data_dir, &line) {
            let mut s = resp.to_string();
            s.push('\n');
            stdout.write_all(s.as_bytes())?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Handles one JSON-RPC message. Returns `None` for notifications and responses.
pub fn handle_message(data_dir: &Path, line: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("Parse error: {e}"),
            ))
        }
    };
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = match msg.get("id") {
        Some(id) if !id.is_null() => id.clone(),
        _ => return None, // notification (e.g. notifications/initialized)
    };
    if method.is_empty() {
        return None; // a response to something we never sent
    }
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => Ok(call_tool(data_dir, &params)),
        _ => Err(format!("Method not found: {method}")),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(m) => error_response(id, -32601, &m),
    })
}

fn initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROTOCOL);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS
    })
}

fn project_prop() -> Value {
    json!({ "type": "string", "description": "Project name (as shown by list_projects) or the project's folder path" })
}

fn section_prop() -> Value {
    json!({ "type": "string", "description": "\"docs\" (default) or the folder name of an extra section such as \"tutorial\" (see get_project_context)" })
}

fn text_prop(desc: &str) -> Value {
    json!({
        "description": desc,
        "anyOf": [ { "type": "string" }, { "type": "array", "items": { "type": "string" } } ]
    })
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_projects",
            "description": "List all software projects registered in Project Manager, with folder, engine and latest activity.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_project_context",
            "description": "Get a full briefing of a project: metadata, the current state (STATE.md), the most recent work log entries and the list of documentation pages. Call this before starting work on a project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "recent_entries": { "type": "integer", "description": "How many recent work log entries to include (default 5)", "minimum": 0 }
                },
                "required": ["project"]
            }
        }),
        json!({
            "name": "list_worklog",
            "description": "List work log entries of a project, newest first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "limit": { "type": "integer", "description": "Maximum number of entries (default 20)", "minimum": 1 },
                    "include_bodies": { "type": "boolean", "description": "Include the full text of each entry (default false)" }
                },
                "required": ["project"]
            }
        }),
        json!({
            "name": "add_worklog_entry",
            "description": "Append a new entry to a project's work log. Do this after every meaningful unit of work and before you finish. Entries are never edited later, so be specific: name files, functions and decisions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "author": { "type": "string", "description": "Who did the work, e.g. your model name such as \"Claude Opus 5.5\"" },
                    "type": { "type": "string", "enum": ["feature", "fix", "refactor", "docs", "research", "setup", "other"] },
                    "title": { "type": "string", "description": "One-line summary of what was done" },
                    "summary": { "type": "string", "description": "What was done and why, 2-5 sentences" },
                    "changes": text_prop("Changed files and what changed, e.g. \"`src/save.rs` - added autosave\""),
                    "decisions": text_prop("Decisions made and the reasons, so later agents don't undo them"),
                    "issues": text_prop("Known bugs, limitations, unfinished work"),
                    "next_steps": text_prop("Concrete suggestions for whoever works next")
                },
                "required": ["project", "author", "type", "title", "summary"]
            }
        }),
        json!({
            "name": "update_state",
            "description": "Replace the project's STATE.md: a short (under ~60 lines) snapshot of where the project stands now: what it is, status, tech, open problems, next priorities. It is a snapshot, not a history; history belongs in the work log. Read the current state with get_project_context first and keep what is still true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "content": { "type": "string", "description": "The complete new Markdown content of STATE.md" }
                },
                "required": ["project", "content"]
            }
        }),
        json!({
            "name": "list_docs",
            "description": "List the pages of a project's documentation (or of an extra section such as the Tutorial) with their titles.",
            "inputSchema": { "type": "object", "properties": { "project": project_prop(), "section": section_prop() }, "required": ["project"] }
        }),
        json!({
            "name": "read_doc",
            "description": "Read one page of a project's documentation or of an extra section.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "section": section_prop(),
                    "name": { "type": "string", "description": "Page file name, e.g. \"overview.md\"" }
                },
                "required": ["project", "name"]
            }
        }),
        json!({
            "name": "write_doc",
            "description": "Create or overwrite a page in the project's documentation or in an extra section. One topic per page, lowercase-kebab-case file names (e.g. \"save-system.md\"; in extra sections with a two-digit prefix such as \"02-controls.md\"), starting with a \"# Title\" line. To change an existing page, read it first and send the full new content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project_prop(),
                    "section": section_prop(),
                    "name": { "type": "string", "description": "Page file name, e.g. \"save-system.md\"" },
                    "content": { "type": "string", "description": "The complete Markdown content of the page" }
                },
                "required": ["project", "name", "content"]
            }
        }),
    ]
}

fn call_tool(data_dir: &Path, params: &Value) -> Value {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match run_tool(data_dir, name, &args) {
        Ok(text) => json!({ "content": [ { "type": "text", "text": text } ], "isError": false }),
        Err(e) => json!({ "content": [ { "type": "text", "text": e } ], "isError": true }),
    }
}

// ---------- argument helpers ----------

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn req_str(args: &Value, key: &str) -> Result<String, String> {
    opt_str(args, key)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("Missing required argument: {key}"))
}

/// A string, or a list of strings turned into a Markdown bullet list.
fn text_arg(args: &Value, key: &str) -> String {
    match args.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str())
            .map(|s| {
                let s = s.trim();
                if s.starts_with("- ") || s.starts_with("* ") {
                    s.to_string()
                } else {
                    format!("- {s}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn usize_arg(args: &Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default)
}

fn project_arg(data_dir: &Path, args: &Value) -> Result<PathBuf, String> {
    let q = req_str(args, "project")?;
    let p = library::find_project(data_dir, &q)?;
    if !project::pm_dir(&p).join("project.json").is_file() {
        return Err(format!(
            "The project folder {} is missing or has no .projectmanager folder",
            p.display()
        ));
    }
    Ok(p)
}

// ---------- tools ----------

pub fn run_tool(data_dir: &Path, name: &str, args: &Value) -> Result<String, String> {
    match name {
        "list_projects" => list_projects(data_dir),
        "get_project_context" => {
            let p = project_arg(data_dir, args)?;
            // Keep the generated rules current (e.g. after tabs were added in the app).
            let _ = project::sync_agents_md(&p);
            context::project_context(&p, usize_arg(args, "recent_entries", 5))
        }
        "list_worklog" => {
            let p = project_arg(data_dir, args)?;
            let limit = usize_arg(args, "limit", 20).max(1);
            let bodies = args
                .get("include_bodies")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let log = project::list_worklog(&p)?;
            if log.is_empty() {
                return Ok("No work log entries yet.".into());
            }
            let mut out = format!("{} entries in total, newest first:\n\n", log.len());
            for e in log.iter().take(limit) {
                out.push_str(&format!(
                    "- {} | {} | {} | {} (worklog/{})\n",
                    e.date, e.author, e.kind, e.title, e.file
                ));
                if bodies {
                    out.push_str(&format!("\n{}\n", e.body.trim()));
                }
            }
            Ok(out)
        }
        "add_worklog_entry" => {
            let p = project_arg(data_dir, args)?;
            let entry = project::NewWorklogEntry {
                author: req_str(args, "author")?,
                kind: opt_str(args, "type").unwrap_or_else(|| "other".into()),
                title: req_str(args, "title")?,
                summary: req_str(args, "summary")?,
                changes: text_arg(args, "changes"),
                decisions: text_arg(args, "decisions"),
                issues: text_arg(args, "issues"),
                next_steps: text_arg(args, "next_steps"),
            };
            let e = project::add_worklog_entry(&p, entry)?;
            Ok(format!("Logged as .projectmanager/worklog/{}", e.file))
        }
        "update_state" => {
            let p = project_arg(data_dir, args)?;
            let content = req_str(args, "content")?;
            project::save_state(&p, &content)?;
            Ok("STATE.md updated.".into())
        }
        "list_docs" => {
            let p = project_arg(data_dir, args)?;
            let section = opt_str(args, "section").unwrap_or_default();
            let docs = project::list_pages(&p, &section)?;
            if docs.is_empty() {
                return Ok("No pages yet.".into());
            }
            Ok(docs
                .iter()
                .map(|d| {
                    let c = project::read_page(&p, &section, d).unwrap_or_default();
                    format!("- {}: {}", d, project::doc_title(d, &c))
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "read_doc" => {
            let p = project_arg(data_dir, args)?;
            let section = opt_str(args, "section").unwrap_or_default();
            let n = project::normalize_doc_name(&req_str(args, "name")?)?;
            project::read_page(&p, &section, &n)
        }
        "write_doc" => {
            let p = project_arg(data_dir, args)?;
            let section = opt_str(args, "section").unwrap_or_default();
            let n = project::normalize_doc_name(&req_str(args, "name")?)?;
            let content = req_str(args, "content")?;
            project::save_page(&p, &section, &n, &content)?;
            let folder = if section.trim().is_empty() {
                "docs"
            } else {
                section.trim()
            };
            Ok(format!("Saved .projectmanager/{folder}/{n}"))
        }
        other => Err(format!("Unknown tool: {other}")),
    }
}

fn list_projects(data_dir: &Path) -> Result<String, String> {
    let lib = library::load_library(data_dir);
    if lib.projects.is_empty() {
        return Ok(
            "No projects registered yet. The user can add projects in the Project Manager app."
                .into(),
        );
    }
    let mut out = String::new();
    for path in &lib.projects {
        let dir = Path::new(path);
        // Read only meta and log here (project::summary would also load the cover image).
        let meta = match project::read_meta(dir) {
            Ok(m) => m,
            Err(_) => {
                out.push_str(&format!("- `{path}`: folder or .projectmanager missing\n"));
                continue;
            }
        };
        let mut line = format!("- **{}** (`{}`)", meta.name, path);
        if meta.engine != "none" && !meta.engine.is_empty() {
            line.push_str(&format!(" | {}", meta.engine));
        }
        if let Some(e) = project::list_worklog(dir)
            .ok()
            .and_then(|v| v.into_iter().next())
        {
            line.push_str(&format!(" | last activity {}: {}", e.date, e.title));
        }
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}
