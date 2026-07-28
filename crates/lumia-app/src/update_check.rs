use anyhow::Context as _;
use gpui::{Context, Window};
use http_client::{AsyncBody, HttpClient};
use semver::Version;
use std::sync::Arc;

use crate::app::LumiaApp;

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/iFence/lumia/releases/latest";
const DEFAULT_BRANCH: &str = "master";
const RELEASES_PAGE_URL: &str = "https://github.com/iFence/lumia/releases/latest";

fn changelog_url() -> String {
    format!("https://raw.githubusercontent.com/iFence/lumia/{DEFAULT_BRANCH}/Changelog.md")
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateState {
    Idle,
    Checking,
    Available {
        latest_version: Version,
        release_notes: String,
        release_url: String,
    },
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
        matches!(self.state, UpdateState::Checking)
    }

    /// `true` when an update is available (used to show the "Open releases page" button).
    pub(crate) fn has_update(&self) -> bool {
        matches!(self.state, UpdateState::Available { .. })
    }

    pub(crate) fn release_url(&self) -> Option<&str> {
        match &self.state {
            UpdateState::Available { release_url, .. } => Some(release_url),
            _ => None,
        }
    }
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
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
    release_url: String,
}

/// Returns `Some(UpdateInfo)` when a newer release exists, `None` when up-to-date.
async fn check_update(
    client: &Arc<dyn HttpClient>,
    current: &Version,
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
    let notes = match fetch_text(client, &changelog_url()).await {
        Ok(content) => aggregate_release_notes(
            &parse_changelog(&content),
            current,
            &latest,
            release.body.as_deref(),
        ),
        Err(_) => release.body.clone().unwrap_or_default(),
    };
    let release_url = release
        .html_url
        .clone()
        .unwrap_or_else(|| RELEASES_PAGE_URL.to_string());
    Ok(Some(UpdateInfo {
        latest_version: latest,
        release_notes: notes,
        release_url,
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

        cx.spawn(async move |_this, cx| {
            let result = check_update(&client, &current).await;
            let _ = handle.update(cx, |this, cx| {
                match result {
                    Ok(Some(info)) => {
                        this.ui.update_check.state = UpdateState::Available {
                            latest_version: info.latest_version,
                            release_notes: info.release_notes,
                            release_url: info.release_url,
                        };
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

    /// Open the releases page in the system browser.
    pub(crate) fn open_releases_page(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = self.ui.update_check.release_url() {
            let _ = crate::shell::open_url_in_browser(url);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    #[test]
    fn parse_changelog_extracts_version_sections() {
        let content =
            "# Changelog\n\npreamble\n\n## v0.1.3\n\n- new feature\n\n## v0.1.2\n\n- initial\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, v(0, 1, 3));
        assert_eq!(entries[1].version, v(0, 1, 2));
        assert!(entries[0].body.contains("new feature"));
        assert!(entries[1].body.contains("initial"));
    }

    #[test]
    fn parse_changelog_strips_v_prefix_and_handles_no_prefix() {
        let content = "## v0.1.3\nbody1\n## 0.1.2\nbody2\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, v(0, 1, 3));
        assert_eq!(entries[1].version, v(0, 1, 2));
    }

    #[test]
    fn parse_changelog_ignores_trailing_date_in_header() {
        let content = "## v0.1.3 (2026-07-28)\nbody\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, v(0, 1, 3));
    }

    #[test]
    fn parse_changelog_handles_separator_between_sections() {
        let content = "## v0.1.3\nbody1\n---\n## v0.1.2\nbody2\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].body.contains("body1"));
        assert!(!entries[0].body.contains("body2"));
        assert!(entries[1].body.contains("body2"));
    }

    #[test]
    fn parse_changelog_handles_adjacent_headers() {
        let content = "## v0.1.3\n## v0.1.2\nbody2\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, v(0, 1, 3));
        assert_eq!(entries[0].body.trim(), "");
        assert_eq!(entries[1].version, v(0, 1, 2));
        assert!(entries[1].body.contains("body2"));
    }

    #[test]
    fn parse_changelog_drops_preamble_before_first_header() {
        let content = "# Changelog\n\n约定：\n- foo\n\n## v0.1.2\nbody\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, v(0, 1, 2));
    }

    #[test]
    fn parse_changelog_treats_non_version_header_as_body() {
        let content = "## v0.1.3\n## NotAVersion\nstill body\n";
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, v(0, 1, 3));
        assert!(entries[0].body.contains("NotAVersion"));
        assert!(entries[0].body.contains("still body"));
    }

    #[test]
    fn aggregate_release_notes_filters_and_sorts_descending() {
        let entries = vec![
            ChangelogEntry {
                version: v(0, 1, 2),
                body: "current".into(),
            },
            ChangelogEntry {
                version: v(0, 1, 3),
                body: "minor".into(),
            },
            ChangelogEntry {
                version: v(0, 1, 4),
                body: "latest".into(),
            },
        ];
        let notes = aggregate_release_notes(&entries, &v(0, 1, 2), &v(0, 1, 4), None);
        assert!(notes.contains("## v0.1.4"));
        assert!(notes.contains("## v0.1.3"));
        assert!(!notes.contains("## v0.1.2"));
        let pos4 = notes.find("v0.1.4").unwrap();
        let pos3 = notes.find("v0.1.3").unwrap();
        assert!(pos4 < pos3);
    }

    #[test]
    fn aggregate_release_notes_falls_back_when_no_match() {
        let entries = vec![ChangelogEntry {
            version: v(0, 1, 2),
            body: "current".into(),
        }];
        let notes =
            aggregate_release_notes(&entries, &v(0, 1, 2), &v(0, 1, 4), Some("fallback notes"));
        assert_eq!(notes, "fallback notes");
    }

    #[test]
    fn aggregate_release_notes_empty_when_no_fallback() {
        let entries: Vec<ChangelogEntry> = vec![];
        let notes = aggregate_release_notes(&entries, &v(0, 1, 2), &v(0, 1, 4), None);
        assert_eq!(notes, "");
    }

    #[test]
    fn aggregate_release_notes_respects_latest_upper_bound() {
        let entries = vec![
            ChangelogEntry {
                version: v(0, 1, 3),
                body: "thirteen".into(),
            },
            ChangelogEntry {
                version: v(0, 1, 4),
                body: "fourteen".into(),
            },
            ChangelogEntry {
                version: v(0, 1, 5),
                body: "fifteen".into(),
            },
        ];
        let notes = aggregate_release_notes(&entries, &v(0, 1, 2), &v(0, 1, 4), None);
        assert!(notes.contains("v0.1.4"));
        assert!(notes.contains("v0.1.3"));
        assert!(!notes.contains("v0.1.5"));
    }

    #[test]
    fn update_check_ui_state_helpers() {
        let mut state = UpdateCheckUiState::default();
        assert!(!state.is_busy());
        assert!(!state.has_update());
        assert_eq!(state.release_url(), None);

        state.state = UpdateState::Checking;
        assert!(state.is_busy());
        assert!(!state.has_update());

        state.state = UpdateState::Available {
            latest_version: v(0, 2, 0),
            release_notes: "notes".into(),
            release_url: "https://example.com".into(),
        };
        assert!(!state.is_busy());
        assert!(state.has_update());
        assert_eq!(state.release_url(), Some("https://example.com"));

        state.state = UpdateState::UpToDate;
        assert!(!state.has_update());

        state.state = UpdateState::Error("err".into());
        assert!(!state.has_update());
    }
}
