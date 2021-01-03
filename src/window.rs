use crate::Settings;
use crate::{
    renderer::{RenderSettings, Renderer},
    Painter,
};
use baseview::{Event, Window, WindowHandler, WindowScalePolicy};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use std::time::Instant;

use egui::{pos2, vec2, Pos2, Rect, Srgba};

struct OpenSettings {
    pub scale_policy: WindowScalePolicy,
    pub logical_width: f64,
    pub logical_height: f64,
}

impl OpenSettings {
    fn new(settings: &Settings) -> Self {
        // WindowScalePolicy does not implement copy/clone.
        let scale_policy = match &settings.window.scale {
            WindowScalePolicy::SystemScaleFactor => WindowScalePolicy::SystemScaleFactor,
            WindowScalePolicy::ScaleFactor(scale) => WindowScalePolicy::ScaleFactor(*scale),
        };

        Self {
            scale_policy,
            logical_width: settings.window.size.width as f64,
            logical_height: settings.window.size.height as f64,
        }
    }
}

/// Handles an egui-baseview application
pub struct EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::CtxRef, &mut Painter, &mut Srgba, &mut State),
    U: 'static + Send,
{
    user_state: State,
    user_update: U,

    egui_ctx: egui::CtxRef,
    raw_input: egui::RawInput,

    renderer: Renderer,
    scale_factor: f64,
    scale_policy: WindowScalePolicy,
    pixels_per_point: f32,
    bg_color: Srgba,
    modifiers: egui::Modifiers,
    start_time: Instant,
}

impl<State, U> EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::CtxRef, &mut Painter, &mut Srgba, &mut State),
    U: 'static + Send,
{
    fn new(
        window: &mut baseview::Window<'_>,
        open_settings: OpenSettings,
        mut render_settings: Option<RenderSettings>,
        update: U,
        state: State,
    ) -> EguiWindow<State, U> {
        // Assume scale for now until there is an event with a new one.
        let scale = match open_settings.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };
        let pixels_per_point = 1.0 / scale as f32;

        let egui_ctx = egui::CtxRef::default();

        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                vec2(
                    open_settings.logical_width as f32,
                    open_settings.logical_height as f32,
                ) * scale as f32,
            )),
            pixels_per_point: Some(pixels_per_point),
            ..Default::default()
        };

        let renderer = Renderer::new(
            window,
            render_settings.take().unwrap(),
            (
                open_settings.logical_width.round() as u32,
                open_settings.logical_height.round() as u32,
            ),
        );

        Self {
            user_state: state,
            user_update: update,

            egui_ctx,
            raw_input,

            renderer,
            scale_factor: scale,
            scale_policy: open_settings.scale_policy,
            pixels_per_point,
            bg_color: Srgba::black_alpha(255),
            modifiers: egui::Modifiers {
                alt: false,
                ctrl: false,
                shift: false,
                mac_cmd: false,
                command: false,
            },
            start_time: Instant::now(),
        }
    }

    /// Open a new child window.
    ///
    /// * `parent` - The parent window.
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_parented<P>(parent: &P, settings: Settings, state: State, update: U)
    where
        P: HasRawWindowHandle,
    {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_parented(
            parent,
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, update, state)
            },
        )
    }

    /// Open a new window as if it had a parent window.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_as_if_parented(settings: Settings, state: State, update: U) -> RawWindowHandle {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_as_if_parented(
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, update, state)
            },
        )
    }

    /// Open a new window that blocks the current thread until the window is destroyed.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_blocking(settings: Settings, state: State, update: U) {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_blocking(
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, update, state)
            },
        )
    }
}

