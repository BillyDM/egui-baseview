use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use egui::CtxRef;
use egui_baseview::{EguiWindow, Queue};

fn main() {
    let settings = WindowOpenOptions {
        title: String::from("egui-baseview hello world"),
        size: Size::new(300.0, 110.0),
        scale: WindowScalePolicy::SystemScaleFactor,
        gl_config: Some(baseview::gl::GlConfig::default()),
    };

    let state = ();

    EguiWindow::open_blocking(
        settings,
        state,
        |_egui_ctx: &CtxRef, _queue: &mut Queue, _state: &mut ()| {},
        |egui_ctx: &CtxRef, _queue: &mut Queue, _state: &mut ()| {
            egui::Window::new("egui-baseview hello world").show(&egui_ctx, |ui| {
                ui.label("Hello World!");
            });
        },
    );
}
