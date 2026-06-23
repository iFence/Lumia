use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, WindowBounds, WindowOptions,
};
use lumia_core::ViewportState;

struct LumiaApp {
    viewport: ViewportState,
}

impl LumiaApp {
    fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            viewport: ViewportState::default(),
        }
    }
}

impl Render for LumiaApp {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("lumia-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x111111))
            .text_color(rgb(0xf2f2f2))
            .child(
                div()
                    .id("toolbar")
                    .h(px(44.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(0x2a2a2a))
                    .child("Lumia"),
            )
            .child(
                div()
                    .id("viewer")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0x8a8a8a))
                    .child(format!(
                        "Drop an image here. Zoom: {:.0}%",
                        self.viewport.zoom * 100.0
                    )),
            )
    }
}

fn main() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Lumia".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| LumiaApp::new(window, cx)),
        )
        .expect("failed to open Lumia window");
        cx.activate(true);
    });

    Ok(())
}