impl<State, U> WindowHandler for EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::CtxRef, &mut Painter, &mut Srgba, &mut State),
    U: 'static + Send,
{
    fn on_frame(&mut self) {
        self.raw_input.time = Some(self.start_time.elapsed().as_nanos() as f64 * 1e-9);

        self.egui_ctx.begin_frame(self.raw_input.take());

        (self.user_update)(
            &self.egui_ctx,
            &mut self.renderer.painter(),
            &mut self.bg_color,
            &mut self.user_state,
        );

        // We aren't handling the output at the moment.
        let (_output, paint_cmds) = self.egui_ctx.end_frame();
        let paint_jobs = self.egui_ctx.tesselate(paint_cmds);

        self.renderer.render(
            self.bg_color,
            paint_jobs,
            &self.egui_ctx.texture(),
            self.pixels_per_point,
        );
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) {
        match &event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved { position } => {
                    self.raw_input.mouse_pos = Some(pos2(position.x as f32, position.y as f32));
                }
                baseview::MouseEvent::ButtonPressed(button) => match button {
                    // TODO: More mouse buttons?
                    baseview::MouseButton::Left => self.raw_input.mouse_down = true,
                    _ => {}
                },
                baseview::MouseEvent::ButtonReleased(button) => match button {
                    // TODO: More mouse buttons?
                    baseview::MouseButton::Left => self.raw_input.mouse_down = false,
                    _ => {}
                },
                baseview::MouseEvent::WheelScrolled(scroll_delta) => {
                    let (lines_x, lines_y) = match scroll_delta {
                        baseview::ScrollDelta::Lines { x, y } => (*x, *y),
                        baseview::ScrollDelta::Pixels { x, y } => (
                            if *x < 0.0 {
                                -1.0
                            } else if *x > 1.0 {
                                1.0
                            } else {
                                0.0
                            },
                            if *y < 0.0 {
                                -1.0
                            } else if *y > 1.0 {
                                1.0
                            } else {
                                0.0
                            },
                        ),
                    };

                    self.raw_input.scroll_delta = vec2(lines_x as f32, lines_y as f32);
                }
                _ => {}
            },
            baseview::Event::Keyboard(event) => {
                use keyboard_types::Code;

                let pressed = event.state == keyboard_types::KeyState::Down;

                match event.code {
                    Code::ShiftLeft | Code::ShiftRight => self.modifiers.shift = pressed,
                    Code::ControlLeft | Code::ControlRight => {
                        self.modifiers.ctrl = pressed;

                        #[cfg(not(target_os = "macos"))]
                        {
                            self.modifiers.command = pressed;
                        }
                    }
                    Code::AltLeft | Code::AltRight => self.modifiers.alt = pressed,
                    Code::MetaLeft | Code::MetaRight => {
                        #[cfg(target_os = "macos")]
                        {
                            self.modifiers.mac_cmd = pressed;
                            self.modifiers.command = pressed;
                        }
                        () // prevent `rustfmt` from breaking this
                    }
                    _ => (),
                }

                if let Some(key) = translate_virtual_key_code(event.code) {
                    self.raw_input.events.push(egui::Event::Key {
                        key,
                        pressed,
                        modifiers: self.modifiers,
                    });
                }

                if pressed {
                    if let keyboard_types::Key::Character(written) = &event.key {
                        self.raw_input
                            .events
                            .push(egui::Event::Text(written.clone()));
                    }
                }
            }
            baseview::Event::Window(event) => match event {
                baseview::WindowEvent::Resized(window_info) => {
                    self.scale_factor = match self.scale_policy {
                        WindowScalePolicy::ScaleFactor(scale) => scale,
                        WindowScalePolicy::SystemScaleFactor => window_info.scale(),
                    };

                    self.pixels_per_point = 1.0 / self.scale_factor as f32;

                    let logical_size = (
                        (window_info.physical_size().width as f64 / self.scale_factor) as f32,
                        (window_info.physical_size().height as f64 / self.scale_factor) as f32,
                    );

                    self.raw_input.pixels_per_point = Some(self.pixels_per_point);

                    self.raw_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::new(0f32, 0f32),
                        vec2(logical_size.0, logical_size.1),
                    ));
                }
                baseview::WindowEvent::WillClose => {}
                _ => {}
            },
        }
    }
}

pub fn translate_virtual_key_code(key: keyboard_types::Code) -> Option<egui::Key> {
    use egui::Key;
    use keyboard_types::Code;

    Some(match key {
        Code::Escape => Key::Escape,
        Code::Insert => Key::Insert,
        Code::Home => Key::Home,
        Code::Delete => Key::Delete,
        Code::End => Key::End,
        Code::PageDown => Key::PageDown,
        Code::PageUp => Key::PageUp,
        Code::ArrowLeft => Key::ArrowLeft,
        Code::ArrowUp => Key::ArrowUp,
        Code::ArrowRight => Key::ArrowRight,
        Code::ArrowDown => Key::ArrowDown,
        Code::Backspace => Key::Backspace,
        Code::Enter => Key::Enter,
        // Space => Key::Space,
        Code::Tab => Key::Tab,
        _ => {
            return None;
        }
    })
}
