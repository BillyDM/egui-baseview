use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use egui::Context;
use egui_baseview::{EguiWindow, Queue, RenderSettings, Settings};

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
        |_egui_ctx: &Context, _queue: &mut Queue, _state: &mut ()| {},
        |egui_ctx: &Context, _queue: &mut Queue, _state: &mut ()| {
            egui::Window::new("egui-baseview hello world").show(&egui_ctx, |ui| {
                ui.label("Hello World!");
            });
        },
    );
}
