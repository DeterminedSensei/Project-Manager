//! `pm` - command-line tool and MCP server for Project Manager.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pm_core::{connect, context, library, mcp, project};

const HELP: &str = r#"pm - Project Manager command-line tool

USAGE
  pm list                          List registered projects
  pm context [PROJECT] [--entries N]
                                   Print a briefing: state, recent work log, docs
  pm log [PROJECT] --author NAME --type TYPE --title TEXT --summary TEXT
         [--changes TEXT] [--decisions TEXT] [--issues TEXT] [--next TEXT]
                                   Add a work log entry
  pm add FOLDER                    Register a folder in the library
  pm mcp                           Run the MCP server (stdio) for AI assistants
  pm connect CLIENT                Register this pm.exe with an AI assistant
  pm disconnect CLIENT             Remove that registration
  pm status                        Show which assistants are connected
  pm help                          Show this help

PROJECT is a project name or folder path. Without it, pm uses the project that
contains the current directory.
CLIENT is claude-desktop or claude-code.
TYPE is one of: feature, fix, refactor, docs, research, setup, other.
In TEXT values, "\n" starts a new line.
"#;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn data_dir() -> Result<PathBuf, String> {
    library::default_data_dir()
        .ok_or_else(|| "Could not find the app data folder (%APPDATA%)".to_string())
}

/// Splits arguments into positional values and `--key value` options.
fn parse(args: &[String]) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    let mut pos = vec![];
    let mut opts = vec![];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(key) = a.strip_prefix("--") {
            let val = args
                .get(i + 1)
                .ok_or_else(|| format!("--{key} needs a value"))?;
            opts.push((key.to_string(), val.replace("\\n", "\n")));
            i += 2;
        } else {
            pos.push(a.clone());
            i += 1;
        }
    }
    Ok((pos, opts))
}

fn opt(opts: &[(String, String)], key: &str) -> Option<String> {
    opts.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

fn resolve_project(arg: Option<&String>) -> Result<PathBuf, String> {
    match arg {
        Some(q) => {
            // An existing folder with .projectmanager works directly; otherwise look up the library.
            let p = Path::new(q);
            if project::pm_dir(p).join("project.json").is_file() {
                Ok(p.to_path_buf())
            } else {
                library::find_project(&data_dir()?, q)
            }
        }
        None => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            library::find_project_upwards(&cwd).ok_or_else(|| {
                "The current directory is not inside a Project Manager project. Pass a project name or path.".to_string()
            })
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    match cmd {
        "help" | "--help" | "-h" => {
            print!("{HELP}");
            Ok(())
        }
        "mcp" => mcp::run(data_dir()?).map_err(|e| e.to_string()),
        "list" => {
            let text = mcp::run_tool(&data_dir()?, "list_projects", &serde_json::json!({}))?;
            print!("{text}");
            Ok(())
        }
        "context" => {
            let (pos, opts) = parse(rest)?;
            let p = resolve_project(pos.first())?;
            let _ = project::sync_agents_md(&p);
            let n = opt(&opts, "entries")
                .and_then(|v| v.parse().ok())
                .unwrap_or(5);
            println!("{}", context::project_context(&p, n)?);
            Ok(())
        }
        "log" => {
            let (pos, opts) = parse(rest)?;
            let p = resolve_project(pos.first())?;
            let need = |k: &str| {
                opt(&opts, k)
                    .filter(|v| !v.trim().is_empty())
                    .ok_or_else(|| format!("--{k} is required"))
            };
            let entry = project::NewWorklogEntry {
                author: need("author")?,
                kind: opt(&opts, "type").unwrap_or_else(|| "other".into()),
                title: need("title")?,
                summary: need("summary")?,
                changes: opt(&opts, "changes").unwrap_or_default(),
                decisions: opt(&opts, "decisions").unwrap_or_default(),
                issues: opt(&opts, "issues").unwrap_or_default(),
                next_steps: opt(&opts, "next").unwrap_or_default(),
            };
            let e = project::add_worklog_entry(&p, entry)?;
            println!(
                "Logged as {}",
                project::pm_dir(&p).join("worklog").join(&e.file).display()
            );
            Ok(())
        }
        "add" => {
            let (pos, _) = parse(rest)?;
            let folder = pos.first().ok_or("pm add needs a folder path")?;
            let abs = std::fs::canonicalize(folder).map_err(|e| format!("{folder}: {e}"))?;
            // canonicalize returns \\?\C:\... on Windows; store the normal form.
            let s = abs
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string();
            library::add_project(&data_dir()?, &s)?;
            println!("Added {s}");
            Ok(())
        }
        "connect" | "disconnect" => {
            let client = rest
                .first()
                .ok_or("Name a client: claude-desktop or claude-code")?;
            let msg = if cmd == "connect" {
                let me = std::env::current_exe().map_err(|e| e.to_string())?;
                connect::connect(client, &me)?
            } else {
                connect::disconnect(client)?
            };
            println!("{msg}");
            Ok(())
        }
        "status" => {
            let me = std::env::current_exe().ok();
            for s in connect::all_status(me.as_deref()) {
                println!("{}: {}", s.name, s.detail);
            }
            Ok(())
        }
        other => Err(format!("Unknown command \"{other}\". Run \"pm help\".")),
    }
}
