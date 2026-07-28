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
    let notes = aggregate_release_notes(&entries, &v(0, 1, 2), &v(0, 1, 4), Some("fallback notes"));
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
fn select_asset_picks_the_platform_installer() {
    #[cfg(target_os = "windows")]
    let expected = "Lumia-Setup-0.1.3-x64.exe";
    #[cfg(target_os = "macos")]
    let expected = "Lumia-macos-arm64.dmg";
    #[cfg(all(unix, not(target_os = "macos")))]
    let expected = "lumia-linux-x64.tar.gz";

    let assets = vec![
        GithubAsset {
            name: "lumia-portable-windows-x64.zip".into(),
            browser_download_url: "https://example.com/portable.zip".into(),
            size: 100,
        },
        GithubAsset {
            name: expected.into(),
            browser_download_url: "https://example.com/installer".into(),
            size: 200,
        },
        GithubAsset {
            name: "Lumia-Annotation-windows-x64.lumiaplugin".into(),
            browser_download_url: "https://example.com/plugin".into(),
            size: 50,
        },
    ];
    let selected = select_asset(&assets).expect("platform asset");
    assert_eq!(selected.name, expected);
    assert_eq!(selected.size, 200);
}

#[test]
fn select_asset_returns_none_when_no_match() {
    let assets = vec![GithubAsset {
        name: "Lumia-Annotation-windows-x64.lumiaplugin".into(),
        browser_download_url: "https://example.com".into(),
        size: 50,
    }];
    assert!(select_asset(&assets).is_none());
}

#[test]
fn update_check_ui_state_helpers() {
    let mut state = UpdateCheckUiState::default();
    assert!(!state.is_busy());
    assert!(!state.has_update());

    state.state = UpdateState::Checking;
    assert!(state.is_busy());

    state.state = UpdateState::Available {
        latest_version: v(0, 2, 0),
        release_notes: "notes".into(),
        asset: UpdateAsset {
            name: "Lumia-Setup-0.2.0-x64.exe".into(),
            url: "https://example.com".into(),
            size: 100,
        },
    };
    assert!(!state.is_busy());
    assert!(state.has_update());

    state.state = UpdateState::Downloading {
        latest_version: v(0, 2, 0),
        downloaded_bytes: 50,
        total_bytes: Some(100),
    };
    assert!(state.is_busy());

    state.state = UpdateState::Installing;
    assert!(state.is_busy());

    state.state = UpdateState::UpToDate;
    assert!(!state.is_busy());
    assert!(!state.has_update());

    state.state = UpdateState::Error("err".into());
    assert!(!state.is_busy());
    assert!(!state.has_update());
}
