use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

/// Asset source that wraps the default gpui_component_assets to provide
/// custom SVG icons on top of the built-in icon set.
pub(crate) struct CustomAssets;

impl AssetSource for CustomAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "custom/actual-size.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../resources/icons/actual-size.svg"
            )))),
            "custom/fit-to-window.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../resources/icons/fit-to-window.svg"
            )))),
            other => gpui_component_assets::Assets.load(other),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path == "custom" || path == "custom/" {
            Ok(vec![
                "custom/actual-size.svg".into(),
                "custom/fit-to-window.svg".into(),
            ])
        } else {
            gpui_component_assets::Assets.list(path)
        }
    }
}
