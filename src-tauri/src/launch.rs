//! Finding installed editors and opening a project in them.
//!
//! Safety: programs are always started directly with arguments passed as a list
//! (std::process::Command), never through a shell, so unusual folder names can't
//! inject commands.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::settings::Settings;
use pm_core::connect::find_claude_cli;
use pm_core::project::find_file_with_ext;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOption {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub hint: String,
    /// The program's own icon as a PNG data URL, when it could be read.
    pub icon: Option<String>,
}

fn icon_of(p: Option<&Path>) -> Option<String> {
    p.and_then(crate::icons::icon_data_url)
}

/// Windows' own explorer.exe (falls back to a bare name if SystemRoot is unset).
fn explorer_exe() -> PathBuf {
    env_path("SystemRoot")
        .map(|r| r.join("explorer.exe"))
        .unwrap_or_else(|| PathBuf::from("explorer.exe"))
}

/// Icon for the Terminal button: PowerShell, which is what runs in it.
/// (Windows Terminal's wt.exe is an app alias stub that only has a generic icon.)
fn terminal_icon_source() -> Option<PathBuf> {
    env_path("SystemRoot").map(|r| {
        r.join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    })
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTools {
    pub vscode: Option<String>,
    pub intellij: Option<String>,
    pub unity_editors_dir: Option<String>,
    pub unreal_editor: Option<String>,
    pub godot: Option<String>,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn existing(p: PathBuf) -> Option<PathBuf> {
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn configured(value: &str) -> Option<PathBuf> {
    let v = value.trim().trim_matches('"');
    if v.is_empty() {
        None
    } else {
        Some(PathBuf::from(v))
    }
}

pub fn find_vscode(s: &Settings) -> Option<PathBuf> {
    if let Some(p) = configured(&s.vscode) {
        return existing(p);
    }
    let mut candidates = vec![];
    if let Some(l) = env_path("LOCALAPPDATA") {
        candidates.push(
            l.join("Programs")
                .join("Microsoft VS Code")
                .join("Code.exe"),
        );
    }
    if let Some(pf) = env_path("ProgramFiles") {
        candidates.push(pf.join("Microsoft VS Code").join("Code.exe"));
    }
    candidates.into_iter().find(|p| p.exists())
}

/// Looks for `IntelliJ IDEA*` folders (Community or Ultimate) and picks the newest.
pub fn find_intellij(s: &Settings) -> Option<PathBuf> {
    if let Some(p) = configured(&s.intellij) {
        return existing(p);
    }
    let mut roots = vec![];
    if let Some(pf) = env_path("ProgramFiles") {
        roots.push(pf.join("JetBrains"));
    }
    if let Some(l) = env_path("LOCALAPPDATA") {
        roots.push(l.join("Programs"));
    }
    let mut found: Vec<PathBuf> = vec![];
    for root in roots {
        if let Ok(entries) = fs::read_dir(&root) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("IntelliJ IDEA") {
                    let exe = e.path().join("bin").join("idea64.exe");
                    if exe.exists() {
                        found.push(exe);
                    }
                }
            }
        }
    }
    found.sort();
    found.pop()
}

pub fn unity_editors_dir(s: &Settings) -> Option<PathBuf> {
    if let Some(p) = configured(&s.unity_editors_dir) {
        return existing(p);
    }
    env_path("ProgramFiles").and_then(|pf| existing(pf.join("Unity").join("Hub").join("Editor")))
}

pub fn find_unreal(s: &Settings) -> Option<PathBuf> {
    configured(&s.unreal_editor).and_then(existing)
}

pub fn find_godot(s: &Settings) -> Option<PathBuf> {
    configured(&s.godot).and_then(existing)
}

pub fn detect_tools(s: &Settings) -> DetectedTools {
    let show = |p: Option<PathBuf>| p.map(|p| p.to_string_lossy().to_string());
    DetectedTools {
        vscode: show(find_vscode(s)),
        intellij: show(find_intellij(s)),
        unity_editors_dir: show(unity_editors_dir(s)),
        unreal_editor: show(find_unreal(s)),
        godot: show(find_godot(s)),
    }
}

/// Reads `m_EditorVersion: 2022.3.10f1` from ProjectSettings/ProjectVersion.txt.
pub fn unity_version(project: &Path) -> Option<String> {
    let text =
        fs::read_to_string(project.join("ProjectSettings").join("ProjectVersion.txt")).ok()?;
    text.lines().find_map(|l| {
        l.trim()
            .strip_prefix("m_EditorVersion:")
            .map(|v| v.trim().to_string())
    })
}

fn unity_exe(project: &Path, s: &Settings) -> Result<PathBuf, String> {
    let version = unity_version(project)
        .ok_or("Could not read the Unity version from ProjectSettings/ProjectVersion.txt")?;
    let dir =
        unity_editors_dir(s).ok_or("Unity Hub editor folder not found. Set it in Settings.")?;
    let exe = dir.join(&version).join("Editor").join("Unity.exe");
    if exe.exists() {
        Ok(exe)
    } else {
        Err(format!(
            "Unity {version} is not installed (looked for {}). Install it with Unity Hub.",
            exe.display()
        ))
    }
}

pub fn launch_options(project: &Path, s: &Settings) -> Vec<LaunchOption> {
    let mut v = vec![];
    let claude = find_claude_cli();
    // The Claude buttons have their own styling, so no icon is read for them.
    v.push(LaunchOption {
        id: "claude".into(),
        label: "Start Claude".into(),
        available: claude.is_some(),
        icon: None,
        hint: if claude.is_some() {
            "Open a terminal in this folder and start Claude Code".into()
        } else {
            "Claude Code was not found on this PC - install it first".into()
        },
    });
    if claude.is_some() && pm_core::sessions::session_count(project) > 0 {
        v.push(LaunchOption {
            id: "claude-continue".into(),
            label: "Continue last session".into(),
            available: true,
            icon: None,
            hint: "Open a terminal here and continue the most recent Claude Code session".into(),
        });
    }
    let vs = find_vscode(s);
    v.push(LaunchOption {
        id: "vscode".into(),
        label: "VS Code".into(),
        available: vs.is_some(),
        icon: icon_of(vs.as_deref()),
        hint: if vs.is_some() {
            "Open folder in VS Code".into()
        } else {
            "VS Code not found - set the path in Settings".into()
        },
    });
    let ij = find_intellij(s);
    v.push(LaunchOption {
        id: "intellij".into(),
        label: "IntelliJ IDEA".into(),
        available: ij.is_some(),
        icon: icon_of(ij.as_deref()),
        hint: if ij.is_some() {
            "Open folder in IntelliJ IDEA".into()
        } else {
            "IntelliJ not found - set the path in Settings".into()
        },
    });
    if project
        .join("ProjectSettings")
        .join("ProjectVersion.txt")
        .is_file()
    {
        let r = unity_exe(project, s);
        v.push(LaunchOption {
            id: "unity".into(),
            label: "Unity".into(),
            available: r.is_ok(),
            icon: r.as_ref().ok().and_then(|p| crate::icons::icon_data_url(p)),
            hint: match r {
                Ok(_) => format!(
                    "Open in Unity {}",
                    unity_version(project).unwrap_or_default()
                ),
                Err(e) => e,
            },
        });
    }
    if let Some(up) = find_file_with_ext(project, "uproject") {
        let name = up
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        v.push(LaunchOption {
            id: "unreal".into(),
            label: "Unreal Engine".into(),
            available: true,
            // The configured editor's icon, or the icon Windows shows for .uproject files.
            icon: icon_of(find_unreal(s).as_deref()).or_else(|| icon_of(Some(&up))),
            hint: format!("Open {name}"),
        });
    }
    if project.join("project.godot").is_file() {
        let g = find_godot(s);
        v.push(LaunchOption {
            id: "godot".into(),
            label: "Godot".into(),
            available: g.is_some(),
            icon: icon_of(g.as_deref()),
            hint: if g.is_some() {
                "Open in the Godot editor".into()
            } else {
                "Set the Godot executable in Settings".into()
            },
        });
    }
    v.push(LaunchOption {
        id: "terminal".into(),
        label: "Terminal".into(),
        available: true,
        icon: icon_of(terminal_icon_source().as_deref()),
        hint: "Open a terminal in this folder".into(),
    });
    v.push(LaunchOption {
        id: "explorer".into(),
        label: "Folder".into(),
        available: true,
        icon: icon_of(Some(&explorer_exe())),
        hint: "Show in File Explorer".into(),
    });
    v
}

/// Opens a terminal in `project` and runs Claude Code with `args`.
/// Arguments must be plain words (flags, session ids): they are added to a
/// PowerShell command line, so anything else is rejected.
pub fn start_claude(project: &Path, args: &[&str]) -> Result<(), String> {
    if !project.is_dir() {
        return Err(format!("Folder not found: {}", project.display()));
    }
    if args
        .iter()
        .any(|a| a.is_empty() || !a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return Err("Invalid argument for claude".into());
    }
    let exe = find_claude_cli().ok_or("Claude Code was not found on this PC.")?;
    // PowerShell call operator with a single-quoted path ('' escapes a quote).
    let mut cmd = format!("& '{}'", exe.to_string_lossy().replace('\'', "''"));
    for a in args {
        cmd.push(' ');
        cmd.push_str(a);
    }
    open_terminal(project, Some(&cmd))
}

/// Windows Terminal's launcher, if installed.
fn find_windows_terminal() -> Option<PathBuf> {
    env_path("LOCALAPPDATA")
        .map(|l| l.join("Microsoft").join("WindowsApps").join("wt.exe"))
        .filter(|p| p.exists())
}

/// Opens PowerShell in `project` (in Windows Terminal if available), optionally
/// running `command` in it. The window stays open afterwards.
fn open_terminal(project: &Path, command: Option<&str>) -> Result<(), String> {
    let mut ps_args: Vec<String> = vec!["-NoExit".into()];
    if let Some(c) = command {
        ps_args.push("-Command".into());
        ps_args.push(c.to_string());
    }
    // Windows Terminal treats ';' as a command separator, so only use it when
    // neither the folder nor the command contains one.
    let has_semicolon = project.to_string_lossy().contains(';')
        || command.map(|c| c.contains(';')).unwrap_or(false);
    if let (Some(wt), false) = (find_windows_terminal(), has_semicolon) {
        let mut c = Command::new(wt);
        c.arg("-d")
            .arg(project)
            .arg("powershell.exe")
            .args(&ps_args);
        return spawn(&mut c);
    }
    let mut c = Command::new("powershell.exe");
    c.args(&ps_args).current_dir(project);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        c.creation_flags(CREATE_NEW_CONSOLE);
    }
    spawn(&mut c)
}

