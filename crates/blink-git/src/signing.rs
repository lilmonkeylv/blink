use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, find_git};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningFormat {
    Ssh,
    Gpg,
}

impl SigningFormat {
    pub fn git_value(self) -> &'static str {
        match self {
            SigningFormat::Ssh => "ssh",
            SigningFormat::Gpg => "openpgp",
        }
    }

    pub fn from_git_value(value: &str) -> Option<Self> {
        match value {
            "ssh" => Some(SigningFormat::Ssh),
            "openpgp" => Some(SigningFormat::Gpg),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigScope {
    Global,
    Local,
}

impl ConfigScope {
    fn flag(self) -> &'static str {
        match self {
            ConfigScope::Global => "--global",
            ConfigScope::Local => "--local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningKey {
    pub format: SigningFormat,
    pub id: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningConfig {
    pub format: Option<SigningFormat>,
    pub key: Option<String>,
    pub scope: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshPublicKey {
    pub kind: String,
    pub blob: String,
    pub comment: Option<String>,
}

impl SshPublicKey {
    pub fn parse(text: &str) -> Option<Self> {
        let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
        let mut parts = line.splitn(3, char::is_whitespace);
        let kind = parts.next()?;
        let blob = parts.next()?;
        let known = kind.starts_with("ssh-") || kind.starts_with("ecdsa-sha2-") || kind.starts_with("sk-");
        let base64 =
            !blob.is_empty() && blob.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='));
        if !known || !base64 {
            return None;
        }
        let comment = parts.next().map(str::trim).filter(|c| !c.is_empty()).map(str::to_string);
        Some(Self { kind: kind.to_string(), blob: blob.to_string(), comment })
    }

    pub fn openssh(&self) -> String {
        format!("{} {}", self.kind, self.blob)
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn on_path(name: &str) -> Option<PathBuf> {
    let file = exe(name);
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
}

fn bundled_with_git(name: &str) -> Option<PathBuf> {
    let git = find_git()?;
    let file = exe(name);
    git.ancestors().skip(1).take(3).map(|root| root.join("usr").join("bin").join(&file)).find(|p| p.is_file())
}

fn windows_locations(name: &str) -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let file = exe(name);
    let mut out = Vec::new();
    if let Some(root) = std::env::var_os("SystemRoot") {
        out.push(PathBuf::from(root).join("System32").join("OpenSSH").join(&file));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(var) {
            out.push(PathBuf::from(root).join("GnuPG").join("bin").join(&file));
        }
    }
    out
}

pub fn find_ssh_keygen() -> Option<PathBuf> {
    on_path("ssh-keygen")
        .or_else(|| windows_locations("ssh-keygen").into_iter().find(|p| p.is_file()))
        .or_else(|| bundled_with_git("ssh-keygen"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgProgram {
    pub path: PathBuf,
    pub on_path: bool,
}

pub fn find_gpg(configured: Option<&str>) -> Option<GpgProgram> {
    if let Some(configured) = configured.map(str::trim).filter(|c| !c.is_empty()) {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(GpgProgram { path, on_path: false });
        }
        if let Some(path) = on_path(configured.trim_end_matches(".exe")) {
            return Some(GpgProgram { path, on_path: true });
        }
    }
    if let Some(path) = on_path("gpg") {
        return Some(GpgProgram { path, on_path: true });
    }
    windows_locations("gpg")
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| bundled_with_git("gpg"))
        .map(|path| GpgProgram { path, on_path: false })
}

fn tool(program: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run_tool(program: &Path, args: &[&str]) -> Result<String> {
    let name = program.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let output = tool(program, args).output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::Tool { tool: name, stderr })
}

pub fn list_ssh_keys(dir: &Path) -> Vec<SigningKey> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut keys: Vec<SigningKey> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "pub") && path.is_file())
        .filter_map(|path| {
            let key = SshPublicKey::parse(&std::fs::read_to_string(&path).ok()?)?;
            let file = path.file_name()?.to_string_lossy().into_owned();
            Some(SigningKey {
                format: SigningFormat::Ssh,
                id: path.to_string_lossy().into_owned(),
                label: key.comment.clone().unwrap_or_else(|| file.clone()),
                detail: format!("{file} - {}", key.kind),
            })
        })
        .collect();
    keys.sort_by(|a, b| a.detail.cmp(&b.detail));
    keys
}

pub fn ssh_key_in(dir: &Path, path: &str) -> Result<(PathBuf, SshPublicKey)> {
    let invalid = || Error::InvalidArgument(format!("{path:?} is not a public key in {}", dir.display()));
    let candidate = PathBuf::from(path);
    if candidate.extension().is_none_or(|ext| ext != "pub") {
        return Err(invalid());
    }
    let canonical = std::fs::canonicalize(&candidate).map_err(|_| invalid())?;
    let root = std::fs::canonicalize(dir).map_err(|_| invalid())?;
    if canonical.parent() != Some(root.as_path()) {
        return Err(invalid());
    }
    let key = SshPublicKey::parse(&std::fs::read_to_string(&canonical)?).ok_or_else(invalid)?;
    Ok((candidate, key))
}

pub fn validate_key_file_name(name: &str) -> Result<&str> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && !name.ends_with(".pub")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    if valid { Ok(name) } else { Err(Error::InvalidArgument(format!("{name:?} is not a usable key file name"))) }
}

pub fn generate_ssh_key(ssh_keygen: &Path, dir: &Path, name: &str, comment: &str) -> Result<PathBuf> {
    let name = validate_key_file_name(name)?;
    let private = dir.join(name);
    let public = dir.join(format!("{name}.pub"));
    if private.exists() || public.exists() {
        return Err(Error::InvalidArgument(format!("{} already exists", private.display())));
    }
    std::fs::create_dir_all(dir)?;
    let file = private.to_string_lossy().into_owned();
    let comment = comment.replace(['\r', '\n'], " ");
    run_tool(ssh_keygen, &["-q", "-t", "ed25519", "-N", "", "-C", comment.trim(), "-f", &file])?;
    if !public.is_file() {
        return Err(Error::Tool { tool: "ssh-keygen".into(), stderr: "no public key was written".into() });
    }
    Ok(public)
}

pub fn validate_gpg_key_id(id: &str) -> Result<&str> {
    if (8..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(id)
    } else {
        Err(Error::InvalidArgument(format!("{id:?} is not a GPG key id")))
    }
}

pub fn parse_gpg_secret_keys(colons: &str) -> Vec<SigningKey> {
    let mut keys: Vec<SigningKey> = Vec::new();
    let mut current: Option<SigningKey> = None;
    for line in colons.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        let field = |i: usize| fields.get(i).copied().unwrap_or_default();
        match field(0) {
            "sec" => {
                keys.extend(current.take());
                let unusable = matches!(field(1), "r" | "e" | "d" | "i");
                let can_sign = field(11).contains(['s', 'S']);
                if !unusable && can_sign && !field(4).is_empty() {
                    current = Some(SigningKey {
                        format: SigningFormat::Gpg,
                        id: field(4).to_string(),
                        label: String::new(),
                        detail: field(4).to_string(),
                    });
                }
            }
            "uid" => {
                if let Some(key) = current.as_mut().filter(|k| k.label.is_empty()) {
                    key.label = unescape_colons(field(9));
                }
            }
            _ => {}
        }
    }
    keys.extend(current);
    for key in &mut keys {
        if key.label.is_empty() {
            key.label = key.id.clone();
        }
    }
    keys
}

fn unescape_colons(text: &str) -> String {
    text.replace("\\x3a", ":")
}

pub fn list_gpg_keys(gpg: &Path) -> Result<Vec<SigningKey>> {
    let out = run_tool(gpg, &["--batch", "--list-secret-keys", "--keyid-format=long", "--with-colons"])?;
    Ok(parse_gpg_secret_keys(&out))
}

pub fn export_gpg_public_key(gpg: &Path, id: &str) -> Result<String> {
    let id = validate_gpg_key_id(id)?;
    let armored = run_tool(gpg, &["--batch", "--armor", "--export", id])?;
    if !armored.contains("BEGIN PGP PUBLIC KEY BLOCK") {
        return Err(Error::InvalidArgument(format!("GPG has no public key for {id}")));
    }
    Ok(armored)
}

impl GitCli {
    pub fn config_get_with_scope(&self, key: &str) -> Option<(String, String)> {
        let out = self.run(&["config", "--show-scope", "--get", key]).ok()?;
        let (scope, value) = out.trim_end_matches(['\r', '\n']).split_once('\t')?;
        Some((scope.to_string(), value.trim().to_string())).filter(|(_, v)| !v.is_empty())
    }

    pub fn config_set(&self, scope: ConfigScope, key: &str, value: &str) -> Result<()> {
        self.run(&["config", scope.flag(), key, value]).map(drop)
    }

    pub fn signing_config(&self) -> SigningConfig {
        let key = self.config_get_with_scope("user.signingkey");
        let format = match self.config_get("gpg.format") {
            Some(value) => SigningFormat::from_git_value(&value),
            None => key.as_ref().map(|_| SigningFormat::Gpg),
        };
        let enabled = self.config_get("commit.gpgsign").is_some_and(|v| v.eq_ignore_ascii_case("true"));
        let (scope, key) = key.map_or((None, None), |(scope, key)| (Some(scope), Some(key)));
        SigningConfig { format, key, scope, enabled }
    }

    pub fn configure_signing(
        &self,
        scope: ConfigScope,
        format: SigningFormat,
        key: &str,
        gpg_program: Option<&Path>,
    ) -> Result<()> {
        self.config_set(scope, "gpg.format", format.git_value())?;
        self.config_set(scope, "user.signingkey", key)?;
        if let Some(program) = gpg_program {
            self.config_set(scope, "gpg.program", &program.to_string_lossy())?;
        }
        self.config_set(scope, "commit.gpgsign", "true")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED25519: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBk0zU1Xv5xmOcBqNnkTAo5f1q2T8ZKcjzkQzRrH7Vbb me@example.com\n";

    #[test]
    fn parses_openssh_public_keys() {
        let key = SshPublicKey::parse(ED25519).unwrap();
        assert_eq!(key.kind, "ssh-ed25519");
        assert_eq!(key.comment.as_deref(), Some("me@example.com"));
        assert!(key.openssh().starts_with("ssh-ed25519 AAAAC3"));
        assert!(!key.openssh().contains("me@example.com"));
        assert!(SshPublicKey::parse("-----BEGIN OPENSSH PRIVATE KEY-----\nabc").is_none());
        assert!(SshPublicKey::parse("ssh-rsa not*base64").is_none());
        assert_eq!(SshPublicKey::parse("ecdsa-sha2-nistp256 AAAAE2VjZHNh").unwrap().comment, None);
    }

    #[test]
    fn lists_only_public_keys_in_the_ssh_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("id_ed25519.pub"), ED25519).unwrap();
        std::fs::write(dir.path().join("id_ed25519"), "-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();
        std::fs::write(dir.path().join("known_hosts.pub"), "github.com ssh-ed25519 AAAA\n").unwrap();
        let keys = list_ssh_keys(dir.path());
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].label, "me@example.com");
        assert_eq!(keys[0].detail, "id_ed25519.pub - ssh-ed25519");

        let (path, key) = ssh_key_in(dir.path(), &keys[0].id).unwrap();
        assert_eq!(path, PathBuf::from(&keys[0].id));
        assert_eq!(key.kind, "ssh-ed25519");
        let outside = tempfile::tempdir().unwrap();
        let stray = outside.path().join("other.pub");
        std::fs::write(&stray, ED25519).unwrap();
        assert!(ssh_key_in(dir.path(), &stray.to_string_lossy()).is_err());
        assert!(ssh_key_in(dir.path(), &dir.path().join("id_ed25519").to_string_lossy()).is_err());
        let sneaky = dir.path().join("..").join(outside.path().file_name().unwrap()).join("other.pub");
        assert!(ssh_key_in(dir.path(), &sneaky.to_string_lossy()).is_err());
    }

    #[test]
    fn parses_gpg_secret_keys() {
        let colons = "\
sec:u:255:22:3AA5C34371567BD2:1700000000:::u:::scESC:::+:::ed25519:::0:
fpr:::::::::0123456789ABCDEF01233AA5C34371567BD2:
uid:u::::1700000000::HASH::Mona Lisa <mona\\x3alisa@example.com>::::::::::0:
ssb:u:255:18:42B317FD4BA89E7A:1700000000::::::e:::+:::cv25519::
sec:r:255:22:1111111111111111:1600000000:::u:::scESC:::+:::ed25519:::0:
uid:r::::1600000000::HASH::Revoked <old@example.com>::::::::::0:
sec:u:255:22:2222222222222222:1600000000:::u:::eE:::+:::ed25519:::0:
sec:u:255:22:4444444444444444:1600000000:::u:::scSC:::+:::ed25519:::0:
";
        let keys = parse_gpg_secret_keys(colons);
        let ids: Vec<_> = keys.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(ids, ["3AA5C34371567BD2", "4444444444444444"]);
        assert_eq!(keys[0].label, "Mona Lisa <mona:lisa@example.com>");
        assert_eq!(keys[1].label, "4444444444444444");
    }

    #[test]
    fn validates_inputs() {
        assert!(validate_gpg_key_id("3AA5C34371567BD2").is_ok());
        assert!(validate_gpg_key_id("--export-secret-keys").is_err());
        assert!(validate_gpg_key_id("abc").is_err());
        assert!(validate_key_file_name("id_ed25519_signing").is_ok());
        for bad in ["", ".hidden", "-f", "a/b", "..\\x", "key.pub", "has space"] {
            assert!(validate_key_file_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn generates_without_overwriting() {
        let Some(keygen) = find_ssh_keygen() else { return };
        let dir = tempfile::tempdir().unwrap();
        let ssh = dir.path().join(".ssh");
        let public = generate_ssh_key(&keygen, &ssh, "id_ed25519_signing", "blink test").unwrap();
        let key = SshPublicKey::parse(&std::fs::read_to_string(&public).unwrap()).unwrap();
        assert_eq!((key.kind.as_str(), key.comment.as_deref()), ("ssh-ed25519", Some("blink test")));
        let private = std::fs::read(ssh.join("id_ed25519_signing")).unwrap();
        assert!(generate_ssh_key(&keygen, &ssh, "id_ed25519_signing", "again").is_err());
        assert_eq!(std::fs::read(ssh.join("id_ed25519_signing")).unwrap(), private);
        std::fs::remove_file(&public).unwrap();
        assert!(generate_ssh_key(&keygen, &ssh, "id_ed25519_signing", "again").is_err());
        assert_eq!(std::fs::read(ssh.join("id_ed25519_signing")).unwrap(), private);
    }

    #[test]
    fn writes_signing_config_to_the_chosen_scope() {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let global = dir.path().join("global.gitconfig");
        std::fs::write(&global, "").unwrap();
        let cli = GitCli::new(dir.path()).with_env("GIT_CONFIG_GLOBAL", global.to_string_lossy());
        cli.configure_signing(ConfigScope::Local, SigningFormat::Ssh, "/home/me/.ssh/id_ed25519.pub", None).unwrap();
        let config = cli.signing_config();
        assert_eq!(config.format, Some(SigningFormat::Ssh));
        assert_eq!(config.key.as_deref(), Some("/home/me/.ssh/id_ed25519.pub"));
        assert_eq!(config.scope.as_deref(), Some("local"));
        assert!(config.enabled);
        assert_eq!(std::fs::read_to_string(&global).unwrap(), "");

        cli.configure_signing(ConfigScope::Global, SigningFormat::Gpg, "3AA5C34371567BD2", Some(Path::new("gpg2")))
            .unwrap();
        let written = std::fs::read_to_string(&global).unwrap();
        assert!(written.contains("signingkey = 3AA5C34371567BD2"));
        assert!(written.contains("format = openpgp"));
        assert!(written.contains("program = gpg2"));
        assert_eq!(cli.signing_config().scope.as_deref(), Some("local"));
    }
}
