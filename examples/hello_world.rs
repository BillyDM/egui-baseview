use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use egui::{CtxRef, Srgba};
use egui_baseview::{EguiWindow, Painter, RenderSettings, Settings};

fn main() {
    let settings = Settings {
        window: WindowOpenOptions {
            title: String::from("egui-baseview hello world"),
            size: Size::new(300.0, 110.0),
            scale: WindowScalePolicy::SystemScaleFactor,
        },
        render_settings: RenderSettings::default(),
    };

    let state = ();

    EguiWindow::open_blocking(
        settings,
        state,
        |egui_ctx: &CtxRef, _painter: &mut Painter, _bg_color: &mut Srgba, _state: &mut ()| {
            egui::Window::new("egui-baseview hello world").show(&egui_ctx, |ui| {
                ui.label("Hello World!");
            });
        },
    );
}
