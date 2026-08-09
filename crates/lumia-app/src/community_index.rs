//! Community plugin index: data model, parsing, search, and platform/version
//! selection. Pure data + pure functions, no gpui — kept in its own module so
//! the app-side state and I/O stay under the project's module size limit.
//!
//! The index (`plugins.json`) lives in the `awesome-lumia-plugin` repository and
//! is the single source of truth for discoverable third-party plugins. It is
//! advisory only: it is never trusted for code execution. The authority is the
//! Ed25519 signature on the `.lumiaplugin` package, which the existing install
//! pipeline verifies via `inspect_official_package` before anything is staged.

use lumia_plugin_api::PluginPermission;
use semver::Version;

use crate::plugin_package::{current_target_arch, current_target_os};

/// Highest `schema_version` this build understands. Unknown-larger schemas are
/// rejected so a future incompatible index fails loudly instead of misparsing.
const COMMUNITY_INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommunityIndex {
    pub(crate) schema_version: u32,
    #[serde(default)]
    pub(crate) index_version: String,
    #[serde(default)]
    pub(crate) plugins: Vec<CommunityPlugin>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommunityPlugin {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) author: Option<CommunityAuthor>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub(crate) repository: Option<String>,
    #[serde(default)]
    pub(crate) website: Option<String>,
    pub(crate) versions: Vec<CommunityPluginVersion>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommunityAuthor {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommunityPluginVersion {
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) minimum_lumia_version: String,
    #[serde(default)]
    pub(crate) plugin_api_version: u32,
    #[serde(default)]
    pub(crate) install_directory: String,
    #[serde(default)]
    pub(crate) artifacts: Vec<CommunityArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommunityArtifact {
    pub(crate) target_os: String,
    pub(crate) target_arch: String,
    pub(crate) url: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

/// Parses and validates the community index. Rejects unknown-larger schema
/// versions so the app never misparses a future incompatible index.
pub(crate) fn parse_index(bytes: &[u8]) -> anyhow::Result<CommunityIndex> {
    let index: CommunityIndex = serde_json::from_slice(bytes)?;
    if index.schema_version > COMMUNITY_INDEX_SCHEMA_VERSION {
        anyhow::bail!(
            "community plugin index schema {} is newer than supported {}",
            index.schema_version,
            COMMUNITY_INDEX_SCHEMA_VERSION
        );
    }
    Ok(index)
}

/// Picks the best (version, artifact) pair for this host: filters artifacts by
/// the current OS/architecture, then takes the highest version.
pub(crate) fn best_artifact_for_host(
    plugin: &CommunityPlugin,
) -> Option<(&CommunityPluginVersion, &CommunityArtifact)> {
    let os = current_target_os();
    let arch = current_target_arch();
    plugin
        .versions
        .iter()
        .filter_map(|version| {
            version
                .artifacts
                .iter()
                .find(|artifact| artifact.target_os == os && artifact.target_arch == arch)
                .map(|artifact| (version, artifact))
        })
        .max_by_key(|(version, _)| parse_version(version))
}

/// Like `best_artifact_for_host`, but only considers versions compatible with
/// the running Lumia build. Advisory: the signed package is re-validated by
/// `validate_compatibility` at install time.
pub(crate) fn best_compatible_artifact_for_host<'a>(
    plugin: &'a CommunityPlugin,
    current_lumia: &Version,
) -> Option<(&'a CommunityPluginVersion, &'a CommunityArtifact)> {
    plugin
        .versions
        .iter()
        .filter(|version| is_compatible(version, current_lumia))
        .filter_map(|version| {
            version
                .artifacts
                .iter()
                .find(|artifact| {
                    artifact.target_os == current_target_os()
                        && artifact.target_arch == current_target_arch()
                })
                .map(|artifact| (version, artifact))
        })
        .max_by_key(|(version, _)| parse_version(version))
}

/// Case-insensitive substring search across id, name, description, tags, and
/// author name. An empty query matches everything.
pub(crate) fn matches_query(plugin: &CommunityPlugin, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let haystacks: Vec<String> = std::iter::once(plugin.id.clone())
        .chain(std::iter::once(plugin.name.clone()))
        .chain(std::iter::once(plugin.description.clone()))
        .chain(plugin.tags.clone())
        .chain(
            plugin
                .author
                .as_ref()
                .map(|author| author.name.clone())
                .into_iter(),
        )
        .collect();
    haystacks
        .iter()
        .any(|haystack| haystack.to_lowercase().contains(&query))
}

/// Advisory compatibility check from the index. The authoritative check still
/// runs inside `validate_compatibility` against the signed package manifest.
pub(crate) fn is_compatible(version: &CommunityPluginVersion, current_lumia: &Version) -> bool {
    if version.plugin_api_version != 0
        && version.plugin_api_version != lumia_plugin_api::PROTOCOL_VERSION
    {
        return false;
    }
    let Ok(minimum) = Version::parse(&version.minimum_lumia_version) else {
        return true;
    };
    minimum <= *current_lumia
}

fn parse_version(version: &CommunityPluginVersion) -> Version {
    Version::parse(&version.version).unwrap_or_else(|_| Version::new(0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_plugin_api::PluginPermission as Permission;

    fn version(
        os: &str,
        arch: &str,
        version: &str,
        plugin_api_version: u32,
        minimum_lumia_version: &str,
    ) -> CommunityPluginVersion {
        CommunityPluginVersion {
            version: version.into(),
            minimum_lumia_version: minimum_lumia_version.into(),
            plugin_api_version,
            install_directory: "foo".into(),
            artifacts: vec![CommunityArtifact {
                target_os: os.into(),
                target_arch: arch.into(),
                url: format!("https://example.com/{version}.lumiaplugin"),
                sha256: "a".repeat(64),
                size: 1,
            }],
        }
    }

    fn plugin(id: &str) -> CommunityPlugin {
        CommunityPlugin {
            id: id.into(),
            name: id.into(),
            description: "A test plugin".into(),
            author: None,
            tags: vec!["decoder".into()],
            permissions: vec![Permission::ReadInputPath],
            repository: None,
            website: None,
            versions: Vec::new(),
        }
    }

    #[test]
    fn parse_index_accepts_valid_and_rejects_unknown_schema() {
        let index = serde_json::json!({
            "schema_version": 1,
            "index_version": "2026-08-09T00:00:00Z",
            "plugins": [{
                "id": "com.example.foo",
                "name": "Foo",
                "versions": []
            }]
        });
        let parsed = parse_index(&serde_json::to_vec(&index).unwrap()).unwrap();
        assert_eq!(parsed.plugins.len(), 1);

        let bad = serde_json::json!({ "schema_version": 2, "plugins": [] });
        assert!(parse_index(&serde_json::to_vec(&bad).unwrap()).is_err());
    }

    #[test]
    fn best_artifact_selects_current_platform_highest_version() {
        let mut p = plugin("com.example.foo");
        // A higher version exists on a foreign platform; the host-platform
        // version must be selected, proving the platform filter runs first.
        let other_os = if current_target_os() == "windows" {
            "linux"
        } else {
            "windows"
        };
        p.versions = vec![
            version(other_os, "x86_64", "9.0.0", 3, "0.1.0"),
            version(
                current_target_os(),
                current_target_arch(),
                "0.5.0",
                3,
                "0.1.0",
            ),
        ];
        let (version, _) = best_artifact_for_host(&p).unwrap();
        assert_eq!(version.version, "0.5.0");
    }

    #[test]
    fn best_compatible_artifact_skips_incompatible_versions() {
        let mut p = plugin("com.example.foo");
        p.versions = vec![
            version(
                current_target_os(),
                current_target_arch(),
                "9.0.0",
                lumia_plugin_api::PROTOCOL_VERSION + 1,
                "0.1.0",
            ),
            version(
                current_target_os(),
                current_target_arch(),
                "1.0.0",
                3,
                "0.1.0",
            ),
        ];
        let current = Version::new(0, 2, 1);
        let (version, _) = best_compatible_artifact_for_host(&p, &current).unwrap();
        assert_eq!(version.version, "1.0.0");
    }

    #[test]
    fn search_matches_id_name_description_and_tags_case_insensitive() {
        let mut p = plugin("com.example.foo");
        p.name = "Foo Converter".into();
        p.description = "Convert HEIF to PNG".into();
        p.tags = vec!["batch".into()];
        p.author = Some(CommunityAuthor {
            name: "Jane Dev".into(),
            url: None,
        });

        assert!(matches_query(&p, "FOO"));
        assert!(matches_query(&p, "convert"));
        assert!(matches_query(&p, "BATCH"));
        assert!(matches_query(&p, "jane"));
        assert!(matches_query(&p, "  "));
        assert!(!matches_query(&p, "xyzzy"));
    }

    #[test]
    fn compat_rejects_wrong_api_version_and_newer_minimum() {
        assert!(!is_compatible(
            &version("windows", "x86_64", "1.0.0", 3, "9.0.0"),
            &Version::new(0, 2, 1)
        ));
        assert!(is_compatible(
            &version("windows", "x86_64", "1.0.0", 3, "0.1.0"),
            &Version::new(0, 2, 1)
        ));
    }
}
