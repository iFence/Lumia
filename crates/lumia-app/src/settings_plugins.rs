use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui_component::input::Input;

use crate::app::LumiaApp;
use crate::community_index::CommunityPlugin;
use crate::community_plugins::{CommunityAction, CommunityStatus, CommunityTab};
use crate::community_text::{tr_community, CommunityTextKey};
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::settings_installed_plugins::permission_summary;
use crate::widgets::{edit_option_button, settings_action_button};

impl LumiaApp {
    pub(crate) fn render_plugin_settings(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        div()
            .id("settings-plugins")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(tr(language, TextKey::Plugins)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(tr_community(language, CommunityTextKey::CommunityPluginsDescription)),
                    ),
            )
            .child(self.render_plugin_sub_tabs(palette, cx))
            .children(match self.community_plugins.active_tab {
                CommunityTab::Community => self
                    .render_community_browser(palette, cx)
                    .map(|element| vec![element])
                    .unwrap_or_default(),
                CommunityTab::Installed => self
                    .render_installed_plugins(palette, cx)
                    .map(|element| vec![element])
                    .unwrap_or_default(),
            })
    }

    fn render_plugin_sub_tabs(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.language;
        div()
            .id("settings-plugin-tabs")
            .flex()
            .items_center()
            .gap_2()
            .child(edit_option_button(
                "plugin-tab-community",
                tr_community(language, CommunityTextKey::Community),
                self.community_plugins.active_tab == CommunityTab::Community,
                palette,
                cx,
                move |this, _, _, cx| {
                    this.set_community_tab(CommunityTab::Community, cx);
                },
            ))
            .child(edit_option_button(
                "plugin-tab-installed",
                tr_community(language, CommunityTextKey::Installed),
                self.community_plugins.active_tab == CommunityTab::Installed,
                palette,
                cx,
                move |this, _, _, cx| {
                    this.set_community_tab(CommunityTab::Installed, cx);
                },
            ))
    }

    // ---------------------------------------------------------------------
    // Community browser
    // ---------------------------------------------------------------------

    fn render_community_browser(
        &self,
        palette: Palette,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let language = self.settings.language;
        let mut content = div().flex().flex_col().gap_3();

        // Search box + refresh row.
        let refresh_handle = self.self_handle.clone();
        let search_input = self.community_plugins.search_input.as_ref();
        let search_row = div().flex().items_center().gap_2();
        let search_row = match search_input {
            Some(input) => search_row.child(div().flex_1().child(Input::new(input))),
            None => search_row.child(div().flex_1()),
        };
        content = content.child(search_row.child(settings_action_button(
            "refresh-community-index",
            tr_community(language, CommunityTextKey::RefreshPlugins),
            false,
            self.community_plugins.status.is_busy(),
            move |_, _, cx| {
                let _ = refresh_handle.update(cx, |this, cx| {
                    this.refresh_community_index(cx);
                });
            },
        )));

        // Status banner.
        content = content.children(self.render_community_status(palette));

        // Result list.
        let results = self.community_search_results();
        if results.is_empty() {
            content = content.child(
                div()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(tr_community(language, CommunityTextKey::NoCommunityResults)),
            );
        } else {
            for (plugin, action) in results {
                content = content.child(self.render_community_plugin_card(plugin, action, palette));
            }
        }

        Some(content.into_any_element())
    }

    fn render_community_status(&self, palette: Palette) -> Option<AnyElement> {
        let language = self.settings.language;
        let (message, is_error) = match &self.community_plugins.status {
            CommunityStatus::Idle | CommunityStatus::Loaded => return None,
            CommunityStatus::Loading => {
                (tr_community(language, CommunityTextKey::CommunityLoading).to_string(), false)
            }
            CommunityStatus::Downloading {
                plugin_id,
                downloaded_bytes,
                total_bytes,
            } => {
                let progress = match total_bytes {
                    Some(total) if *total > 0 => format!(
                        " {} / {}",
                        crate::util::format_file_size(*downloaded_bytes),
                        crate::util::format_file_size(*total)
                    ),
                    _ => format!(" {}", crate::util::format_file_size(*downloaded_bytes)),
                };
                (
                    format!(
                        "{}: {plugin_id}{progress}",
                        tr_community(language, CommunityTextKey::CommunityDownloading)
                    ),
                    false,
                )
            }
            CommunityStatus::Error(message) => (message.clone(), true),
        };
        Some(
            div()
                .p_3()
                .rounded_md()
                .bg(rgb(palette.sidebar_bg))
                .text_sm()
                .text_color(rgb(if is_error {
                    palette.error_text
                } else {
                    palette.text
                }))
                .child(message)
                .into_any_element(),
        )
    }

    fn render_community_plugin_card(
        &self,
        plugin: &CommunityPlugin,
        action: CommunityAction,
        palette: Palette,
    ) -> AnyElement {
        let language = self.settings.language;
        let plugin_id = plugin.id.clone();
        let install_handle = self.self_handle.clone();
        let busy = self.community_plugins.status.is_busy();
        let version_text = plugin
            .versions
            .iter()
            .max_by(|left, right| {
                semver::Version::parse(&left.version)
                    .unwrap_or_else(|_| semver::Version::new(0, 0, 0))
                    .cmp(
                        &semver::Version::parse(&right.version)
                            .unwrap_or_else(|_| semver::Version::new(0, 0, 0)),
                    )
            })
            .map(|version| version.version.clone())
            .unwrap_or_default();

        let permission_summary =
            permission_summary(language, &plugin.permissions, TextKey::NoPermissions);
        let author_text = plugin
            .author
            .as_ref()
            .map(|author| author.name.clone())
            .map(|name| format!("  {}  {name}", tr_community(language, CommunityTextKey::CommunityAuthor)))
            .unwrap_or_default();

        // Action button. Self-drawn div (matching the installed-plugin Remove
        // button) instead of gpui-component Button: the component's base is
        // `flex_shrink_0`, so a long label like the "incompatible" hint would
        // push the card wider than its container.
        let (button_label, button_primary, button_disabled) = match action {
            CommunityAction::Incompatible => {
                (tr_community(language, CommunityTextKey::CommunityIncompatible), false, true)
            }
            CommunityAction::Install => {
                (tr_community(language, CommunityTextKey::CommunityInstall), true, busy)
            }
            CommunityAction::Update => {
                (tr_community(language, CommunityTextKey::CommunityUpdate), true, busy)
            }
            CommunityAction::Installed => {
                (tr_community(language, CommunityTextKey::CommunityInstalled), false, true)
            }
        };
        let install_handle = install_handle.clone();
        let install_id = plugin_id.clone();
        let action_button = div()
            .id(format!("install-community-plugin-{plugin_id}"))
            .flex_none()
            .min_w(px(64.0))
            .h(px(32.0))
            .px_3()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(if button_primary {
                palette.accent
            } else {
                palette.border
            }))
            .bg(rgb(if button_primary {
                palette.accent_soft
            } else {
                palette.sidebar_bg
            }))
            .text_xs()
            .text_color(rgb(if button_disabled {
                palette.muted_text
            } else if button_primary {
                palette.text
            } else {
                palette.text
            }))
            .when(!button_disabled, |button| {
                button
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(palette.button_hover)))
            })
            .on_click(move |_, _, cx| {
                let install_id = install_id.clone();
                let _ = install_handle.update(cx, |this, cx| {
                    this.install_community_plugin(install_id, cx);
                });
            })
            .child(div().truncate().child(button_label));

        div()
            .id(format!("community-plugin-{plugin_id}"))
            .w_full()
            .p_4()
            .rounded_md()
            .border_1()
            .border_color(rgb(palette.border))
            .flex()
            .items_start()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{}  {version_text}", plugin.name)),
                    )
                    .when(!plugin.description.is_empty(), |card| {
                        card.child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(palette.muted_text))
                                .child(plugin.description.clone()),
                        )
                    })
                    .when(!author_text.is_empty(), |card| {
                        card.child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(palette.muted_text))
                                .child(author_text),
                        )
                    })
                    .when(!plugin.tags.is_empty(), |card| {
                        card.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .children(plugin.tags.iter().map(|tag| {
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_full()
                                        .border_1()
                                        .border_color(rgb(palette.border))
                                        .text_xs()
                                        .text_color(rgb(palette.muted_text))
                                        .child(tag.clone())
                                })),
                        )
                    })
                    .when(!plugin.permissions.is_empty(), |card| {
                        card.child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(palette.muted_text))
                                .child(format!(
                                    "{}: {permission_summary}",
                                    tr(language, TextKey::Permissions)
                                )),
                        )
                    }),
            )
            .child(action_button)
            .into_any_element()
    }
}
