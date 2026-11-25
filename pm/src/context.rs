//! Builds a single Markdown "briefing" of a project for AI agents:
//! metadata, STATE.md, the most recent worklog entries and the list of docs.

use std::path::Path;

use crate::project;

pub fn project_context(project_dir: &Path, recent_entries: usize) -> Result<String, String> {
    let meta = project::read_meta(project_dir)?;
    let mut out = String::new();

    out.push_str(&format!("# Project context: {}\n\n", meta.name));
    out.push_str(&format!("- Folder: `{}`\n", project_dir.display()));
    if meta.engine != "none" && !meta.engine.is_empty() {
        out.push_str(&format!("- Engine: {}\n", meta.engine));
    }
    if !meta.tags.is_empty() {
        out.push_str(&format!("- Tags: {}\n", meta.tags.join(", ")));
    }
    if !meta.description.is_empty() {
        out.push_str(&format!("- Description: {}\n", meta.description));
    }
    out.push_str(
        "- Rules for agents: `.projectmanager/AGENTS.md` (log every work session; keep STATE.md current; document new features in docs/; keep extra sections up to date; follow its project-specific rules)\n\n",
    );

    out.push_str("## Current state (STATE.md)\n\n");
    let state = project::read_state(project_dir);
    if state.trim().is_empty() {
        out.push_str("_STATE.md is empty._\n\n");
    } else {
        // Demote headings so the briefing keeps one clear structure.
        for line in state.replace("\r\n", "\n").lines() {
            if line.starts_with('#') {
                out.push_str("##");
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    let log = project::list_worklog(project_dir)?;
    out.push_str(&format!(
        "## Recent work log (newest first, {} of {})\n\n",
        log.len().min(recent_entries),
        log.len()
    ));
    if log.is_empty() {
        out.push_str("_No entries yet._\n\n");
    }
    for e in log.iter().take(recent_entries) {
        out.push_str(&format!(
            "### {} | {} | {} | {}\n\n",
            e.date, e.author, e.kind, e.title
        ));
        for line in e.body.lines() {
            if line.starts_with('#') {
                out.push_str("##");
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    if log.len() > recent_entries {
        out.push_str("_Older entries: use list_worklog, or read `.projectmanager/worklog/`._\n\n");
    }

    out.push_str("## Documentation pages (.projectmanager/docs/)\n\n");
    let docs = project::list_docs(project_dir)?;
    if docs.is_empty() {
        out.push_str("_None yet._\n");
    }
    for d in docs {
        let content = project::read_doc(project_dir, &d).unwrap_or_default();
        out.push_str(&format!(
            "- `{}`: {}\n",
            d,
            project::doc_title(&d, &content)
        ));
    }

    for tab in &meta.tabs {
        out.push_str(&format!(
            "\n## Extra section: {} (.projectmanager/{}/, section \"{}\")\n\n",
            tab.title, tab.id, tab.id
        ));
        out.push_str(
            "Keep this section up to date whenever your work affects it. Instructions:\n\n",
        );
        let instr = tab.instructions.trim();
        if instr.is_empty() {
            out.push_str("Keep the pages in this section accurate and up to date.\n");
        } else {
            out.push_str(instr);
            out.push('\n');
        }
        out.push_str("\nPages:\n");
        let pages = project::list_pages(project_dir, &tab.id).unwrap_or_default();
        if pages.is_empty() {
            out.push_str("- _none yet_\n");
        }
        for pg in pages {
            let content = project::read_page(project_dir, &tab.id, &pg).unwrap_or_default();
            out.push_str(&format!(
                "- `{}`: {}\n",
                pg,
                project::doc_title(&pg, &content)
            ));
        }
    }
    Ok(out)
}
