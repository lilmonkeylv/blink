use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    id: &'static str,
    name: &'static str,
}

struct Candidate {
    id: &'static str,
    name: &'static str,
    exe: &'static [&'static str],
}

const EDITORS: &[Candidate] = &[
    Candidate { id: "code", name: "Visual Studio Code", exe: &["code", "code.cmd"] },
    Candidate { id: "cursor", name: "Cursor", exe: &["cursor", "cursor.cmd"] },
    Candidate { id: "zed", name: "Zed", exe: &["zed", "zeditor"] },
    Candidate { id: "subl", name: "Sublime Text", exe: &["subl", "subl.exe"] },
    Candidate { id: "idea", name: "IntelliJ IDEA", exe: &["idea", "idea64.exe", "idea.cmd"] },
    Candidate { id: "webstorm", name: "WebStorm", exe: &["webstorm", "webstorm64.exe"] },
    Candidate { id: "rustrover", name: "RustRover", exe: &["rustrover", "rustrover64.exe"] },
    Candidate { id: "notepad++", name: "Notepad++", exe: &["notepad++.exe"] },
    Candidate { id: "nvim-qt", name: "Neovim Qt", exe: &["nvim-qt"] },
];

#[cfg(windows)]
const SHELLS: &[Candidate] = &[
    Candidate { id: "wt", name: "Windows Terminal", exe: &["wt.exe"] },
    Candidate { id: "pwsh", name: "PowerShell 7", exe: &["pwsh.exe"] },
    Candidate { id: "powershell", name: "Windows PowerShell", exe: &["powershell.exe"] },
    Candidate { id: "gitbash", name: "Git Bash", exe: &["git-bash.exe"] },
    Candidate { id: "cmd", name: "Command Prompt", exe: &["cmd.exe"] },
];

#[cfg(target_os = "macos")]
const SHELLS: &[Candidate] = &[
    Candidate { id: "terminal", name: "Terminal", exe: &[] },
    Candidate { id: "iterm", name: "iTerm2", exe: &[] },
    Candidate { id: "warp", name: "Warp", exe: &[] },
];

#[cfg(all(unix, not(target_os = "macos")))]
const SHELLS: &[Candidate] = &[
    Candidate { id: "gnome-terminal", name: "GNOME Terminal", exe: &["gnome-terminal"] },
    Candidate { id: "konsole", name: "Konsole", exe: &["konsole"] },
    Candidate { id: "kitty", name: "kitty", exe: &["kitty"] },
    Candidate { id: "alacritty", name: "Alacritty", exe: &["alacritty"] },
    Candidate { id: "x-terminal-emulator", name: "Default terminal", exe: &["x-terminal-emulator"] },
];

fn which(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe)).find(|p| p.is_file())
}

#[cfg(windows)]
fn git_bash() -> Option<PathBuf> {
    let git = blink_git::GitCli::new(".").run(&["--exec-path"]).ok()?;
    let root = Path::new(git.trim()).ancestors().find(|p| p.join("git-bash.exe").is_file())?.to_path_buf();
    Some(root.join("git-bash.exe"))
}

fn resolve(candidate: &Candidate) -> Option<PathBuf> {
    #[cfg(windows)]
    if candidate.id == "gitbash" {
        return git_bash();
    }
    #[cfg(target_os = "macos")]
    if candidate.exe.is_empty() {
        let app = match candidate.id {
            "terminal" => "/System/Applications/Utilities/Terminal.app",
            "iterm" => "/Applications/iTerm.app",
            _ => "/Applications/Warp.app",
        };
        return Path::new(app).exists().then(|| PathBuf::from(app));
    }
    candidate.exe.iter().find_map(|e| which(e))
}

fn available(list: &[Candidate]) -> Vec<Tool> {
    list.iter().filter(|c| resolve(c).is_some()).map(|c| Tool { id: c.id, name: c.name }).collect()
}

#[derive(Serialize)]
pub struct Integrations {
    editors: Vec<Tool>,
    shells: Vec<Tool>,
}

