//! Everything stored inside a project's `.projectmanager/` folder:
//! metadata, current state, docs, the work log and the cover image.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::templates;

pub const PM_DIR: &str = ".projectmanager";
const COVER_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
const MAX_COVER_BYTES: u64 = 20 * 1024 * 1024;

fn default_schema() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// "unity", "unreal", "godot" or "none"
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// File name of the cover inside `.projectmanager/`, e.g. "cover.png"
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    /// Extra tabs shown on the project page, e.g. a Tutorial.
    #[serde(default)]
    pub tabs: Vec<CustomTab>,
}

/// An extra tab of a project. Its pages live in `.projectmanager/<id>/`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CustomTab {
    /// Folder name and identifier, e.g. "tutorial".
    pub id: String,
    pub title: String,
    /// What AI agents should keep in this tab (copied into AGENTS.md).
    #[serde(default)]
    pub instructions: String,
    /// The preset it was created from ("tutorial", "design", "changelog", "custom").
    #[serde(default)]
    pub preset: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorklogEntry {
    pub file: String,
    /// The entry's time. If the entry claimed a time later than the file was
    /// actually written, this is the file's time instead (see `date_corrected`).
    pub date: String,
    pub author: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    /// True if `date` was replaced by the file's real creation time.
    pub date_corrected: bool,
    /// The time written in the entry, when it was corrected.
    pub claimed_date: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewWorklogEntry {
    pub author: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub changes: String,
    #[serde(default)]
    pub decisions: String,
    #[serde(default)]
    pub issues: String,
    #[serde(default)]
    pub next_steps: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub path: String,
    pub meta: ProjectMeta,
    pub cover_url: Option<String>,
    pub last_entry: Option<WorklogEntry>,
    pub missing: bool,
}

// ---------- helpers ----------

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub fn pm_dir(project: &Path) -> PathBuf {
    project.join(PM_DIR)
}

fn now_iso() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    let out: String = out.chars().take(40).collect();
    if out.is_empty() {
        "entry".into()
    } else {
        out
    }
}

/// Only allow plain `something.md` names inside docs/ (no paths, no traversal).
pub fn safe_doc_name(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.to_ascii_lowercase().ends_with(".md")
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.contains(':');
    if ok {
        Ok(name.to_string())
    } else {
        Err(format!("Invalid document name: {name}"))
    }
}

/// Accepts "save-system" or "save-system.md" and returns a safe "save-system.md".
pub fn normalize_doc_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.to_ascii_lowercase().ends_with(".md") {
        safe_doc_name(name)
    } else {
        safe_doc_name(&format!("{name}.md"))
    }
}

/// The first `# Heading` of a Markdown page, or the file name without `.md`.
pub fn doc_title(name: &str, content: &str) -> String {
    content
        .lines()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| name.trim_end_matches(".md").replace(['-', '_'], " "))
}

/// Splits `---` front matter (simple `key: value` lines) from the Markdown body.
pub fn parse_frontmatter(text: &str) -> (HashMap<String, String>, String) {
    let text = text.replace("\r\n", "\n");
    let text = text.trim_start_matches('\u{feff}');
    let mut map = HashMap::new();
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let fm = &rest[..end];
            let after = &rest[end + 4..];
            let body = match after.find('\n') {
                Some(i) => &after[i + 1..],
                None => "",
            };
            for line in fm.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    map.insert(k.trim().to_lowercase(), v);
                }
            }
            return (map, body.trim_start_matches('\n').to_string());
        }
    }
    (map, text.to_string())
}

pub fn detect_engine(project: &Path) -> String {
    if project
        .join("ProjectSettings")
        .join("ProjectVersion.txt")
        .is_file()
    {
        "unity".into()
    } else if find_file_with_ext(project, "uproject").is_some() {
        "unreal".into()
    } else if project.join("project.godot").is_file() {
        "godot".into()
    } else {
        "none".into()
    }
}

pub fn find_file_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file()
            && p.extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
        {
            return Some(p);
        }
    }
    None
}

// ---------- metadata ----------

pub fn read_meta(project: &Path) -> Result<ProjectMeta, String> {
    let text = fs::read_to_string(pm_dir(project).join("project.json")).map_err(err)?;
    serde_json::from_str(&text).map_err(|e| format!("project.json is invalid: {e}"))
}

