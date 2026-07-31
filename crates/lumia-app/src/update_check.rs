use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use gpui::{Context, Window};
use http_client::{AsyncBody, HttpClient};
use semver::Version;

use crate::app::LumiaApp;
use crate::persistence::save_settings;
use lumia_core::Language;

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/iFence/lumia/releases/latest";
const DEFAULT_BRANCH: &str = "master";

fn changelog_url(language: Language) -> String {
    let filename = match language {
        Language::English => "Changelog.md",
        Language::Chinese => "Changelog-zh-CN.md",
    };
    format!("https://raw.githubusercontent.com/iFence/lumia/{DEFAULT_BRANCH}/{filename}")
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateState {
    Idle,
    Checking,
    Available {
        latest_version: Version,
        release_notes: String,
        asset: UpdateAsset,
    },
    Downloading {
        latest_version: Version,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Installing,
    UpToDate,
    Error(String),
}

impl Default for UpdateState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UpdateCheckUiState {
    pub(crate) state: UpdateState,
}

impl UpdateCheckUiState {
    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing
        )
    }

    /// `true` when an update is available and ready to download.
    pub(crate) fn has_update(&self) -> bool {
        matches!(self.state, UpdateState::Available { .. })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateAsset {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) size: u64,
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Select the platform-appropriate installer asset from a GitHub release.
fn select_asset(assets: &[GithubAsset]) -> Option<UpdateAsset> {
    let matches_platform = |name: &str| {
        #[cfg(target_os = "windows")]
        {
            name.starts_with("Lumia-Setup-") && name.ends_with("-x64.exe")
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            name == "Lumia-macos-arm64.dmg"
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            name == "Lumia-macos-x64.dmg"
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            name == "lumia-linux-x64.tar.gz"
        }
    };
    assets
        .iter()
        .find(|asset| matches_platform(&asset.name))
        .map(|asset| UpdateAsset {
            name: asset.name.clone(),
            url: asset.browser_download_url.clone(),
            size: asset.size,
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangelogEntry {
    pub(crate) version: Version,
    pub(crate) body: String,
}

/// Parse `## vX.Y.Z` sections out of `Changelog.md`.
///
/// - Headers are `## ` followed by `vX.Y.Z` (leading `v`/`V` optional).
/// - A trailing date/token on the header line (e.g. `## v0.1.3 (2026-07-28)`)
///   is ignored: only the first whitespace-delimited token after `## ` is parsed.
/// - `---` separators and adjacent headers both cleanly terminate the previous entry.
/// - Lines before the first recognized header are dropped.
/// - A `## ` line whose token is not a version is treated as body content of the
///   current section (so sub-headers inside a version block are preserved).
pub(crate) fn parse_changelog(content: &str) -> Vec<ChangelogEntry> {
    let mut entries = Vec::new();
    let mut current: Option<(Version, String)> = None;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            let token = rest.split_whitespace().next().unwrap_or("");
            let ver_str = token
                .strip_prefix('v')
                .or_else(|| token.strip_prefix('V'))
                .unwrap_or(token);
            if let Ok(version) = Version::parse(ver_str) {
                if let Some((v, body)) = current.take() {
                    entries.push(ChangelogEntry {
                        version: v,
                        body: body.trim_end().to_string(),
                    });
                }
                current = Some((version, String::new()));
            } else if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        } else if trimmed == "---" {
            if let Some((v, body)) = current.take() {
                entries.push(ChangelogEntry {
                    version: v,
                    body: body.trim_end().to_string(),
                });
            }
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some((v, body)) = current.take() {
        entries.push(ChangelogEntry {
            version: v,
            body: body.trim_end().to_string(),
        });
    }
    entries
}

/// Return release notes for versions `current < v <= latest`, sorted descending,
/// joined with a `---` separator. Falls back to `fallback_body` (the GitHub
/// release body) when no matching entries are found.
pub(crate) fn aggregate_release_notes(
    entries: &[ChangelogEntry],
    current: &Version,
    latest: &Version,
    fallback_body: Option<&str>,
) -> String {
    let mut matched: Vec<&ChangelogEntry> = entries
        .iter()
        .filter(|entry| entry.version > *current && entry.version <= *latest)
        .collect();
    matched.sort_by(|a, b| b.version.cmp(&a.version));

    if matched.is_empty() {
        return fallback_body
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_default()
            .to_string();
    }

    matched
        .into_iter()
        .map(|entry| format!("## v{}\n\n{}", entry.version, entry.body.trim()))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

async fn fetch_text(client: &Arc<dyn HttpClient>, url: &str) -> anyhow::Result<String> {
    let response = client.get(url, AsyncBody::from(()), true).await?;
    if !response.status().is_success() {
        anyhow::bail!("http status {}", response.status());
    }
    let (_parts, mut body) = response.into_parts();
    let mut bytes = Vec::new();
    futures::io::AsyncReadExt::read_to_end(&mut body, &mut bytes).await?;
    Ok(String::from_utf8(bytes)?)
}

struct UpdateInfo {
    latest_version: Version,
    release_notes: String,
    asset: UpdateAsset,
}

/// Returns `Some(UpdateInfo)` when a newer release exists, `None` when up-to-date.
async fn check_update(
    client: &Arc<dyn HttpClient>,
    current: &Version,
    language: Language,
) -> anyhow::Result<Option<UpdateInfo>> {
    let json = fetch_text(client, RELEASES_LATEST_URL).await?;
    let release: GithubRelease =
        serde_json::from_str(&json).context("parse GitHub releases/latest response")?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let tag = release.tag_name.trim();
    let ver_str = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    let latest = Version::parse(ver_str).with_context(|| format!("parse release tag {tag:?}"))?;
    if latest <= *current {
        return Ok(None);
    }
    let asset =
        select_asset(&release.assets).context("no matching installer asset for platform")?;
    let notes = match fetch_text(client, &changelog_url(language)).await {
        Ok(content) => aggregate_release_notes(
            &parse_changelog(&content),
            current,
            &latest,
            release.body.as_deref(),
        ),
        Err(_) => release.body.clone().unwrap_or_default(),
    };
    Ok(Some(UpdateInfo {
        latest_version: latest,
        release_notes: notes,
        asset,
    }))
}

impl LumiaApp {
    /// Manual check (surfaces errors) or startup check (silent on failure when `manual=false`).
    pub(crate) fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.ui.update_check.is_busy() {
            return;
        }
        self.ui.update_check.state = UpdateState::Checking;
        cx.notify();

        let client = cx.http_client();
        let handle = self.self_handle.clone();
        let current =
            Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is valid semver");
        let language = self.settings.language;

        cx.spawn(async move |_this, cx| {
            let result = check_update(&client, &current, language).await;
            let _ = handle.update(cx, |this, cx| {
                match result {
                    Ok(Some(info)) => {
                        let skipped = this
                            .settings
                            .skipped_update_version
                            .as_deref()
                            .map(|v| v.trim_start_matches('v'))
                            .and_then(|v| Version::parse(v).ok());
                        if skipped.as_ref() == Some(&info.latest_version) {
                            this.ui.update_check.state = if manual {
                                UpdateState::UpToDate
                            } else {
                                UpdateState::Idle
                            };
                        } else {
                            this.ui.update_check.state = UpdateState::Available {
                                latest_version: info.latest_version,
                                release_notes: info.release_notes,
                                asset: info.asset,
                            };
                        }
                    }
                    Ok(None) => {
                        this.ui.update_check.state = UpdateState::UpToDate;
                    }
                    Err(err) => {
                        if manual {
                            this.ui.update_check.state = UpdateState::Error(format!("{err:#}"));
                        } else {
                            this.ui.update_check.state = UpdateState::Idle;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Called once from `bootstrap.rs` after the window/view is created.
    pub(crate) fn maybe_check_for_updates_on_startup(&mut self, cx: &mut Context<Self>) {
        if self.settings.check_updates_on_startup {
            self.check_for_updates(false, cx);
        }
    }

    /// Action handler for `CheckForUpdates` (manual).
    pub(crate) fn handle_check_for_updates(
        &mut self,
        _: &crate::CheckForUpdates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.check_for_updates(true, cx);
    }

    /// Download the installer for the currently-available update, launch it, and quit.
    pub(crate) fn download_and_install(&mut self, cx: &mut Context<Self>) {
        let (latest_version, asset) = match &self.ui.update_check.state {
            UpdateState::Available {
                latest_version,
                asset,
                ..
            } => (latest_version.clone(), asset.clone()),
            _ => return,
        };

        let total_bytes = (asset.size > 0).then_some(asset.size);
        self.ui.update_check.state = UpdateState::Downloading {
            latest_version: latest_version.clone(),
            downloaded_bytes: 0,
            total_bytes,
        };
        cx.notify();

        let client = cx.http_client();
        let handle = self.self_handle.clone();
        let dest = std::env::temp_dir().join(&asset.name);

        cx.spawn(async move |_this, cx| {
            let result: anyhow::Result<()> = async {
                let response = client.get(&asset.url, AsyncBody::from(()), true).await?;
                if !response.status().is_success() {
                    anyhow::bail!("http status {}", response.status());
                }
                let (_parts, mut body) = response.into_parts();
                let mut file = std::fs::File::create(&dest)?;
                let mut downloaded: u64 = 0;
                let mut buf = [0u8; 8192];
                let mut last_notify = Instant::now();
                loop {
                    let n = futures::io::AsyncReadExt::read(&mut body, &mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    std::io::Write::write_all(&mut file, &buf[..n])?;
                    downloaded += n as u64;
                    if last_notify.elapsed() >= Duration::from_millis(100) {
                        last_notify = Instant::now();
                        let _ = handle.update(cx, |this, cx| {
                            if let UpdateState::Downloading {
                                downloaded_bytes, ..
                            } = &mut this.ui.update_check.state
                            {
                                *downloaded_bytes = downloaded;
                            }
                            cx.notify();
                        });
                    }
                }
                file.sync_all()?;
                if let Some(expected) = total_bytes {
                    if downloaded != expected {
                        anyhow::bail!("download incomplete: {downloaded}/{expected} bytes");
                    }
                }
                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = handle.update(cx, |this, cx| {
                        this.ui.update_check.state = UpdateState::Installing;
                        cx.notify();
                    });
                    let _ = crate::shell::open_url_in_browser(&dest.to_string_lossy());
                    let _ = handle.update(cx, |_, cx| {
                        cx.quit();
                    });
                }
                Err(err) => {
                    let _ = handle.update(cx, |this, cx| {
                        this.ui.update_check.state = UpdateState::Error(format!("{err:#}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// Persist the currently-available version as skipped and dismiss the prompt.
    pub(crate) fn skip_update(&mut self, cx: &mut Context<Self>) {
        if let UpdateState::Available { latest_version, .. } = &self.ui.update_check.state {
            self.settings.skipped_update_version = Some(latest_version.to_string());
            let _ = save_settings(&self.settings);
        }
        self.ui.update_check.state = UpdateState::Idle;
        cx.notify();
    }
}

#[cfg(test)]
#[path = "update_check_tests.rs"]
mod update_check_tests;
