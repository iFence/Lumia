use gpui::{
    div, img, px, rgb, App, Context, ExternalPaths, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, StatefulInteractiveElement, Styled, StyledImage, Window,
};
use gpui_component::{Root, Theme, ThemeMode as ComponentThemeMode};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::util::status_message;
use crate::Quit;

impl Render for LumiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window_title(window);
        let component_theme_mode = ComponentThemeMode::Dark;
        if Theme::global(cx).mode != component_theme_mode {
            Theme::change(component_theme_mode, None, cx);
        }

        let palette = self.palette(window);
        self.sync_annotation_text_input(window, cx);
        self.sync_community_search_input(window, cx);
        self.schedule_viewer_resamples(window, cx);
        self.refresh_large_image_tiles(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);

        div()
            .id("lumia-root")
            .track_focus(&self.focus_handle)
            .key_context(if self.comparison_active() {
                "Lumia Comparison"
            } else {
                "Lumia"
            })
            .relative()
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::zoom_fit))
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::toggle_image_info))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::open_about))
            .on_action(cx.listener(Self::next_image))
            .on_action(cx.listener(Self::previous_image))
            .on_action(cx.listener(Self::rotate_clockwise))
            .on_action(cx.listener(Self::compare_images))
            .on_action(cx.listener(Self::toggle_comparison_target_action))
            .on_action(cx.listener(Self::close_comparison_action))
            .on_action(cx.listener(Self::apply_selected_resample_filter))
            .on_action(cx.listener(Self::rotate_counter_clockwise))
            .on_action(|_: &Quit, _: &mut Window, cx: &mut App| cx.quit())
            .on_mouse_move(cx.listener(Self::handle_root_mouse_move))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.viewer_bg))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .id("viewer-workspace")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .overflow_hidden()
                    .child(self.render_viewer(window, palette, cx))
                    .children(self.render_edit_panel(palette, cx))
                    .children(self.render_plugin_panel(palette, cx)),
            )
            .children(
                self.status_bar_visible()
                    .then(|| self.render_status_bar(window, palette, cx)),
            )
            .children(self.render_settings_panel(window, palette, cx))
            .children(dialog_layer)
    }
}

