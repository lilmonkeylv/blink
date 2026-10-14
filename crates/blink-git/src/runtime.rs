use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::RwLock;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Runtime {
    pub executable: Option<PathBuf>,
    pub ssh: Option<PathBuf>,
    pub signing_key: Option<String>,
}

static RUNTIME: RwLock<Runtime> = RwLock::new(Runtime { executable: None, ssh: None, signing_key: None });

pub fn configure(runtime: Runtime) {
    *RUNTIME.write().unwrap_or_else(|e| e.into_inner()) = runtime;
}

pub fn runtime() -> Runtime {
    RUNTIME.read().unwrap_or_else(|e| e.into_inner()).clone()
}

impl Runtime {
    pub(crate) fn config_args(&self) -> Vec<String> {
        match self.signing_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
            Some(key) => vec!["-c".into(), format!("user.signingkey={key}")],
            None => Vec::new(),
        }
    }

    pub(crate) fn env(&self) -> Vec<(&'static str, PathBuf)> {
        self.ssh.iter().map(|ssh| ("GIT_SSH", ssh.clone())).collect()
    }
}

pub fn system_openssh(system_root: Option<&Path>) -> Option<PathBuf> {
    let ssh = system_root?.join("System32").join("OpenSSH").join("ssh.exe");
    ssh.is_file().then_some(ssh)
}

pub fn git_version(git: &Path) -> Result<String> {
    let mut cmd = Command::new(git);
    cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::GitNotFound,
        _ => e.into(),
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    match stdout.trim().strip_prefix("git version ") {
        Some(version) if output.status.success() => Ok(version.to_string()),
        _ => Err(Error::Git(format!("{} is not a Git executable", git.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_key_becomes_config_override() {
        let rt = Runtime { signing_key: Some(" ABC123 ".into()), ..Default::default() };
        assert_eq!(rt.config_args(), ["-c", "user.signingkey=ABC123"]);
        assert!(Runtime { signing_key: Some("  ".into()), ..Default::default() }.config_args().is_empty());
        assert!(Runtime::default().config_args().is_empty());
    }

    #[test]
    fn ssh_override_sets_git_ssh() {
        let rt = Runtime { ssh: Some(PathBuf::from("C:/Windows/System32/OpenSSH/ssh.exe")), ..Default::default() };
        assert_eq!(rt.env(), [("GIT_SSH", PathBuf::from("C:/Windows/System32/OpenSSH/ssh.exe"))]);
        assert!(Runtime::default().env().is_empty());
    }

    #[test]
    fn finds_system_openssh_only_when_present() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(system_openssh(Some(root.path())), None);
        assert_eq!(system_openssh(None), None);
        let dir = root.path().join("System32").join("OpenSSH");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ssh.exe"), b"").unwrap();
        assert_eq!(system_openssh(Some(root.path())), Some(dir.join("ssh.exe")));
    }

    #[test]
    fn validates_git_executables() {
        let git = crate::cli::find_git().expect("git on PATH");
        assert!(git_version(&git).unwrap().chars().next().unwrap().is_ascii_digit());
        assert!(matches!(git_version(Path::new("definitely-not-git-here")), Err(Error::GitNotFound)));
        let dir = tempfile::tempdir().unwrap();
        assert!(git_version(dir.path()).is_err());
    }
}
