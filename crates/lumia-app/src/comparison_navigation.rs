//! Folder navigation for comparison mode.
//!
//! The two panes hold images from independent folders, so each one walks its
//! own sibling list. With no pane selected a step advances both; with one
//! selected only that pane moves, matching how zoom, pan and rotation already
//! narrow to the selection.

use std::path::Path;

use gpui::{Context, Window};
use lumia_core::FolderNavigation;

use crate::app::LumiaApp;
use crate::load_state::PreparedImage;

impl LumiaApp {
    /// Step the comparison panes. Falls back to single-pane stepping outside
    /// comparison mode.
    pub(crate) fn navigate_comparison(
        &mut self,
        step: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .comparison
            .as_ref()
            .and_then(|state| state.individual_target);
        if target != Some(true) {
            self.navigate_current_image(step, window, cx);
        }
        if target != Some(false) {
            self.navigate_comparison_pane(step, cx);
        }
    }

    /// Whether a step would move at least one pane. Panes walk their own
    /// folders, so a step is offered whenever either of them can advance.
    pub(crate) fn comparison_can_step(&self, step: i32) -> bool {
        if step == 0 {
            return false;
        }
        let Some(state) = self.comparison.as_ref() else {
            return false;
        };
        let target = state.individual_target;
        let left_can = target != Some(true) && self.navigation.len() > 1;
        let right_can = target != Some(false) && state.navigation.len() > 1;
        match target {
            Some(true) => right_can,
            Some(false) => left_can,
            None => left_can || right_can,
        }
    }

    /// Step the right pane through its own folder.
    fn navigate_comparison_pane(&mut self, step: i32, cx: &mut Context<Self>) {
        let next = self.comparison.as_ref().and_then(|state| {
            state
                .navigation
                .step_path(&state.path, step)
                .map(Path::to_path_buf)
        });
        let Some(path) = next else {
            return;
        };
        // Stepping onto the file already shown is a no-op, which is what a
        // single-image folder does on every step.
        if self
            .comparison
            .as_ref()
            .is_some_and(|state| state.path == path)
        {
            return;
        }
        self.load_comparison_image(path, cx);
    }

    /// Decode `path` into the right pane and rescan its folder when the step
    /// lands outside the list currently held.
    pub(crate) fn load_comparison_image(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.comparison.as_mut() else {
            return;
        };
        let needs_scan = !state.navigation.contains(&path);
        state.begin_load(path.clone());

        let handle = self.self_handle.clone();
        cx.spawn(async move |_, cx| {
            let decode_path = path.clone();
            let decode = cx
                .background_executor()
                .spawn(async move {
                    crate::comparison_transform::load_decoded_image_for_comparison(&decode_path)
                })
                .await;
            let scan = needs_scan.then(|| {
                let scan_path = path.clone();
                cx.background_executor()
                    .spawn(async move { FolderNavigation::scan(&scan_path) })
            });
            let catalog = match scan {
                Some(task) => task.await.ok(),
                None => None,
            };
            let size = {
                let size_path = path.clone();
                cx.background_executor()
                    .spawn(async move { std::fs::metadata(&size_path).map(|meta| meta.len()).ok() })
                    .await
            };
            let _ = handle.update(cx, |this, cx| {
                let Some(state) = this.comparison.as_mut() else {
                    return;
                };
                // A newer navigation already repointed the pane, so this
                // decode is stale and must not overwrite the newer image.
                if state.path != path {
                    return;
                }
                if let Some(catalog) = catalog {
                    state.navigation = catalog;
                }
                state.file_size = size;
                state.loading = false;
                // Same reasoning as the left pane: the resample cache is only
                // valid for the bitmap it was produced from.
                state.right_resample.invalidate();
                match decode {
                    Ok(decoded) => {
                        state.error = None;
                        state.image = Some(PreparedImage::from_decoded(decoded));
                        // A rotation requested while the decode was in flight
                        // applies as soon as the image arrives.
                        state.rebuild_rotated_image();
                    }
                    Err(error) => {
                        state.error = Some(error.to_string());
                        state.clear_image();
                    }
                }
                if state.take_pending_fit_reset() {
                    state.viewport.reset_fit();
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparison::{next_pane_target, ComparisonState};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-compare-nav-{tag}-{nonce}"))
    }

    fn comparison(path: std::path::PathBuf, navigation: FolderNavigation) -> ComparisonState {
        let mut state = ComparisonState::new(path);
        state.navigation = navigation;
        state
    }

    #[test]
    fn clicking_a_pane_chip_toggles_its_target() {
        assert_eq!(next_pane_target(None, true), Some(true));
        assert_eq!(next_pane_target(None, false), Some(false));
        assert_eq!(
            next_pane_target(Some(true), true),
            None,
            "clicking the targeted pane clears the target so both panes move together"
        );
        assert_eq!(
            next_pane_target(Some(true), false),
            Some(false),
            "clicking the other pane moves the target across"
        );
        assert_eq!(next_pane_target(Some(false), false), None);
        assert_eq!(next_pane_target(Some(false), true), Some(true));
    }

    #[test]
    fn can_step_requires_a_sibling_somewhere() {
        let dir = temp_dir("step");
        std::fs::create_dir(&dir).unwrap();
        for name in ["a.jpg", "b.jpg"] {
            std::fs::write(dir.join(name), []).unwrap();
        }
        let navigation = FolderNavigation::scan(&dir.join("a.jpg")).unwrap();
        let state = comparison(dir.join("a.jpg"), navigation);
        // The folder holds two images, so the right pane alone can advance.
        assert!(state.navigation.len() > 1);
        assert_eq!(
            state.navigation.step_path(&dir.join("a.jpg"), 1),
            Some(dir.join("b.jpg").as_path())
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pane_with_no_siblings_never_steps() {
        let dir = temp_dir("lonely");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("only.jpg"), []).unwrap();
        let navigation = FolderNavigation::scan(&dir.join("only.jpg")).unwrap();
        assert_eq!(navigation.len(), 1);
        let current = dir.join("only.jpg");
        assert_eq!(
            navigation.step_path(&current, 1),
            Some(current.as_path()),
            "wraparound keeps a single-image folder in place"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn both_panes_step_independently_through_their_own_folders() {
        let left_dir = temp_dir("left");
        let right_dir = temp_dir("right");
        std::fs::create_dir(&left_dir).unwrap();
        std::fs::create_dir(&right_dir).unwrap();
        for name in ["l1.jpg", "l2.jpg", "l3.jpg"] {
            std::fs::write(left_dir.join(name), []).unwrap();
        }
        for name in ["r1.png", "r2.png"] {
            std::fs::write(right_dir.join(name), []).unwrap();
        }

        let left = FolderNavigation::scan(&left_dir.join("l1.jpg")).unwrap();
        let right = FolderNavigation::scan(&right_dir.join("r1.png")).unwrap();
        let mut left_path = left_dir.join("l1.jpg");
        let mut right_path = right_dir.join("r1.png");

        for expected in [
            ("l2.jpg", "r2.png"),
            ("l3.jpg", "r1.png"),
            ("l1.jpg", "r2.png"),
        ] {
            left_path = left
                .step_path(&left_path, 1)
                .expect("left folder has three images")
                .to_path_buf();
            right_path = right
                .step_path(&right_path, 1)
                .expect("right folder has two images")
                .to_path_buf();
            assert_eq!(left_path.file_name().unwrap(), expected.0);
            assert_eq!(right_path.file_name().unwrap(), expected.1);
        }

        std::fs::remove_dir_all(left_dir).unwrap();
        std::fs::remove_dir_all(right_dir).unwrap();
    }
}
