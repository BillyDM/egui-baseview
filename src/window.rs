use baseview::{Event, EventStatus, Window, WindowHandle, WindowHandler, WindowScalePolicy};
use copypasta::ClipboardProvider;
use egui::{pos2, vec2, Pos2, Rect, Rgba};
use raw_window_handle::HasRawWindowHandle;
use std::time::Instant;

use crate::renderer::{RenderSettings, Renderer};
use crate::Settings;

pub struct Queue<'a> {
    bg_color: &'a mut Rgba,
    close_requested: &'a mut bool,
}

impl<'a> Queue<'a> {
    pub(crate) fn new(bg_color: &'a mut Rgba, close_requested: &'a mut bool) -> Self {
        Self {
            bg_color,
            //renderer,
            //repaint_requested,
            close_requested,
        }
    }

    /// Set the background color.
    pub fn bg_color(&mut self, bg_color: Rgba) {
        *self.bg_color = bg_color;
    }

    /// Close the window.
    pub fn close_window(&mut self) {
        *self.close_requested = true;
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
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    user_state: Option<State>,
    user_update: U,

    egui_ctx: egui::Context,
    egui_input: egui::RawInput,
    clipboard_ctx: Option<copypasta::ClipboardContext>,

    renderer: Renderer,
    scale_factor: f32,
    scale_policy: WindowScalePolicy,
    bg_color: Rgba,
    physical_width: u32,
    physical_height: u32,
    start_time: Instant,
    repaint_after: Option<Instant>,
    mouse_pos: Option<Pos2>,
    close_requested: bool,
}

impl<State, U> EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::Context, &mut Queue, &mut State),
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
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        // Assume scale for now until there is an event with a new one.
        let scale = match open_settings.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        } as f32;

        let egui_ctx = egui::Context::default();

        let egui_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                vec2(
                    open_settings.logical_width as f32,
                    open_settings.logical_height as f32,
                ),
            )),
            pixels_per_point: Some(scale),
            modifiers: egui::Modifiers {
                alt: false,
                ctrl: false,
                shift: false,
                mac_cmd: false,
                command: false,
            },
            ..Default::default()
        };

        let physical_width = (open_settings.logical_width * scale as f64).round() as u32;
        let physical_height = (open_settings.logical_height * scale as f64).round() as u32;

        let renderer = Renderer::new(window, render_settings.take().unwrap());

        let mut bg_color = Rgba::BLACK;
        let mut close_requested = false;
        let mut queue = Queue::new(
            &mut bg_color,
            //&mut renderer,
            //&mut repaint_requested,
            &mut close_requested,
        );
        (build)(&egui_ctx, &mut queue, &mut state);

        let clipboard_ctx = match copypasta::ClipboardContext::new() {
            Ok(clipboard_ctx) => Some(clipboard_ctx),
            Err(e) => {
                eprintln!("Failed to initialize clipboard: {}", e);
                None
            }
        };

        Self {
            user_state: Some(state),
            user_update: update,

            egui_ctx,
            egui_input,
            clipboard_ctx,

            renderer,
            scale_factor: scale,
            scale_policy: open_settings.scale_policy,
            bg_color,
            physical_width,
            physical_height,
            start_time: Instant::now(),
            repaint_after: Some(Instant::now()),
            mouse_pos: None,
            close_requested,
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
    pub fn open_parented<P, B>(
        parent: &P,
        settings: Settings,
        state: State,
        build: B,
        update: U,
    ) -> WindowHandle
    where
        P: HasRawWindowHandle,
        B: FnMut(&egui::Context, &mut Queue, &mut State),
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
    ) -> WindowHandle
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
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
        B: FnMut(&egui::Context, &mut Queue, &mut State),
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
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn on_frame(&mut self, window: &mut Window) {
        if let Some(state) = &mut self.user_state {
            self.egui_input.time = Some(self.start_time.elapsed().as_nanos() as f64 * 1e-9);
            self.egui_ctx.begin_frame(self.egui_input.take());

            //let mut repaint_requested = false;
            let mut queue = Queue::new(
                &mut self.bg_color,
                //&mut self.renderer,
                //&mut repaint_requested,
                &mut self.close_requested,
            );

            (self.user_update)(&self.egui_ctx, &mut queue, state);

            let egui::FullOutput {
                platform_output,
                repaint_after,
                mut textures_delta,
                mut shapes,
            } = self.egui_ctx.end_frame();

            let now = Instant::now();
            let do_repaint_now = if let Some(t) = self.repaint_after {
                now >= t || repaint_after.is_zero()
            } else {
                repaint_after.is_zero()
            };

            if do_repaint_now {
                self.renderer.render(
                    self.bg_color,
                    self.physical_width,
                    self.physical_height,
                    self.scale_factor,
                    &mut self.egui_ctx,
                    &mut shapes,
                    &mut textures_delta,
                );

                self.repaint_after = None;
            } else if let Some(repaint_after) = now.checked_add(repaint_after) {
                // Schedule to repaint after the requested time has elapsed.
                self.repaint_after = Some(repaint_after);
            }

            if !platform_output.copied_text.is_empty() {
                if let Some(clipboard_ctx) = &mut self.clipboard_ctx {
                    if let Err(err) = clipboard_ctx.set_contents(platform_output.copied_text) {
                        eprintln!("Copy/Cut error: {}", err);
                    }
                }
            }

            // TODO: Handle setting the cursor icon.

            if self.close_requested {
                window.close();
            }
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved { position } => {
                    let pos = pos2(position.x as f32, position.y as f32);
                    self.mouse_pos = Some(pos);
                    self.egui_input.events.push(egui::Event::PointerMoved(pos));
                }
                baseview::MouseEvent::ButtonPressed(button) => {
                    if let Some(pos) = self.mouse_pos {
                        if let Some(button) = translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: true,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::ButtonReleased(button) => {
                    if let Some(pos) = self.mouse_pos {
                        if let Some(button) = translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: false,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::WheelScrolled(scroll_delta) => {
                    let mut delta = match scroll_delta {
                        baseview::ScrollDelta::Lines { x, y } => {
                            let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                            egui::vec2(*x, *y) * points_per_scroll_line
                        }
                        baseview::ScrollDelta::Pixels { x, y } => {
                            if let Some(pixels_per_point) = self.egui_input.pixels_per_point {
                                egui::vec2(*x, *y) / pixels_per_point
                            } else {
                                egui::vec2(*x, *y)
                            }
                        }
                    };
                    if cfg!(target_os = "macos") {
                        // This is still buggy in winit despite
                        // https://github.com/rust-windowing/winit/issues/1695 being closed
                        delta.x *= -1.0;
                    }

                    if self.egui_input.modifiers.ctrl || self.egui_input.modifiers.command {
                        // Treat as zoom instead:
                        let factor = (delta.y / 200.0).exp();
                        self.egui_input.events.push(egui::Event::Zoom(factor));
                    } else if self.egui_input.modifiers.shift {
                        // Treat as horizontal scrolling.
                        // Note: one Mac we already get horizontal scroll events when shift is down.
                        self.egui_input
                            .events
                            .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                    } else {
                        self.egui_input.events.push(egui::Event::Scroll(delta));
                    }
                }
                baseview::MouseEvent::CursorLeft => {
                    self.mouse_pos = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                _ => {}
            },
            baseview::Event::Keyboard(event) => {
                use keyboard_types::Code;

                let pressed = event.state == keyboard_types::KeyState::Down;

                match event.code {
                    Code::ShiftLeft | Code::ShiftRight => self.egui_input.modifiers.shift = pressed,
                    Code::ControlLeft | Code::ControlRight => {
                        self.egui_input.modifiers.ctrl = pressed;

                        #[cfg(not(target_os = "macos"))]
                        {
                            self.egui_input.modifiers.command = pressed;
                        }
                    }
                    Code::AltLeft | Code::AltRight => self.egui_input.modifiers.alt = pressed,
                    Code::MetaLeft | Code::MetaRight => {
                        #[cfg(target_os = "macos")]
                        {
                            self.egui_input.modifiers.mac_cmd = pressed;
                            self.egui_input.modifiers.command = pressed;
                        }
                        () // prevent `rustfmt` from breaking this
                    }
                    _ => (),
                }

                if let Some(key) = translate_virtual_key_code(event.code) {
                    self.egui_input.events.push(egui::Event::Key {
                        key,
                        pressed,
                        modifiers: self.egui_input.modifiers,
                    });
                }

                if pressed {
                    // VirtualKeyCode::Paste etc in winit are broken/untrustworthy,
                    // so we detect these things manually:
                    if is_cut_command(self.egui_input.modifiers, event.code) {
                        self.egui_input.events.push(egui::Event::Cut);
                    } else if is_copy_command(self.egui_input.modifiers, event.code) {
                        self.egui_input.events.push(egui::Event::Copy);
                    } else if is_paste_command(self.egui_input.modifiers, event.code) {
                        if let Some(clipboard_ctx) = &mut self.clipboard_ctx {
                            match clipboard_ctx.get_contents() {
                                Ok(contents) => {
                                    self.egui_input.events.push(egui::Event::Text(contents))
                                }
                                Err(err) => {
                                    eprintln!("Paste error: {}", err);
                                }
                            }
                        }
                    } else if let keyboard_types::Key::Character(written) = &event.key {
                        if !self.egui_input.modifiers.ctrl && !self.egui_input.modifiers.command {
                            self.egui_input
                                .events
                                .push(egui::Event::Text(written.clone()));
                            self.egui_ctx.wants_keyboard_input();
                        }
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

                    self.physical_width = window_info.physical_size().width;
                    self.physical_height = window_info.physical_size().height;

                    self.egui_input.pixels_per_point = Some(self.scale_factor);

                    self.egui_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::new(0f32, 0f32),
                        vec2(logical_size.0, logical_size.1),
                    ));

                    // Schedule to repaint on the next frame.
                    self.repaint_after = Some(Instant::now());
                }
                baseview::WindowEvent::WillClose => {}
                _ => {}
            },
        }

        EventStatus::Captured
    }
}

pub fn translate_mouse_button(button: baseview::MouseButton) -> Option<egui::PointerButton> {
    match button {
        baseview::MouseButton::Left => Some(egui::PointerButton::Primary),
        baseview::MouseButton::Right => Some(egui::PointerButton::Secondary),
        baseview::MouseButton::Middle => Some(egui::PointerButton::Middle),
        _ => None,
    }
}

pub fn translate_virtual_key_code(key: keyboard_types::Code) -> Option<egui::Key> {
    use egui::Key;
    use keyboard_types::Code;

    Some(match key {
        Code::ArrowDown => Key::ArrowDown,
        Code::ArrowLeft => Key::ArrowLeft,
        Code::ArrowRight => Key::ArrowRight,
        Code::ArrowUp => Key::ArrowUp,

        Code::Escape => Key::Escape,
        Code::Tab => Key::Tab,
        Code::Backspace => Key::Backspace,
        Code::Enter => Key::Enter,
        Code::Space => Key::Space,

        Code::Insert => Key::Insert,
        Code::Delete => Key::Delete,
        Code::Home => Key::Home,
        Code::End => Key::End,
        Code::PageUp => Key::PageUp,
        Code::PageDown => Key::PageDown,

        Code::Digit0 | Code::Numpad0 => Key::Num0,
        Code::Digit1 | Code::Numpad1 => Key::Num1,
        Code::Digit2 | Code::Numpad2 => Key::Num2,
        Code::Digit3 | Code::Numpad3 => Key::Num3,
        Code::Digit4 | Code::Numpad4 => Key::Num4,
        Code::Digit5 | Code::Numpad5 => Key::Num5,
        Code::Digit6 | Code::Numpad6 => Key::Num6,
        Code::Digit7 | Code::Numpad7 => Key::Num7,
        Code::Digit8 | Code::Numpad8 => Key::Num8,
        Code::Digit9 | Code::Numpad9 => Key::Num9,

        Code::KeyA => Key::A,
        Code::KeyB => Key::B,
        Code::KeyC => Key::C,
        Code::KeyD => Key::D,
        Code::KeyE => Key::E,
        Code::KeyF => Key::F,
        Code::KeyG => Key::G,
        Code::KeyH => Key::H,
        Code::KeyI => Key::I,
        Code::KeyJ => Key::J,
        Code::KeyK => Key::K,
        Code::KeyL => Key::L,
        Code::KeyM => Key::M,
        Code::KeyN => Key::N,
        Code::KeyO => Key::O,
        Code::KeyP => Key::P,
        Code::KeyQ => Key::Q,
        Code::KeyR => Key::R,
        Code::KeyS => Key::S,
        Code::KeyT => Key::T,
        Code::KeyU => Key::U,
        Code::KeyV => Key::V,
        Code::KeyW => Key::W,
        Code::KeyX => Key::X,
        Code::KeyY => Key::Y,
        Code::KeyZ => Key::Z,
        _ => {
            return None;
        }
    })
}

fn is_cut_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyX)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == keyboard_types::Code::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyC)
        || (cfg!(target_os = "windows")
            && modifiers.ctrl
            && keycode == keyboard_types::Code::Insert)
}

fn is_paste_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyV)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == keyboard_types::Code::Insert)
}
