//! Community plugin browser: state, index fetching/caching, search input, and
//! download-then-install. The index data model and pure functions live in
//! [`crate::community_index`]; this module holds the gpui-owned state and the
//! I/O paths that need the app context.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use semver::Version;
use sha2::{Digest, Sha256};

use crate::app::LumiaApp;
use crate::community_index::{
    best_artifact_for_host, best_compatible_artifact_for_host, matches_query, parse_index,
    CommunityIndex, CommunityPlugin,
};
use crate::community_text::{tr_community, CommunityTextKey};
use lumia_core::SettingsGroup;

const COMMUNITY_INDEX_URL: &str =
    "https://raw.githubusercontent.com/iFence/awesome-lumia-plugin/main/plugins.json";
/// Cached copy of the index, stored under the per-user data directory so the
/// community browser renders instantly while offline.
const COMMUNITY_INDEX_FILE: &str = "community-plugins.json";
const MAX_COMMUNITY_INDEX_BYTES: usize = 4 * 1024 * 1024;
const MAX_PACKAGE_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
const DOWNLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CommunityTab {
    #[default]
    Community,
    Installed,
}

#[derive(Debug, Clone)]
pub(crate) enum CommunityStatus {
    Idle,
    Loading,
    Loaded,
    Downloading {
        plugin_id: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Error(String),
}

impl Default for CommunityStatus {
    fn default() -> Self {
        Self::Idle
    }
}

impl CommunityStatus {
    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self,
            CommunityStatus::Loading | CommunityStatus::Downloading { .. }
        )
    }
}

#[derive(Default)]
pub(crate) struct CommunityPluginsState {
    /// Parsed index, shared so the background fetch can hand it off cheaply.
    pub(crate) index: Option<Arc<CommunityIndex>>,
    pub(crate) search_query: String,
    pub(crate) status: CommunityStatus,
    pub(crate) active_tab: CommunityTab,
    /// Owned search-box input entity, created lazily while the Plugins group
    /// is active so typing survives re-renders.
    pub(crate) search_input: Option<Entity<InputState>>,
    pub(crate) search_input_subscription: Option<Subscription>,
    /// Guards against stale async index-fetch completions.
    pub(crate) generation: u64,
}

impl CommunityPluginsState {
    fn begin_load(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.status = CommunityStatus::Loading;
        self.generation
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

/// Result of a community search, precomputed for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommunityAction {
    /// Not shown / not installable on this host at this Lumia version.
    Incompatible,
    Install,
    Update,
    Installed,
}

/// Path of the cached index under the per-user data directory.
fn community_index_cache_path() -> Option<PathBuf> {
    let application_dir = if cfg!(target_os = "linux") {
        "lumia"
    } else {
        "Lumia"
    };
    dirs::data_dir().map(|data_dir| data_dir.join(application_dir).join(COMMUNITY_INDEX_FILE))
}

fn load_cached_index() -> Option<Arc<CommunityIndex>> {
    let bytes = community_index_cache_path().and_then(|path| std::fs::read(path).ok())?;
    parse_index(&bytes).ok().map(Arc::new)
}

fn save_cached_index(index: &CommunityIndex) {
    let Some(path) = community_index_cache_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(json) = serde_json::to_string_pretty(index) else {
        return;
    };
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, json).is_err() {
        return;
    }
    let _ = std::fs::rename(temporary, path);
}

impl LumiaApp {
    fn community_browser_visible(&self) -> bool {
        self.ui.show_settings_panel
            && matches!(self.ui.active_settings_group, SettingsGroup::Plugins)
    }