impl LumiaApp {
    fn render_empty_state(&self, palette: Palette, _cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("empty-state")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.text))
                    .child("Lumia"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(language, TextKey::EmptyState)),
            )
            .child(
                div()
                    .id("empty-state-open-button")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(rgb(palette.accent))
                    .text_color(rgb(palette.accent_text))
                    .hover(move |style| style.bg(rgb(palette.accent_hover)))
                    .active(move |style| style.bg(rgb(palette.accent_active)))
                    .on_mouse_down(MouseButton::Left, {
                        let self_handle = self.self_handle.clone();
                        move |_, window, cx| {
                            cx.stop_propagation();
                            let _ = self_handle.update(cx, |this, cx| {
                                this.open_file_dialog(cx, Some(window));
                            });
                        }
                    })
                    .child(tr(language, TextKey::EmptyStateOpenButton)),
            )
    }

    fn render_viewer(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewer = div()
            .id("viewer")
            .flex_1()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .bg(rgb(palette.viewer_bg))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                if this.is_viewer_blocked() {
                    return;
                }
                this.ui.pending_drop_paths = paths.paths().to_vec();
                this.load_first_supported_drop(window, cx);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if this.is_viewer_blocked() {
                    return;
                }
                let delta = match event.delta {
                    ScrollDelta::Pixels(delta) => f32::from(delta.y),
                    ScrollDelta::Lines(delta) => delta.y,
                };
                if delta > 0.0 {
                    if this.comparison_active() {
                        this.compare_scroll_zoom(false, event.position, window, cx);
                    } else {
                        this.scroll_zoom_at_cursor(false, event.position, window, cx);
                    }
                } else if delta < 0.0 {
                    if this.comparison_active() {
                        this.compare_scroll_zoom(true, event.position, window, cx);
                    } else {
                        this.scroll_zoom_at_cursor(true, event.position, window, cx);
                    }
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if this.is_viewer_blocked() {
                        return;
                    }
                    if this.ui.context_menu_position.take().is_some() {
                        cx.notify();
                        return;
                    }
                    if this.plugins.active.is_some()
                        && !event.modifiers.shift
                        && this.handle_annotation_mouse_down(event.position, window, cx)
                    {
                        return;
                    }
                    if this.viewer.has_document() {
                        this.ui.is_panning = true;
                        this.ui.last_mouse_position = Some(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.is_viewer_blocked() {
                        return;
                    }
                    if this.ui.annotation_drag.is_some() {
                        this.commit_annotation_rect(window, cx);
                        return;
                    }
                    this.ui.is_panning = false;
                    this.ui.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if this.is_viewer_blocked() {
                        return;
                    }
                    this.ui.context_menu_position =
                        Some(this.clamped_context_menu_position(event.position, window));
                    this.ui.is_panning = false;
                    this.ui.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.is_viewer_blocked() {
                        return;
                    }
                    if this.ui.annotation_drag.is_some() {
                        this.commit_annotation_rect(window, cx);
                        return;
                    }
                    this.ui.is_panning = false;
                    this.ui.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.is_viewer_blocked() {
                    return;
                }
                if this.ui.annotation_drag.is_some() && event.dragging() {
                    this.update_annotation_drag(event.position, window);
                    cx.notify();
                    return;
                }
                if this.ui.is_panning && event.dragging() {
                    if let Some(last_position) = this.ui.last_mouse_position {
                        let dx = f32::from(event.position.x - last_position.x);
                        let dy = f32::from(event.position.y - last_position.y);
                        if this.comparison_active() {
                            this.compare_pan(dx, dy, cx);
                        } else {
                            this.viewer.viewport_mut().pan_by(dx, dy);
                            this.refresh_large_image_tiles(window, cx);
                            cx.notify();
                        }
                    }
                    this.ui.last_mouse_position = Some(event.position);
                }
            }));

        if let Some(message) = &self.ui.error_message {
            viewer
                .child(status_message("error-state", message, palette.error_text))
                .children(self.render_image_info_overlay(window, cx))
                .children(self.render_context_menu(palette, cx))
        } else if self.comparison_active() {
            viewer.child(self.render_comparison(window, palette))
        } else if let Some(path) = self.image_path() {
            // Cloning this Arc is constant-time and avoids copying or hashing
            // the full pixel buffer during every render.
            let prepared = self
                .loads
                .display_image(self.viewer.rotation_quarter_turns());
            let display_size = self.scaled_image_size(window);
            // Prefer a filtered, display-sized bitmap; while a new scale is
            // processing, keep showing the previous filtered bitmap instead
            // of falling back to unfiltered GPU sampling mid-gesture.
            let mut shown = prepared.cloned();
            if let (Some(_), Some((display_width, display_height))) =
                (prepared.as_ref(), display_size)
            {
                let key = crate::display_resample::ResampleKey {
                    path: path.to_path_buf(),
                    filter: self.settings.resample_filter,
                    width: display_width.round().max(1.0) as u32,
                    height: display_height.round().max(1.0) as u32,
                };
                if let Some(resampled) = self.display_resample.ready(&key) {
                    shown = Some(resampled);
                } else if let Some(previous) = self.display_resample.latest_for(path) {
                    shown = Some(previous);
                }
            }

            let image = if let Some(large_image) = self.render_large_image_content(window) {
                large_image
            } else if let Some(prepared) = shown {
                let (image_width, image_height) = prepared.dimensions();
                let (display_w, display_h) = self
                    .scaled_image_size(window)
                    .unwrap_or((image_width as f32, image_height as f32));
                img(prepared.render_image())
                    .w(px(display_w))
                    .h(px(display_h))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else if self.loads.is_decoding() {
                status_message(
                    "loading-image",
                    tr(self.settings.language, TextKey::LoadingImage),
                    palette.muted_text,
                )
                .into_any_element()
            } else {
                img(path.to_path_buf())
                    .max_w_full()
                    .max_h_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            };

            let display_size = self.scaled_image_size(window);
            let crop_overlay = display_size.and_then(|(width, _)| {
                let scale = width / self.editing.source_width.max(1) as f32;
                self.render_crop_overlay(scale, palette, cx)
            });
            let mut image_frame = div()
                .flex_shrink_0()
                .relative()
                .left(px(self.viewer.viewport().pan_x))
                .top(px(self.viewer.viewport().pan_y))
                .child(image)
                .children(crop_overlay)
                .children(self.render_annotation_overlay(display_size, palette));
            if let Some((width, height)) = display_size {
                image_frame = image_frame.w(px(width)).h(px(height));
            }

            let surface_handle = self.self_handle.clone();
            let image_handle = self.self_handle.clone();
            viewer
                .child(
                    div()
                        .size_full()
                        .on_children_prepainted(move |bounds, _, cx| {
                            if let Some(bounds) = bounds.first() {
                                let _ = surface_handle.update(cx, |this, _| {
                                    this.ui.viewer_surface_bounds = Some(*bounds);
                                });
                            }
                        })
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_children_prepainted(move |bounds, _, cx| {
                                    if let Some(bounds) = bounds.first() {
                                        let _ = image_handle.update(cx, |this, _| {
                                            this.ui.viewer_image_bounds = Some(*bounds);
                                        });
                                    }
                                })
                                .child(image_frame),
                        ),
                )
                .children(self.large_image.detail_error().map(|message| {
                    status_message("large-image-detail-error", message, palette.error_text)
                }))
                .children(self.render_image_overview(window, palette, cx))
                .children(self.render_image_info_overlay(window, cx))
                .children(self.render_context_menu(palette, cx))
        } else {
            viewer
                .child(self.render_empty_state(palette, cx))
                .children(self.render_image_info_overlay(window, cx))
                .children(self.render_context_menu(palette, cx))
        }
    }
}
