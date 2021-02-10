use crate::renderer::{RenderSettings, Renderer};
use crate::Settings;
use baseview::{Event, EventStatus, Window, WindowHandler, WindowScalePolicy};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use std::time::Instant;

use egui::{pos2, vec2, Color32, Pos2, Rect};

pub struct Queue<'a> {
    bg_color: &'a mut Color32,
    renderer: &'a mut Renderer,
    repaint_requested: &'a mut bool,
}

impl<'a> Queue<'a> {
    pub(crate) fn new(
        bg_color: &'a mut Color32,
        renderer: &'a mut Renderer,
        repaint_requested: &'a mut bool,
    ) -> Self {
        Self {
            bg_color,
            renderer,
            repaint_requested,
        }
    }

    /// Set the background color.
    pub fn bg_color(&mut self, bg_color: Color32) {
        *self.bg_color = bg_color;
    }

    /// Create a new custom texture.
    pub fn new_user_texture(
        &mut self,
        size: (usize, usize),
        srgba_pixels: &[Color32],
        filtering: bool,
    ) -> egui::TextureId {
        self.renderer
            .new_user_texture(size, srgba_pixels, filtering)
    }

    /// Update a custom texture.
    pub fn update_user_texture_data(&mut self, texture_id: egui::TextureId, pixels: &[Color32]) {
        self.renderer.update_user_texture_data(texture_id, pixels)
    }

    /// Request to repaint the UI on the next frame.
    pub fn request_repaint(&mut self) {
        *self.repaint_requested = true;
    }
}

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
    U: FnMut(&egui::CtxRef, &mut Queue, &mut State),
    U: 'static + Send,
{
    user_state: State,
    user_update: U,

    egui_ctx: egui::CtxRef,
    raw_input: egui::RawInput,

    renderer: Renderer,
    scale_factor: f32,
    scale_policy: WindowScalePolicy,
    bg_color: Color32,
    modifiers: egui::Modifiers,
    start_time: Instant,
    redraw: bool,
}

impl<State, U> EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::CtxRef, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn new<B>(
        window: &mut baseview::Window<'_>,
        open_settings: OpenSettings,
        mut render_settings: Option<RenderSettings>,
        mut build: B,
        update: U,
        mut state: State,
    ) -> EguiWindow<State, U>
    where
        B: FnMut(&egui::CtxRef, &mut Queue, &mut State),
        B: 'static + Send,
    {
        // Assume scale for now until there is an event with a new one.
        let scale = match open_settings.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        } as f32;

        let egui_ctx = egui::CtxRef::default();

        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                vec2(
                    open_settings.logical_width as f32,
                    open_settings.logical_height as f32,
                ),
            )),
            pixels_per_point: Some(scale),
            ..Default::default()
        };

        let mut renderer = Renderer::new(
            window,
            render_settings.take().unwrap(),
            (
                (open_settings.logical_width * scale as f64).round() as u32,
                (open_settings.logical_height * scale as f64).round() as u32,
            ),
        );

        let mut bg_color = Color32::BLACK;

        let mut repaint_requested = false;
        let mut queue = Queue::new(&mut bg_color, &mut renderer, &mut repaint_requested);
        (build)(&egui_ctx, &mut queue, &mut state);

        Self {
            user_state: state,
            user_update: update,

            egui_ctx,
            raw_input,

            renderer,
            scale_factor: scale,
            scale_policy: open_settings.scale_policy,
            bg_color,
            modifiers: egui::Modifiers {
                alt: false,
                ctrl: false,
                shift: false,
                mac_cmd: false,
                command: false,
            },
            start_time: Instant::now(),
            redraw: true,
        }
    }

    /// Open a new child window.
    ///
    /// * `parent` - The parent window.
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_parented<P, B>(parent: &P, settings: Settings, state: State, build: B, update: U)
    where
        P: HasRawWindowHandle,
        B: FnMut(&egui::CtxRef, &mut Queue, &mut State),
        B: 'static + Send,
    {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_parented(
            parent,
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, build, update, state)
            },
        )
    }

    /// Open a new window as if it had a parent window.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_as_if_parented<B>(
        settings: Settings,
        state: State,
        build: B,
        update: U,
    ) -> RawWindowHandle
    where
        B: FnMut(&egui::CtxRef, &mut Queue, &mut State),
        B: 'static + Send,
    {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_as_if_parented(
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, build, update, state)
            },
        )
    }

    /// Open a new window that blocks the current thread until the window is destroyed.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_blocking<B>(settings: Settings, state: State, build: B, update: U)
    where
        B: FnMut(&egui::CtxRef, &mut Queue, &mut State),
        B: 'static + Send,
    {
        let open_settings = OpenSettings::new(&settings);
        let render_settings = Some(settings.render_settings);

        Window::open_blocking(
            settings.window,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, render_settings, build, update, state)
            },
        )
    }
}

impl<State, U> WindowHandler for EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::CtxRef, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn on_frame(&mut self, _window: &mut Window) {
        self.raw_input.time = Some(self.start_time.elapsed().as_nanos() as f64 * 1e-9);
        self.egui_ctx.begin_frame(self.raw_input.take());

        let mut repaint_requested = false;
        let mut queue = Queue::new(
            &mut self.bg_color,
            &mut self.renderer,
            &mut repaint_requested,
        );

        (self.user_update)(&self.egui_ctx, &mut queue, &mut self.user_state);

        let (output, paint_cmds) = self.egui_ctx.end_frame();

        if output.needs_repaint || self.redraw || repaint_requested {
            let paint_jobs = self.egui_ctx.tessellate(paint_cmds);

            self.renderer.render(
                self.bg_color,
                paint_jobs,
                &self.egui_ctx.texture(),
                self.scale_factor,
            );

            self.redraw = false;
        }

        // TODO: Handle the rest of the outputs.
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved { position } => {
                    self.raw_input.mouse_pos = Some(pos2(position.x as f32, position.y as f32));
                }
                baseview::MouseEvent::ButtonPressed(button) => match button {
                    // TODO: More mouse buttons?
                    baseview::MouseButton::Left => {
                        self.raw_input.mouse_down = true;
                    }
                    _ => {}
                },
                baseview::MouseEvent::ButtonReleased(button) => match button {
                    // TODO: More mouse buttons?
                    baseview::MouseButton::Left => {
                        self.raw_input.mouse_down = false;
                    }
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
                    } as f32;

                    let logical_size = (
                        (window_info.physical_size().width as f32 / self.scale_factor),
                        (window_info.physical_size().height as f32 / self.scale_factor),
                    );

                    self.raw_input.pixels_per_point = Some(self.scale_factor);

                    self.raw_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::new(0f32, 0f32),
                        vec2(logical_size.0, logical_size.1),
                    ));

                    self.renderer.update_window_size(
                        window_info.physical_size().width,
                        window_info.physical_size().height,
                    );

                    self.redraw = true;
                }
                baseview::WindowEvent::WillClose => {}
                _ => {}
            },
        }

        EventStatus::Captured
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
