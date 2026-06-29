//! Best-effort "new version available" check against the project website.
//!
//! Runs once per process on a detached background thread — never on the audio
//! thread or the egui/UI thread. It fetches a tiny JSON manifest from the site,
//! compares `latest` to the compiled version, and (only if newer) publishes the
//! result into [`UiShared`] for the editor to surface as a non-modal nudge.
//!
//! Throttled to roughly once a day via a small cache file beside the presets,
//! and silent on any failure (offline, sandboxed config dir, malformed JSON) —
//! matching the preset I/O house style. There is deliberately no opt-out.

use crate::UiShared;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// The manifest, served from the same host as the download links.
const MANIFEST_URL: &str = "https://naturarum.github.io/re-deemer/version.json";
/// Fallback opened on click if the manifest omits a URL.
const SITE_URL: &str = "https://naturarum.github.io/re-deemer/";
/// Re-hit the network at most this often; otherwise trust the cache.
const THROTTLE_SECS: u64 = 24 * 60 * 60;

/// The website manifest (`docs/version.json`).
#[derive(Deserialize)]
struct Manifest {
    latest: String,
    #[serde(default)]
    url: String,
}

/// On-disk throttle cache so we don't hit the network every launch.
#[derive(Serialize, Deserialize, Default)]
struct Cache {
    last_check_unix: u64,
    latest: String,
    url: String,
}

/// Kick off the check once per process. A cheap no-op on later calls (the
/// editor being reopened, or several plugin instances loading).
pub fn spawn_once(shared: Arc<UiShared>) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("re-deemer-update".into())
        .spawn(move || run(&shared));
}

fn run(shared: &UiShared) {
    let current = env!("CARGO_PKG_VERSION");

    // Inside the throttle window: trust the cache, skip the network entirely.
    if let Some(cache) = read_cache() {
        if now_secs().saturating_sub(cache.last_check_unix) < THROTTLE_SECS {
            publish(shared, &cache.latest, &cache.url, current);
            return;
        }
    }

    let Some(manifest) = fetch() else { return };
    let url = if manifest.url.is_empty() {
        SITE_URL.to_string()
    } else {
        manifest.url
    };
    write_cache(&Cache {
        last_check_unix: now_secs(),
        latest: manifest.latest.clone(),
        url: url.clone(),
    });
    publish(shared, &manifest.latest, &url, current);
}

/// Blocking HTTPS GET of the manifest; `None` on any error (silent / offline).
fn fetch() -> Option<Manifest> {
    let resp = minreq::get(MANIFEST_URL)
        .with_header("User-Agent", concat!("RE-DEEMER/", env!("CARGO_PKG_VERSION")))
        .with_timeout(5)
        .send()
        .ok()?;
    if resp.status_code != 200 {
        return None;
    }
    serde_json::from_str::<Manifest>(resp.as_str().ok()?).ok()
}

/// Publish into the shared state iff `latest` is strictly newer than `current`.
fn publish(shared: &UiShared, latest: &str, url: &str, current: &str) {
    if latest.is_empty() || !is_newer(latest, current) {
        return;
    }
    if let Ok(mut slot) = shared.update_info.lock() {
        *slot = Some((latest.to_string(), url.to_string()));
    }
    shared.update_available.store(true, Ordering::Relaxed);
}

/// `MAJOR.MINOR.PATCH` compare. Tolerates a leading `v` and pre-release
/// suffixes (e.g. "1.2.0-beta" parses as (1,2,0)).
fn is_newer(latest: &str, current: &str) -> bool {
    parse(latest) > parse(current)
}

fn parse(s: &str) -> (u32, u32, u32) {
    let mut parts = s.trim().trim_start_matches('v').split('.').map(|p| {
        p.bytes()
            .take_while(u8::is_ascii_digit)
            .fold(0u32, |acc, b| acc.saturating_mul(10).saturating_add((b - b'0') as u32))
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `.../RE-DEEMER/update-check.json` — a sibling of the `presets/` dir.
fn cache_path() -> Option<PathBuf> {
    let presets = crate::presets::preset_dir()?;
    Some(presets.parent()?.join("update-check.json"))
}

fn read_cache() -> Option<Cache> {
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(cache: &Cache) {
    let Some(path) = cache_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::{is_newer, parse};

    #[test]
    fn version_compare() {
        assert!(is_newer("1.1.2", "1.1.1"));
        assert!(is_newer("1.2.0", "1.1.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("v1.1.2", "1.1.1")); // tolerate a leading v
        assert!(!is_newer("1.1.1", "1.1.1"));
        assert!(!is_newer("1.1.0", "1.1.1"));
        assert!(!is_newer("1.1.1-beta", "1.1.1")); // suffix ignored -> equal
        assert_eq!(parse("1.2.3"), (1, 2, 3));
        assert_eq!(parse("bogus"), (0, 0, 0));
    }
}
