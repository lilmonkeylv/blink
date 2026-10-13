use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub integrations: Integrations,
    pub git: GitSettings,
    pub appearance: Appearance,
    pub notifications: Notifications,
    pub prompts: Prompts,
    pub advanced: Advanced,
    pub accessibility: Accessibility,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            integrations: Integrations::default(),
            git: GitSettings::default(),
            appearance: Appearance::default(),
            notifications: Notifications::default(),
            prompts: Prompts::default(),
            advanced: Advanced::default(),
            accessibility: Accessibility::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiffTool {
    BuiltIn,
    VsCode,
    BeyondCompare,
    Meld,
    KDiff3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Integrations {
    pub editor: Option<String>,
    pub custom_editor_path: Option<String>,
    pub shell: Option<String>,
    pub custom_shell_path: Option<String>,
    pub diff_tool: DiffTool,
    pub terminal_opens_at_file: bool,
}

impl Default for Integrations {
    fn default() -> Self {
        Self {
            editor: None,
            custom_editor_path: None,
            shell: None,
            custom_shell_path: None,
            diff_tool: DiffTool::BuiltIn,
            terminal_opens_at_file: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PullStrategy {
    Merge,
    Rebase,
    FfOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitExecutable {
    System,
    Bundled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GitSettings {
    pub default_branch: String,
    pub sign_commits: bool,
    pub signing_key: Option<String>,
    pub pull_strategy: PullStrategy,
    pub prune_on_fetch: bool,
    pub executable: GitExecutable,
    pub executable_path: Option<String>,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            default_branch: "main".into(),
            sign_commits: false,
            signing_key: None,
            pull_strategy: PullStrategy::Merge,
            prune_on_fetch: false,
            executable: GitExecutable::System,
            executable_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateFormat {
    Relative,
    Absolute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: Theme,
    pub tab_size: u8,
    pub accent_color: Option<String>,
    pub density: Density,
    pub diff_font: Option<String>,
    pub diff_font_size: u8,
    pub date_format: DateFormat,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            tab_size: 8,
            accent_color: None,
            density: Density::Comfortable,
            diff_font: None,
            diff_font_size: 12,
            date_format: DateFormat::Relative,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Notifications {
    pub enabled: bool,
    pub checks_failed: bool,
    pub review_requested: bool,
    pub pr_merged: bool,
    pub push_rejected: bool,
    pub quiet_hours: Option<(u8, u8)>,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            checks_failed: true,
            review_requested: true,
            pr_merged: false,
            push_rejected: true,
            quiet_hours: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prompts {
    pub remove_repository: bool,
    pub discard_changes: bool,
    pub discard_changes_permanently: bool,
    pub force_push: bool,
    pub undo_commit: bool,
    pub hard_reset: bool,
    pub delete_branch_with_unpushed: bool,
}

impl Default for Prompts {
    fn default() -> Self {
        Self {
            remove_repository: true,
            discard_changes: true,
            discard_changes_permanently: true,
            force_push: true,
            undo_commit: true,
            hard_reset: true,
            delete_branch_with_unpushed: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchSwitchBehavior {
    Ask,
    BringChanges,
    StashOnCurrent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatcherMode {
    Fsmonitor,
    Native,
    Polling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Advanced {
    pub branch_switch: BranchSwitchBehavior,
    pub background_fetch: bool,
    pub background_fetch_minutes: u32,
    pub use_system_openssh: bool,
    pub watcher: WatcherMode,
    pub cache_limit_mb: u32,
    pub log_level: String,
    pub foreground_threads: Option<usize>,
    pub background_threads: Option<usize>,
    pub update_channel: UpdateChannel,
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            branch_switch: BranchSwitchBehavior::Ask,
            background_fetch: true,
            background_fetch_minutes: 60,
            use_system_openssh: false,
            watcher: WatcherMode::Native,
            cache_limit_mb: 512,
            log_level: "info".into(),
            foreground_threads: None,
            background_threads: None,
            update_channel: UpdateChannel::Stable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Accessibility {
    pub underline_links: bool,
    pub diff_check_marks: bool,
    pub reduced_motion: bool,
    pub high_contrast: bool,
    pub large_hit_targets: bool,
    pub keyboard_navigation: bool,
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());

        let mut s = Settings::default();
        s.appearance.theme = Theme::Dark;
        s.git.pull_strategy = PullStrategy::Rebase;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let s: Settings = toml::from_str("[appearance]\ntheme = \"light\"\n").unwrap();
        assert_eq!(s.appearance.theme, Theme::Light);
        assert_eq!(s.git.default_branch, "main");
    }

    #[test]
    fn removed_keys_are_ignored() {
        let s: Settings = toml::from_str("[advanced]\nusageStats = true\nbackgroundFetch = false\n").unwrap();
        assert!(!s.advanced.background_fetch);
    }

    #[test]
    fn update_channel_defaults_to_stable() {
        assert_eq!(Settings::default().advanced.update_channel, UpdateChannel::Stable);
        let s: Settings = toml::from_str("[advanced]\nupdateChannel = \"beta\"\n").unwrap();
        assert_eq!(s.advanced.update_channel, UpdateChannel::Beta);
    }
}