fn spawn(cmd: &mut Command) -> Result<(), String> {
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start program: {e}"))
}

pub fn launch(project: &Path, tool: &str, s: &Settings) -> Result<(), String> {
    if !project.is_dir() {
        return Err(format!("Folder not found: {}", project.display()));
    }
    match tool {
        "vscode" => {
            let exe = find_vscode(s).ok_or("VS Code not found. Set its path in Settings.")?;
            spawn(Command::new(exe).arg(project).current_dir(project))
        }
        "intellij" => {
            let exe =
                find_intellij(s).ok_or("IntelliJ IDEA not found. Set its path in Settings.")?;
            spawn(Command::new(exe).arg(project).current_dir(project))
        }
        "unity" => {
            let exe = unity_exe(project, s)?;
            spawn(Command::new(exe).arg("-projectPath").arg(project))
        }
        "unreal" => {
            let up = find_file_with_ext(project, "uproject")
                .ok_or("No .uproject file in this folder")?;
            match find_unreal(s) {
                Some(exe) => spawn(Command::new(exe).arg(&up)),
                // No editor configured: open the .uproject with its Windows file
                // association (Epic's version selector picks the right engine).
                None => spawn(Command::new("explorer.exe").arg(&up)),
            }
        }
        "godot" => {
            let exe = find_godot(s).ok_or("Godot executable not set. Choose it in Settings.")?;
            spawn(Command::new(exe).arg("--editor").arg("--path").arg(project))
        }
        "terminal" => open_terminal(project, None),
        "claude" => start_claude(project, &[]),
        "claude-continue" => start_claude(project, &["--continue"]),
        "explorer" => spawn(Command::new("explorer.exe").arg(project)),
        other => Err(format!("Unknown tool: {other}")),
    }
}
