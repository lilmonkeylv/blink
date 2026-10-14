use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const EXTENSIONS: &[(&str, &str)] =
    &[("image/png", "png"), ("image/jpeg", "jpg"), ("image/gif", "gif"), ("image/webp", "webp")];

fn allowed(url: &url::Url, enterprise_hosts: &[String]) -> bool {
    let Some(host) = url.host_str() else { return false };
    url.scheme() == "https"
        && (host == "avatars.githubusercontent.com"
            || enterprise_hosts.iter().any(|h| host == h || host.ends_with(&format!(".{h}"))))
}

pub fn avatar_dir(paths: &blink_store::AppPaths) -> PathBuf {
    paths.data_dir.join("avatars")
}

fn cache_path(dir: &std::path::Path, url: &str) -> PathBuf {
    let digest: String = Sha256::digest(url.as_bytes()).iter().take(16).map(|b| format!("{b:02x}")).collect();
    dir.join(digest)
}

fn data_uri(kind: &str, bytes: &[u8]) -> String {
    format!("data:{kind};base64,{}", STANDARD.encode(bytes))
}

fn read_cached(base: &std::path::Path) -> Option<String> {
    EXTENSIONS.iter().find_map(|(kind, ext)| {
        let path = base.with_extension(ext);
        let fresh = std::fs::metadata(&path).ok()?.modified().ok()?.elapsed().ok()? < MAX_AGE;
        fresh.then(|| std::fs::read(&path).ok()).flatten().map(|bytes| data_uri(kind, &bytes))
    })
}

#[tauri::command]
pub async fn avatar(state: State<'_, AppState>, url: String) -> CmdResult<Option<String>> {
    let parsed = url::Url::parse(&url).map_err(|e| CommandError::new("avatar", e.to_string()))?;
    let hosts: Vec<String> = state.enterprise_accounts().into_iter().map(|a| a.host).collect();
    if !allowed(&parsed, &hosts) {
        return Ok(None);
    }
    let dir = avatar_dir(&state.paths);
    let base = cache_path(&dir, &url);
    if let Some(hit) = read_cached(&base) {
        return Ok(Some(hit));
    }
    let (kind, bytes) = match blink_github::fetch_image(&url).await {
        Ok(image) => image,
        Err(e) => {
            tracing::debug!(error = %e, "avatar fetch failed");
            return Ok(None);
        }
    };
    let ext = EXTENSIONS.iter().find(|(k, _)| *k == kind).map(|(_, e)| *e).unwrap_or("png");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(base.with_extension(ext), &bytes);
    }
    Ok(Some(data_uri(kind, &bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_avatar_hosts_over_https() {
        let ok = |u: &str, hosts: &[&str]| {
            let hosts: Vec<String> = hosts.iter().map(|h| h.to_string()).collect();
            allowed(&url::Url::parse(u).unwrap(), &hosts)
        };
        assert!(ok("https://avatars.githubusercontent.com/u/1?v=4", &[]));
        assert!(!ok("http://avatars.githubusercontent.com/u/1", &[]));
        assert!(!ok("https://evil.example.com/a.png", &[]));
        assert!(ok("https://ghe.corp.com/avatars/u/3", &["ghe.corp.com"]));
        assert!(ok("https://avatars.acme.ghe.com/u/3", &["acme.ghe.com"]));
        assert!(!ok("https://notghe.corp.com.evil.io/a", &["ghe.corp.com"]));
    }

    #[test]
    fn cached_files_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let base = cache_path(dir.path(), "https://avatars.githubusercontent.com/u/1");
        assert!(read_cached(&base).is_none());
        std::fs::write(base.with_extension("png"), [1, 2, 3]).unwrap();
        assert_eq!(read_cached(&base).unwrap(), "data:image/png;base64,AQID");
    }
}