pub fn write_meta(project: &Path, meta: &ProjectMeta) -> Result<(), String> {
    let text = serde_json::to_string_pretty(meta).map_err(err)?;
    fs::write(pm_dir(project).join("project.json"), text).map_err(err)
}

/// Creates `.projectmanager/` with templates if it does not exist yet.
pub fn init_project(project: &Path) -> Result<(), String> {
    if !project.is_dir() {
        return Err(format!("Folder not found: {}", project.display()));
    }
    let dir = pm_dir(project);
    fs::create_dir_all(dir.join("docs")).map_err(err)?;
    fs::create_dir_all(dir.join("worklog")).map_err(err)?;

    let fresh = !dir.join("project.json").exists();
    if fresh {
        let name = project
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string();
        let engine = detect_engine(project);
        let meta = ProjectMeta {
            name: name.clone(),
            description: String::new(),
            engine: engine.clone(),
            tags: vec![],
            cover: None,
            created: now_iso(),
            schema_version: 1,
            tabs: vec![],
        };
        write_meta(project, &meta)?;
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        write_if_missing(
            &dir.join("STATE.md"),
            &templates::state_md(&name, &engine, &date),
        )?;
        write_if_missing(
            &dir.join("docs").join("overview.md"),
            templates::OVERVIEW_MD,
        )?;
        add_worklog_entry(
            project,
            NewWorklogEntry {
                author: "Project Manager".into(),
                kind: "setup".into(),
                title: "Project registered in Project Manager".into(),
                summary: format!(
                    "Created the `.projectmanager/` folder with STATE.md, AGENTS.md, docs/ and worklog/. Detected engine: {engine}."
                ),
                changes: "- `.projectmanager/` - created".into(),
                decisions: String::new(),
                issues: String::new(),
                next_steps: "- Fill in STATE.md and docs/overview.md".into(),
            },
        )?;
    }
    // AGENTS.md is always (re)written if missing so agents know the rules.
    sync_agents_md(project)?;
    Ok(())
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
    if !path.exists() {
        fs::write(path, content).map_err(err)?;
    }
    Ok(())
}

// ---------- summary ----------

pub fn summary(project: &Path) -> ProjectSummary {
    let path = project.to_string_lossy().to_string();
    if !pm_dir(project).join("project.json").is_file() {
        let name = project
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Missing project")
            .to_string();
        return ProjectSummary {
            path,
            meta: ProjectMeta {
                name,
                description: String::new(),
                engine: "none".into(),
                tags: vec![],
                cover: None,
                created: String::new(),
                schema_version: 1,
                tabs: vec![],
            },
            cover_url: None,
            last_entry: None,
            missing: true,
        };
    }
    let meta = read_meta(project).unwrap_or_else(|_| ProjectMeta {
        name: "(broken project.json)".into(),
        description: String::new(),
        engine: "none".into(),
        tags: vec![],
        cover: None,
        created: String::new(),
        schema_version: 1,
        tabs: vec![],
    });
    let cover_url = meta
        .cover
        .as_ref()
        .and_then(|c| cover_data_url(project, c).ok());
    let last_entry = list_worklog(project)
        .ok()
        .and_then(|v| v.into_iter().next());
    ProjectSummary {
        path,
        meta,
        cover_url,
        last_entry,
        missing: false,
    }
}

// ---------- state ----------

pub fn read_state(project: &Path) -> String {
    fs::read_to_string(pm_dir(project).join("STATE.md")).unwrap_or_default()
}

pub fn save_state(project: &Path, content: &str) -> Result<(), String> {
    fs::write(pm_dir(project).join("STATE.md"), content).map_err(err)
}

// ---------- docs and extra sections ----------

pub const DOCS_SECTION: &str = "docs";
const RESERVED_SECTIONS: &[&str] = &["docs", "worklog"];

/// Tab ids are folder names: lowercase letters, digits and dashes.
pub fn valid_tab_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !RESERVED_SECTIONS.contains(&id)
}