    /// Creates the search-box input while the community browser is visible,
    /// drops it otherwise — mirroring `sync_annotation_text_input`.
    pub(crate) fn sync_community_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.community_browser_visible() {
            if self.community_plugins.search_input.is_none() {
                let placeholder =
                    tr_community(self.settings.language, CommunityTextKey::CommunitySearchPlaceholder)
                        .to_string();
                let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                let subscription =
                    cx.subscribe_in(&input, window, Self::handle_community_search_input);
                self.community_plugins.search_input = Some(input);
                self.community_plugins.search_input_subscription = Some(subscription);
            }
        } else if self.community_plugins.search_input.is_some() {
            self.community_plugins.search_input = None;
            self.community_plugins.search_input_subscription = None;
        }
    }

    fn handle_community_search_input(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        self.community_plugins.search_query = input.read(cx).value().to_string();
        cx.notify();
    }

    /// Fetches (and caches) the community index. `manual=true` surfaces errors;
    /// the lazy startup load stays silent on failure, like `check_for_updates`.
    pub(crate) fn load_community_index(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.community_plugins.status.is_busy() {
            return;
        }
        // Seed from cache first so the browser renders while offline.
        if self.community_plugins.index.is_none() {
            self.community_plugins.index = load_cached_index();
        }
        let generation = self.community_plugins.begin_load();
        cx.notify();

        let client = cx.http_client();
        let handle = self.self_handle.clone();
        cx.spawn(async move |_this, cx| {
            let result = fetch_index(&client).await;
            let _ = handle.update(cx, |this, cx| {
                if !this.community_plugins.is_current(generation) {
                    return;
                }
                match result {
                    Ok(index) => {
                        save_cached_index(&index);
                        this.community_plugins.index = Some(Arc::new(index));
                        this.community_plugins.status = CommunityStatus::Loaded;
                    }
                    Err(error) => {
                        if manual {
                            this.community_plugins.status =
                                CommunityStatus::Error(format!("{error:#}"));
                        } else if this.community_plugins.index.is_some() {
                            // Keep showing the cached copy; the banner shows the cache hint.
                            this.community_plugins.status = CommunityStatus::Loaded;
                        } else {
                            this.community_plugins.status = CommunityStatus::Idle;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Refreshes the community index on demand, surfacing errors.
    pub(crate) fn refresh_community_index(&mut self, cx: &mut Context<Self>) {
        self.load_community_index(true, cx);
    }

    pub(crate) fn set_community_tab(&mut self, tab: CommunityTab, cx: &mut Context<Self>) {
        self.community_plugins.active_tab = tab;
        cx.notify();
    }

    /// Downloads the best compatible artifact for `plugin_id` into a temp file,
    /// verifies its SHA-256, then hands it to the existing local-file install
    /// pipeline (`inspect_plugin_package`) for signature + compatibility
    /// verification and the user confirmation card.
    pub(crate) fn install_community_plugin(&mut self, plugin_id: String, cx: &mut Context<Self>) {
        let Some(index) = self.community_plugins.index.clone() else {
            return;
        };
        let Some(plugin) = index.plugins.iter().find(|plugin| plugin.id == plugin_id) else {
            return;
        };
        let current = Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("CARGO_PKG_VERSION is valid semver");
        let Some((version, artifact)) = best_compatible_artifact_for_host(plugin, &current) else {
            // Distinguish "no artifact for this platform" from "incompatible
            // with this Lumia build" so the user gets an actionable message.
            let message = if best_artifact_for_host(plugin).is_some() {
                tr_community(self.settings.language, CommunityTextKey::CommunityIncompatible)
                    .to_string()
            } else {
                tr_community(self.settings.language, CommunityTextKey::CommunityRequiresLumia)
                    .to_string()
            };
            self.community_plugins.status = CommunityStatus::Error(message);
            cx.notify();
            return;
        };

        let client = cx.http_client();
        let handle = self.self_handle.clone();
        let url = artifact.url.clone();
        let sha256 = artifact.sha256.clone();
        let size = artifact.size;
        let download_id = plugin.id.clone();
        let version_str = version.version.clone();
        let os = artifact.target_os.clone();
        let arch = artifact.target_arch.clone();
        self.community_plugins.status = CommunityStatus::Downloading {
            plugin_id: plugin.id.clone(),
            downloaded_bytes: 0,
            total_bytes: Some(size),
        };
        cx.notify();

        let dest = std::env::temp_dir().join(format!(
            "{download_id}-{version_str}-{os}-{arch}.lumiaplugin"
        ));
        cx.spawn(async move |_this, cx| {
            let cleanup_dest = dest.clone();
            let result: anyhow::Result<PathBuf> = async {
                let response =
                    client.get(&url, http_client::AsyncBody::from(()), true).await?;
                if !response.status().is_success() {
                    anyhow::bail!("http status {}", response.status());
                }
                let (_parts, mut body) = response.into_parts();
                let mut file = std::fs::File::create(&dest)?;
                let mut hasher = Sha256::new();
                let mut downloaded: u64 = 0;
                let mut buffer = [0_u8; 8192];
                let mut last_notify = Instant::now();
                loop {
                    let n = futures::io::AsyncReadExt::read(&mut body, &mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    downloaded += n as u64;
                    if downloaded > MAX_PACKAGE_DOWNLOAD_BYTES {
                        anyhow::bail!(
                            "package exceeds the {} MiB download limit",
                            MAX_PACKAGE_DOWNLOAD_BYTES / 1024 / 1024
                        );
                    }
                    std::io::Write::write_all(&mut file, &buffer[..n])?;
                    hasher.update(&buffer[..n]);
                    if last_notify.elapsed() >= DOWNLOAD_PROGRESS_INTERVAL {
                        last_notify = Instant::now();
                        let _ = handle.update(cx, |this, cx| {
                            if let CommunityStatus::Downloading { .. } =
                                this.community_plugins.status
                            {
                                this.community_plugins.status = CommunityStatus::Downloading {
                                    plugin_id: download_id.clone(),
                                    downloaded_bytes: downloaded,
                                    total_bytes: Some(size),
                                };
                            }
                            cx.notify();
                        });
                    }
                }
                if size != 0 && downloaded != size {
                    anyhow::bail!("download incomplete: {downloaded}/{size} bytes");
                }
                let digest = hex::encode(hasher.finalize());
                if !digest.eq_ignore_ascii_case(&sha256) {
                    anyhow::bail!("download failed SHA-256 verification");
                }
                Ok(dest)
            }
            .await;

            let _ = handle.update(cx, |this, cx| {
                match result {
                    Ok(path) => {
                        this.community_plugins.status = CommunityStatus::Idle;
                        this.inspect_plugin_package(path, cx);
                    }
                    Err(error) => {
                        let _ = std::fs::remove_file(&cleanup_dest);
                        this.community_plugins.status =
                            CommunityStatus::Error(format!("{error:#}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// Community plugins matching the current search query, sorted by name,
    /// with the action the result card should offer.
    pub(crate) fn community_search_results(&self) -> Vec<(&CommunityPlugin, CommunityAction)> {
        let Some(index) = self.community_plugins.index.as_ref() else {
            return Vec::new();
        };
        let query = self.community_plugins.search_query.clone();
        let current = Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("CARGO_PKG_VERSION is valid semver");
        let mut results: Vec<&CommunityPlugin> = index
            .plugins
            .iter()
            .filter(|plugin| matches_query(plugin, &query))
            .collect();
        results.sort_by(|left, right| left.name.cmp(&right.name));
        results
            .into_iter()
            .map(|plugin| {
                let installed = self
                    .plugin_management
                    .installed
                    .iter()
                    .find(|managed| managed.id == plugin.id);
                let action = match installed {
                    None if best_compatible_artifact_for_host(plugin, &current).is_some() => {
                        CommunityAction::Install
                    }
                    None => CommunityAction::Incompatible,
                    Some(managed)
                        if best_compatible_artifact_for_host(plugin, &current)
                            .is_some_and(|(version, _)| version.version != managed.version) =>
                    {
                        CommunityAction::Update
                    }
                    Some(_) => CommunityAction::Installed,
                };
                (plugin, action)
            })
            .collect()
    }
}

async fn fetch_index(client: &Arc<dyn http_client::HttpClient>) -> anyhow::Result<CommunityIndex> {
    let response = client
        .get(COMMUNITY_INDEX_URL, http_client::AsyncBody::from(()), true)
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("http status {}", response.status());
    }
    let (_parts, mut body) = response.into_parts();
    let mut bytes = Vec::new();
    futures::io::AsyncReadExt::read_to_end(&mut body, &mut bytes).await?;
    if bytes.len() > MAX_COMMUNITY_INDEX_BYTES {
        anyhow::bail!(
            "community plugin index exceeds {} MiB",
            MAX_COMMUNITY_INDEX_BYTES / 1024 / 1024
        );
    }
    parse_index(&bytes)
}