#[tauri::command]
pub async fn detect_integrations() -> Integrations {
    tauri::async_runtime::spawn_blocking(|| Integrations { editors: available(EDITORS), shells: available(SHELLS) })
        .await
        .unwrap_or(Integrations { editors: Vec::new(), shells: Vec::new() })
}

fn spawn(mut cmd: Command) -> CmdResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if cmd.get_program().to_string_lossy().ends_with(".cmd") {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
    }
    cmd.spawn().map(drop).map_err(|e| CommandError::new("launch", e.to_string()))
}

fn editor_command(state: &AppState) -> CmdResult<Command> {
    let settings = state.settings.read();
    if let Some(custom) = settings.integrations.custom_editor_path.as_deref().filter(|p| !p.is_empty()) {
        return Ok(Command::new(custom));
    }
    let chosen = settings.integrations.editor.clone();
    let candidate = EDITORS
        .iter()
        .find(|c| Some(c.id) == chosen.as_deref())
        .or_else(|| EDITORS.iter().find(|c| resolve(c).is_some()))
        .ok_or_else(|| {
            CommandError::new("noEditor", "No external editor found. Choose one in Settings → Integrations.")
        })?;
    let exe = resolve(candidate)
        .ok_or_else(|| CommandError::new("noEditor", format!("{} is not installed", candidate.name)))?;
    Ok(Command::new(exe))
}

#[tauri::command]
pub fn open_in_editor(state: State<'_, AppState>, id: String, path: Option<String>) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let root = handle.repo().workdir().to_path_buf();
    let mut cmd = editor_command(&state)?;
    cmd.arg(&root);
    if let Some(p) = path {
        cmd.arg(root.join(p));
    }
    spawn(cmd)
}

fn shell_command(state: &AppState, dir: &Path) -> CmdResult<Command> {
    let settings = state.settings.read();
    if let Some(custom) = settings.integrations.custom_shell_path.as_deref().filter(|p| !p.is_empty()) {
        let mut cmd = Command::new(custom);
        cmd.current_dir(dir);
        return Ok(cmd);
    }
    let chosen = settings.integrations.shell.clone();
    let candidate = SHELLS
        .iter()
        .find(|c| Some(c.id) == chosen.as_deref())
        .or_else(|| SHELLS.iter().find(|c| resolve(c).is_some()))
        .ok_or_else(|| CommandError::new("noShell", "No terminal found"))?;
    let exe = resolve(candidate)
        .ok_or_else(|| CommandError::new("noShell", format!("{} is not installed", candidate.name)))?;
    let mut cmd;
    #[cfg(target_os = "macos")]
    {
        cmd = Command::new("open");
        cmd.args(["-a", &exe.to_string_lossy(), &dir.to_string_lossy()]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        cmd = Command::new(&exe);
        match candidate.id {
            "wt" => {
                cmd.args(["-d", &dir.to_string_lossy()]);
            }
            "cmd" | "powershell" | "pwsh" => {
                cmd = Command::new("cmd.exe");
                cmd.args(["/c", "start", "", &exe.to_string_lossy()]);
            }
            "gitbash" => {
                cmd.arg(format!("--cd={}", dir.display()));
            }
            "gnome-terminal" => {
                cmd.arg(format!("--working-directory={}", dir.display()));
            }
            "kitty" => {
                cmd.arg("--directory").arg(dir);
            }
            "alacritty" => {
                cmd.arg("--working-directory").arg(dir);
            }
            "konsole" => {
                cmd.arg("--workdir").arg(dir);
            }
            _ => {}
        }
        cmd.current_dir(dir);
    }
    Ok(cmd)
}

#[tauri::command]
pub fn open_in_shell(state: State<'_, AppState>, id: String, path: Option<String>) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let root = handle.repo().workdir().to_path_buf();
    let at_file = state.settings.read().integrations.terminal_opens_at_file;
    let dir = match path.filter(|_| at_file) {
        Some(p) => root.join(p).parent().map(Path::to_path_buf).unwrap_or(root),
        None => root,
    };
    spawn(shell_command(&state, &dir)?)
}