/// The folder of a section: "docs" or the id of one of the project's tabs.
pub fn section_dir(project: &Path, section: &str) -> Result<PathBuf, String> {
    let section = section.trim();
    if section.is_empty() || section == DOCS_SECTION {
        return Ok(pm_dir(project).join(DOCS_SECTION));
    }
    let meta = read_meta(project)?;
    if valid_tab_id(section) && meta.tabs.iter().any(|t| t.id == section) {
        return Ok(pm_dir(project).join(section));
    }
    let known: Vec<&str> = std::iter::once(DOCS_SECTION)
        .chain(meta.tabs.iter().map(|t| t.id.as_str()))
        .collect();
    Err(format!(
        "Unknown section \"{section}\". Sections of this project: {}",
        known.join(", ")
    ))
}

pub fn list_pages(project: &Path, section: &str) -> Result<Vec<String>, String> {
    let dir = section_dir(project, section)?;
    let mut out = vec![];
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    if n.to_ascii_lowercase().ends_with(".md") {
                        out.push(n.to_string());
                    }
                }
            }
        }
    }
    // overview.md first, then alphabetical (numbered pages sort by their prefix)
    out.sort_by(|a, b| {
        let ao = a.eq_ignore_ascii_case("overview.md");
        let bo = b.eq_ignore_ascii_case("overview.md");
        bo.cmp(&ao).then(a.to_lowercase().cmp(&b.to_lowercase()))
    });
    Ok(out)
}

pub fn read_page(project: &Path, section: &str, name: &str) -> Result<String, String> {
    let name = safe_doc_name(name)?;
    fs::read_to_string(section_dir(project, section)?.join(name)).map_err(err)
}

pub fn save_page(project: &Path, section: &str, name: &str, content: &str) -> Result<(), String> {
    let name = safe_doc_name(name)?;
    let dir = section_dir(project, section)?;
    fs::create_dir_all(&dir).map_err(err)?;
    fs::write(dir.join(name), content).map_err(err)
}

/// The two-digit prefix of a page name ("03-cards.md" -> 3).
fn page_number(name: &str) -> Option<u32> {
    let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Creates a page with a `# Title` line. In extra sections the file name gets the
/// next two-digit prefix so pages keep their reading order.
pub fn create_page(project: &Path, section: &str, title: &str) -> Result<String, String> {
    let dir = section_dir(project, section)?;
    fs::create_dir_all(&dir).map_err(err)?;
    let base = slugify(title);
    let numbered = !(section.trim().is_empty() || section.trim() == DOCS_SECTION);
    let make = |suffix: &str| -> String {
        if numbered {
            let next = list_pages(project, section)
                .unwrap_or_default()
                .iter()
                .filter_map(|n| page_number(n))
                .max()
                .unwrap_or(0)
                + 1;
            format!("{next:02}-{base}{suffix}.md")
        } else {
            format!("{base}{suffix}.md")
        }
    };
    let mut name = make("");
    let mut i = 2;
    while dir.join(&name).exists() {
        name = make(&format!("-{i}"));
        i += 1;
    }
    fs::write(dir.join(&name), format!("# {}\n\n", title.trim())).map_err(err)?;
    Ok(name)
}

pub fn list_docs(project: &Path) -> Result<Vec<String>, String> {
    list_pages(project, DOCS_SECTION)
}

pub fn read_doc(project: &Path, name: &str) -> Result<String, String> {
    read_page(project, DOCS_SECTION, name)
}

pub fn save_doc(project: &Path, name: &str, content: &str) -> Result<(), String> {
    save_page(project, DOCS_SECTION, name, content)
}

pub fn create_doc(project: &Path, title: &str) -> Result<String, String> {
    create_page(project, DOCS_SECTION, title)
}

// ---------- extra tabs ----------

pub fn add_tab(
    project: &Path,
    title: &str,
    preset: &str,
    instructions: &str,
) -> Result<CustomTab, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("The tab needs a name".into());
    }
    if title.chars().count() > 40 {
        return Err("The tab name is too long (max. 40 characters)".into());
    }
    let mut meta = read_meta(project)?;
    let mut base = slugify(title);
    if !valid_tab_id(&base) {
        base = format!("tab-{base}");
    }
    let mut id = base.clone();
    let mut i = 2;
    while meta.tabs.iter().any(|t| t.id == id) {
        id = format!("{base}-{i}");
        i += 1;
    }
    let tab = CustomTab {
        id: id.clone(),
        title: title.to_string(),
        instructions: instructions.trim().to_string(),
        preset: preset.to_string(),
    };
    let dir = pm_dir(project).join(&id);
    fs::create_dir_all(&dir).map_err(err)?;
    let has_pages = fs::read_dir(&dir)
        .map(|r| {
            r.flatten()
                .any(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        })
        .unwrap_or(false);
    if !has_pages {
        let (name, content) = templates::starter_page(preset, title);
        fs::write(dir.join(name), content).map_err(err)?;
    }
    meta.tabs.push(tab.clone());
    write_meta(project, &meta)?;
    sync_agents_md(project)?;
    let next = if preset == "tutorial" {
        "- Write the tutorial from the current state of the game, following the Tutorial section in AGENTS.md".to_string()
    } else {
        format!("- Fill in the {title} pages, following the \"{title}\" section in AGENTS.md")
    };
    add_worklog_entry(
        project,
        NewWorklogEntry {
            author: "Project Manager".into(),
            kind: "setup".into(),
            title: format!("Added the \"{title}\" tab"),
            summary: format!(
                "The project now has a \"{title}\" section in `.projectmanager/{id}/`. Its instructions were added to `.projectmanager/AGENTS.md`, so AI agents keep it up to date from now on."
            ),
            changes: format!("- `.projectmanager/{id}/` - created\n- `.projectmanager/AGENTS.md` - instructions for the new section"),
            decisions: String::new(),
            issues: String::new(),
            next_steps: next,
        },
    )?;
    Ok(tab)
}

