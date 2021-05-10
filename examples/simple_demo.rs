use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use egui::CtxRef;
use egui_baseview::{EguiWindow, Queue, RenderSettings, Settings};

fn main() {
    let settings = Settings {
        window: WindowOpenOptions {
            title: String::from("egui-baseview simple demo"),
            size: Size::new(400.0, 200.0),
            scale: WindowScalePolicy::SystemScaleFactor,
        },
        render_settings: RenderSettings::default(),
    };

    let state = State::new();

    EguiWindow::open_blocking(
        settings,
        state,
        // Called once before the first frame. Allows you to do setup code and to
        // call `ctx.set_fonts()`. Optional.
        |_egui_ctx: &CtxRef, _queue: &mut Queue, _state: &mut State| {},
        // Called before each frame. Here you should update the state of your
        // application and build the UI.
        |egui_ctx: &CtxRef, _queue: &mut Queue, state: &mut State| {
            egui::Window::new("egui-baseview simple demo").show(&egui_ctx, |ui| {
                ui.heading("My Egui Application");
                ui.horizontal(|ui| {
                    ui.label("Your name: ");
                    ui.text_edit_singleline(&mut state.name);
                });
                ui.add(egui::Slider::new(&mut state.age, 0..=120).text("age"));
                if ui.button("Click each year").clicked() {
                    state.age += 1;
                }
                ui.label(format!("Hello '{}', age {}", state.name, state.age));
            });
        },
    );
}

struct State {
    pub name: String,
    pub age: u32,
}

impl State {
    pub fn new() -> State {
        State {
            name: String::from(""),
            age: 30,
        }
    }
}