pub fn update_tab(
    project: &Path,
    id: &str,
    title: &str,
    instructions: &str,
) -> Result<CustomTab, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("The tab needs a name".into());
    }
    let mut meta = read_meta(project)?;
    let tab = meta
        .tabs
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("Tab not found")?;
    tab.title = title.to_string();
    tab.instructions = instructions.trim().to_string();
    let updated = tab.clone();
    write_meta(project, &meta)?;
    sync_agents_md(project)?;
    Ok(updated)
}

/// Removes the tab from the project page and from AGENTS.md. Its pages stay on disk.
pub fn remove_tab(project: &Path, id: &str) -> Result<(), String> {
    let mut meta = read_meta(project)?;
    let before = meta.tabs.len();
    meta.tabs.retain(|t| t.id != id);
    if meta.tabs.len() == before {
        return Err("Tab not found".into());
    }
    write_meta(project, &meta)?;
    sync_agents_md(project)?;
    Ok(())
}

// ---------- AGENTS.md (generated rules + project-specific rules) ----------

fn project_rules_section(custom: &str) -> String {
    let custom = custom.trim();
    let body = if custom.is_empty() {
        templates::PROJECT_RULES_PLACEHOLDER
    } else {
        custom
    };
    format!("\n{}\n\n{}\n", templates::PROJECT_RULES_HEADING, body)
}

fn has_project_rules(after: &str) -> bool {
    after.lines().any(|l| {
        let t = l.trim();
        !t.is_empty()
            && t != templates::PROJECT_RULES_HEADING
            && t != templates::PROJECT_RULES_PLACEHOLDER
    })
}

/// Splits a marked AGENTS.md into (text before the block, text after the block).
pub fn split_rules(text: &str) -> Option<(String, String)> {
    let s = text.find(templates::RULES_START)?;
    let rel = text[s..].find(templates::RULES_END)?;
    let e = s + rel + templates::RULES_END.len();
    let after = &text[e..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    Some((text[..s].to_string(), after.to_string()))
}

/// Lines of an old, unmarked AGENTS.md that are not part of the old template,
/// i.e. rules someone added for this project.
pub fn legacy_custom_lines(text: &str) -> String {
    let template: std::collections::HashSet<&str> = templates::LEGACY_AGENTS_MD
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty())
        .collect();
    text.lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty() && !template.contains(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The full new AGENTS.md text for `existing` content (None = no file yet).
/// Returns (text, legacy custom rules found).
pub fn render_agents_md(existing: Option<&str>, tabs: &[CustomTab]) -> (String, bool) {
    let (before, after, migrated) = match existing.map(|t| t.replace("\r\n", "\n")) {
        None => (String::new(), project_rules_section(""), false),
        Some(text) => match split_rules(&text) {
            Some((b, a)) => (b, a, false),
            None => {
                let custom = legacy_custom_lines(&text);
                let migrated = !custom.is_empty();
                (String::new(), project_rules_section(&custom), migrated)
            }
        },
    };
    let rules = templates::agents_rules(tabs, has_project_rules(&after));
    (format!("{before}{rules}{after}"), migrated)
}

/// Rewrites the generated part of `.projectmanager/AGENTS.md` (for example after a tab
/// was added), keeping project-specific rules, and refreshes the rules copy in an AI
/// folder if the root AGENTS.md points there. Old files without markers are converted;
/// if they contain added rules, a backup is saved first. Returns true if a file changed.
pub fn sync_agents_md(project: &Path) -> Result<bool, String> {
    let dir = pm_dir(project);
    if !dir.join("project.json").is_file() {
        return Ok(false);
    }
    let meta = read_meta(project)?;
    let path = dir.join("AGENTS.md");
    let existing = fs::read_to_string(&path).ok();
    let (text, migrated) = render_agents_md(existing.as_deref(), &meta.tabs);
    let mut changed = false;
    let unchanged = existing
        .as_deref()
        .map(|e| e.replace("\r\n", "\n") == text)
        .unwrap_or(false);
    if !unchanged {
        if migrated {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            fs::copy(&path, dir.join(format!("AGENTS.backup-{stamp}.md"))).map_err(err)?;
        }
        fs::write(&path, &text).map_err(err)?;
        changed = true;
    }
    if let Some(target) = current_pointer_target(project) {
        if target != PM_DIR
            && AI_FOLDERS.contains(&target.as_str())
            && project.join(&target).is_dir()
        {
            let f = project.join(&target).join("project-manager.md");
            let content = templates::ai_folder_rules(&text);
            if fs::read_to_string(&f).ok().as_deref() != Some(content.as_str()) {
                fs::write(&f, &content).map_err(err)?;
                changed = true;
            }
        }
    }
    Ok(changed)
}

// ---------- worklog ----------

/// Allowed difference between an entry's claimed time and the file's real time.
const CLOCK_TOLERANCE_SECS: i64 = 120;

fn iso(d: &chrono::DateTime<chrono::FixedOffset>) -> String {
    d.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// Parses the `date` of a worklog entry. Times without an offset count as local time.
pub fn parse_entry_date(s: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    use chrono::{Offset, TimeZone};
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(d);
    }
    for f in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, f) {
            let local = chrono::Local.from_local_datetime(&n).earliest()?;
            return Some(local.with_timezone(&local.offset().fix()));
        }
    }
    None
}

/// When the file was really written: the earlier of its creation and modification time.
fn file_time(p: &Path) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    use chrono::Offset;
    let m = fs::metadata(p).ok()?;
    let t = [m.created().ok(), m.modified().ok()]
        .into_iter()
        .flatten()
        .min()?;
    let local = chrono::DateTime::<chrono::Local>::from(t);
    Some(local.with_timezone(&local.offset().fix()))
}

/// Decides which time to show for an entry: (date, corrected, sort key).
/// An entry cannot have been written before the file existed, so a claimed time later
/// than the file's real time (beyond a small tolerance) was guessed and is replaced.
pub fn resolve_entry_time(
    claimed: &str,
    file: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> (String, bool, i64) {
    match (parse_entry_date(claimed), file) {
        (Some(d), Some(f)) if d.timestamp() > f.timestamp() + CLOCK_TOLERANCE_SECS => {
            (iso(&f), true, f.timestamp())
        }
        (Some(d), _) => (iso(&d), false, d.timestamp()),
        (None, Some(f)) => (iso(&f), !claimed.trim().is_empty(), f.timestamp()),
        (None, None) => (claimed.to_string(), false, 0),
    }
}

pub fn list_worklog(project: &Path) -> Result<Vec<WorklogEntry>, String> {
    let dir = pm_dir(project).join("worklog");
    let mut out: Vec<(i64, WorklogEntry)> = vec![];
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            let is_md = p
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !p.is_file() || !is_md {
                continue;
            }
            let file = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let text = fs::read_to_string(&p).unwrap_or_default();
            let (fm, body) = parse_frontmatter(&text);
            let get = |k: &str| fm.get(k).cloned().unwrap_or_default();
            let title = {
                let t = get("title");
                if t.is_empty() {
                    file.trim_end_matches(".md").to_string()
                } else {
                    t
                }
            };
            let claimed = get("date");
            let (date, corrected, key) = resolve_entry_time(&claimed, file_time(&p));
            let kind = {
                let k = get("type");
                if k.is_empty() {
                    "other".into()
                } else {
                    k
                }
            };
            out.push((
                key,
                WorklogEntry {
                    date,
                    author: get("author"),
                    kind,
                    title,
                    body,
                    date_corrected: corrected,
                    claimed_date: if corrected { claimed } else { String::new() },
                    file,
                },
            ));
        }
    }
    // Newest first; file names (which start with a timestamp) break ties.
    out.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.file.cmp(&a.1.file)));
    Ok(out.into_iter().map(|(_, e)| e).collect())
}

fn section(title: &str, content: &str) -> String {
    let c = content.trim();
    let c = if c.is_empty() { "- none" } else { c };
    format!("## {title}\n{c}\n\n")
}

pub fn add_worklog_entry(project: &Path, e: NewWorklogEntry) -> Result<WorklogEntry, String> {
    let dir = pm_dir(project).join("worklog");
    fs::create_dir_all(&dir).map_err(err)?;
    let now = chrono::Local::now();
    let author = if e.author.trim().is_empty() {
        "Unknown".to_string()
    } else {
        e.author.trim().to_string()
    };
    let title = e.title.trim().replace('\n', " ");
    let kind = if e.kind.trim().is_empty() {
        "other".to_string()
    } else {
        e.kind.trim().to_string()
    };
    let date = now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();

    let mut file = format!("{}_{}.md", now.format("%Y-%m-%d_%H%M%S"), slugify(&author));
    let mut i = 2;
    while dir.join(&file).exists() {
        file = format!(
            "{}_{}-{i}.md",
            now.format("%Y-%m-%d_%H%M%S"),
            slugify(&author)
        );
        i += 1;
    }

    let body = format!(
        "{}{}{}{}{}",
        section("Summary", &e.summary),
        section("Changes", &e.changes),
        section("Decisions", &e.decisions),
        section("Known issues", &e.issues),
        section("Next steps", &e.next_steps),
    );
    let text =
        format!("---\ndate: {date}\nauthor: {author}\ntype: {kind}\ntitle: {title}\n---\n\n{body}");
    fs::write(dir.join(&file), text).map_err(err)?;
    Ok(WorklogEntry {
        file,
        date,
        author,
        kind,
        title,
        body,
        date_corrected: false,
        claimed_date: String::new(),
    })
}

// ---------- cover ----------

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

pub fn cover_data_url(project: &Path, cover: &str) -> Result<String, String> {
    if cover.contains('/') || cover.contains('\\') || cover.contains("..") {
        return Err("invalid cover name".into());
    }
    let path = pm_dir(project).join(cover);
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = fs::read(&path).map_err(err)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{};base64,{}", mime_for(&ext), b64))
}

pub fn set_cover(project: &Path, source: &Path) -> Result<String, String> {
    let ext = source
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !COVER_EXTS.contains(&ext.as_str()) {
        return Err("Cover must be a PNG, JPG, WEBP or GIF image".into());
    }
    let size = fs::metadata(source).map_err(err)?.len();
    if size > MAX_COVER_BYTES {
        return Err("Cover image is larger than 20 MB".into());
    }
    let dir = pm_dir(project);
    // Remove older covers with a different extension so only one remains.
    for e in COVER_EXTS {
        let old = dir.join(format!("cover.{e}"));
        if e != ext && old.exists() {
            let _ = fs::remove_file(old);
        }
    }
    let name = format!("cover.{ext}");
    fs::copy(source, dir.join(&name)).map_err(err)?;
    let mut meta = read_meta(project)?;
    meta.cover = Some(name.clone());
    write_meta(project, &meta)?;
    cover_data_url(project, &name)
}

// ---------- AI pointer files in the project root ----------

/// Folder names that AI coding tools create in a project.
pub const AI_FOLDERS: &[&str] = &[
    ".claude",
    ".cursor",
    ".windsurf",
    ".gemini",
    ".codex",
    ".junie",
    ".kiro",
    ".roo",
    ".clinerules",
    ".continue",
    ".amazonq",
    ".agents",
    ".agent",
    ".ai",
];

/// The AI folders that exist in this project, in the order of `AI_FOLDERS`.
pub fn detect_ai_folders(project: &Path) -> Vec<String> {
    AI_FOLDERS
        .iter()
        .filter(|f| project.join(f).is_dir())
        .map(|f| f.to_string())
        .collect()
}

/// Inserts or replaces the Project Manager block in the text of a root
/// AGENTS.md / CLAUDE.md. Returns the new text.
pub fn replace_pointer(text: &str, block: &str) -> String {
    let start = templates::POINTER_START;
    let end = templates::POINTER_END;
    if let Some(s) = text.find(start) {
        if let Some(rel) = text[s..].find(end) {
            let mut e = s + rel + end.len();
            if text[e..].starts_with("\r\n") {
                e += 2;
            } else if text[e..].starts_with('\n') {
                e += 1;
            }
            return format!("{}{}{}", &text[..s], block, &text[e..]);
        }
    }
    let legacy = templates::LEGACY_ROOT_POINTER;
    let normalized = text.replace("\r\n", "\n");
    if normalized.contains(legacy) {
        return normalized.replacen(legacy, block.trim_end(), 1);
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    out
}

/// Makes the project's root AGENTS.md and CLAUDE.md point AI agents to the rules.
///
/// - `target` None or ".projectmanager": they point to `.projectmanager/AGENTS.md`.
/// - `target` an existing AI folder such as ".claude": the rules are written to
///   `.claude/project-manager.md` and the root files point there.
///
/// Root files are created if missing; an earlier Project Manager block is replaced,
/// everything else in them is kept.
pub fn install_ai_pointer(project: &Path, target: Option<&str>) -> Result<Vec<String>, String> {
    let mut changed = vec![];
    let rules_path = match target {
        None | Some("") | Some(PM_DIR) => format!("{PM_DIR}/AGENTS.md"),
        Some(folder) => {
            if !AI_FOLDERS.contains(&folder) {
                return Err(format!("{folder} is not a known AI folder"));
            }
            let dir = project.join(folder);
            if !dir.is_dir() {
                return Err(format!("{folder} does not exist in this project"));
            }
            let file = dir.join("project-manager.md");
            sync_agents_md(project)?;
            let agents = fs::read_to_string(pm_dir(project).join("AGENTS.md"))
                .map_err(err)?
                .replace("\r\n", "\n");
            let content = templates::ai_folder_rules(&agents);
            if fs::read_to_string(&file).ok().as_deref() != Some(content.as_str()) {
                fs::write(&file, &content).map_err(err)?;
                changed.push(format!("{folder}/project-manager.md"));
            }
            format!("{folder}/project-manager.md")
        }
    };
    let block = templates::root_pointer(&rules_path);
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let p = project.join(name);
        if p.exists() {
            let text = fs::read_to_string(&p).map_err(err)?;
            let new_text = replace_pointer(&text, &block);
            if new_text != text {
                fs::write(&p, new_text).map_err(err)?;
                changed.push(format!("{name} (updated)"));
            }
        } else {
            fs::write(&p, format!("# {name}\n\n{block}")).map_err(err)?;
            changed.push(format!("{name} (created)"));
        }
    }
    Ok(changed)
}

/// Where the root AGENTS.md currently points, if it has a Project Manager block.
pub fn current_pointer_target(project: &Path) -> Option<String> {
    let text = fs::read_to_string(project.join("AGENTS.md")).ok()?;
    let s = text.find(templates::POINTER_START)?;
    let block = &text[s..];
    let i = block.find("read `")? + 6;
    let j = block[i..].find('`')?;
    let path = &block[i..i + j];
    Some(path.split('/').next().unwrap_or(path).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parses() {
        let (fm, body) = parse_frontmatter(
            "---\r\ndate: 2026-01-01\r\ntitle: \"Hi: there\"\r\n---\r\n\r\n## Summary\r\nx\r\n",
        );
        assert_eq!(fm.get("date").unwrap(), "2026-01-01");
        assert_eq!(fm.get("title").unwrap(), "Hi: there");
        assert!(body.starts_with("## Summary"));
    }

    #[test]
    fn slug() {
        assert_eq!(slugify("Claude Opus 5.5"), "claude-opus-5-5");
        assert_eq!(slugify("***"), "entry");
    }

    #[test]
    fn doc_names() {
        assert!(safe_doc_name("a.md").is_ok());
        assert!(safe_doc_name("../a.md").is_err());
        assert!(safe_doc_name("C:a.md").is_err());
    }
}
