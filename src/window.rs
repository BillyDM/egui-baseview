use baseview::{
    Event, EventStatus, Window, WindowHandle, WindowHandler, WindowOpenOptions, WindowScalePolicy,
};
use copypasta::ClipboardProvider;
use egui::{pos2, vec2, Pos2, Rect, Rgba, ViewportCommand};
use keyboard_types::Modifiers;
use raw_window_handle::HasRawWindowHandle;
use std::time::Instant;

use crate::renderer::{get_renderer, Renderer};

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
    scale_policy: WindowScalePolicy,
    logical_width: f64,
    logical_height: f64,
    title: String,
}

impl OpenSettings {
    fn new(settings: &WindowOpenOptions) -> Self {
        // WindowScalePolicy does not implement copy/clone.
        let scale_policy = match &settings.scale {
            WindowScalePolicy::SystemScaleFactor => WindowScalePolicy::SystemScaleFactor,
            WindowScalePolicy::ScaleFactor(scale) => WindowScalePolicy::ScaleFactor(*scale),
        };

        Self {
            scale_policy,
            logical_width: settings.size.width,
            logical_height: settings.size.height,
            title: settings.title.clone(),
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
    viewport_id: egui::ViewportId,
    start_time: Instant,
    egui_input: egui::RawInput,
    pointer_pos_in_points: Option<egui::Pos2>,
    current_cursor_icon: baseview::MouseCursor,

    renderer: Renderer,

    clipboard_ctx: Option<copypasta::ClipboardContext>,

    physical_width: u32,
    physical_height: u32,
    scale_policy: WindowScalePolicy,
    pixels_per_point: f32,
    points_per_pixel: f32,
    points_per_scroll_line: f32,
    bg_color: Rgba,
    close_requested: bool,
    repaint_after: Option<Instant>,
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
        mut build: B,
        update: U,
        mut state: State,
    ) -> EguiWindow<State, U>
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        let renderer = get_renderer(window);
        let egui_ctx = egui::Context::default();

        // Assume scale for now until there is an event with a new one.
        let pixels_per_point = match open_settings.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        } as f32;
        let points_per_pixel = pixels_per_point.recip();
        let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461

        let screen_rect = Rect::from_min_size(
            Pos2::new(0f32, 0f32),
            vec2(
                open_settings.logical_width as f32,
                open_settings.logical_height as f32,
            ),
        );

        let viewport_info = egui::ViewportInfo {
            parent: None,
            title: Some(open_settings.title),
            native_pixels_per_point: Some(pixels_per_point),
            focused: Some(true),
            inner_rect: Some(screen_rect),
            ..Default::default()
        };
        let viewport_id = egui::ViewportId::default();

        let mut egui_input = egui::RawInput {
            max_texture_side: Some(renderer.max_texture_side()),
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        let _ = egui_input.viewports.insert(viewport_id, viewport_info);

        let physical_width = (open_settings.logical_width * pixels_per_point as f64).round() as u32;
        let physical_height =
            (open_settings.logical_height * pixels_per_point as f64).round() as u32;

        let mut bg_color = Rgba::BLACK;
        let mut close_requested = false;
        let mut queue = Queue::new(&mut bg_color, &mut close_requested);
        (build)(&egui_ctx, &mut queue, &mut state);

        let clipboard_ctx = match copypasta::ClipboardContext::new() {
            Ok(clipboard_ctx) => Some(clipboard_ctx),
            Err(e) => {
                log::error!("Failed to initialize clipboard: {}", e);
                None
            }
        };

        let start_time = Instant::now();

        Self {
            user_state: Some(state),
            user_update: update,

            egui_ctx,
            viewport_id,
            start_time,
            egui_input,
            pointer_pos_in_points: None,
            current_cursor_icon: baseview::MouseCursor::Default,

            renderer,

            clipboard_ctx,

            physical_width,
            physical_height,
            pixels_per_point,
            points_per_pixel,
            scale_policy: open_settings.scale_policy,
            points_per_scroll_line,
            bg_color,
            close_requested,
            repaint_after: Some(start_time),
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
        #[allow(unused_mut)] mut settings: WindowOpenOptions,
        state: State,
        build: B,
        update: U,
    ) -> WindowHandle
    where
        P: HasRawWindowHandle,
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        #[cfg(feature = "opengl")]
        if settings.gl_config.is_none() {
            settings.gl_config = Some(Default::default());
        }

        let open_settings = OpenSettings::new(&settings);

        Window::open_parented(
            parent,
            settings,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, build, update, state)
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
    pub fn open_blocking<B>(#[allow(unused_mut)] mut settings: WindowOpenOptions, state: State, build: B, update: U)
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        #[cfg(feature = "opengl")]
        if settings.gl_config.is_none() {
            settings.gl_config = Some(Default::default());
        }

        let open_settings = OpenSettings::new(&settings);

        Window::open_blocking(
            settings,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, build, update, state)
            },
        )
    }

    /// Update the pressed key modifiers when a mouse event has sent a new set of modifiers.
    fn update_modifiers(&mut self, modifiers: &Modifiers) {
        self.egui_input.modifiers.alt = !(*modifiers & Modifiers::ALT).is_empty();
        self.egui_input.modifiers.shift = !(*modifiers & Modifiers::SHIFT).is_empty();
        self.egui_input.modifiers.command = !(*modifiers & Modifiers::CONTROL).is_empty();
    }
}

impl<State, U> WindowHandler for EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn on_frame(&mut self, window: &mut Window) {
        let Some(state) = &mut self.user_state else {
            return;
        };

        self.egui_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.egui_ctx.begin_frame(self.egui_input.take());

        //let mut repaint_requested = false;
        let mut queue = Queue::new(&mut self.bg_color, &mut self.close_requested);

        (self.user_update)(&self.egui_ctx, &mut queue, state);

        if self.close_requested {
            window.close();
        }

        // Prevent data from being allocated every frame by storing this
        // in a member field.
        let mut full_output = self.egui_ctx.end_frame();

        let Some(viewport_output) = full_output.viewport_output.get(&self.viewport_id) else {
            // The main window was closed by egui.
            window.close();
            return;
        };

        for command in viewport_output.commands.iter() {
            match command {
                ViewportCommand::Close => {
                    window.close();
                }
                ViewportCommand::InnerSize(size) => window.resize(baseview::Size {
                    width: size.x.max(1.0) as f64,
                    height: size.y.max(1.0) as f64,
                }),
                _ => {}
            }
        }

        let now = Instant::now();
        let do_repaint_now = if let Some(t) = self.repaint_after {
            now >= t || viewport_output.repaint_delay.is_zero()
        } else {
            viewport_output.repaint_delay.is_zero()
        };

        if do_repaint_now {
            self.renderer.render(
                window,
                self.bg_color,
                (self.physical_width, self.physical_height),
                self.pixels_per_point,
                &mut self.egui_ctx,
                &mut full_output
            );

            self.repaint_after = None;
        } else if let Some(repaint_after) = now.checked_add(viewport_output.repaint_delay) {
            // Schedule to repaint after the requested time has elapsed.
            self.repaint_after = Some(repaint_after);
        }

        if !full_output.platform_output.copied_text.is_empty() {
            if let Some(clipboard_ctx) = &mut self.clipboard_ctx {
                if let Err(err) =
                    clipboard_ctx.set_contents(full_output.platform_output.copied_text.clone())
                {
                    log::error!("Copy/Cut error: {}", err);
                }
            }
        }

        if let Some(open_url) = &self.full_output.platform_output.open_url {
            if let Err(err) = open::that_detached(&open_url.url) {
                log::error!("Open error: {}", err);
            }
        }

        let cursor_icon =
            crate::translate::translate_cursor_icon(full_output.platform_output.cursor_icon);
        if self.current_cursor_icon != cursor_icon {
            self.current_cursor_icon = cursor_icon;

            // TODO: Set mouse cursor for other platforms once baseview supports it.
            #[cfg(target_os = "linux")]
            window.set_mouse_cursor(cursor_icon);
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved {
                    position,
                    modifiers,
                } => {
                    self.update_modifiers(modifiers);

                    let pos = pos2(position.x as f32, position.y as f32);
                    self.pointer_pos_in_points = Some(pos);
                    self.egui_input.events.push(egui::Event::PointerMoved(pos));
                }
                baseview::MouseEvent::ButtonPressed { button, modifiers } => {
                    self.update_modifiers(modifiers);

                    if let Some(pos) = self.pointer_pos_in_points {
                        if let Some(button) = crate::translate::translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: true,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::ButtonReleased { button, modifiers } => {
                    self.update_modifiers(modifiers);

                    if let Some(pos) = self.pointer_pos_in_points {
                        if let Some(button) = crate::translate::translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: false,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::WheelScrolled {
                    delta: scroll_delta,
                    modifiers,
                } => {
                    self.update_modifiers(modifiers);

                    let mut delta = match scroll_delta {
                        baseview::ScrollDelta::Lines { x, y } => {
                            egui::vec2(*x, *y) * self.points_per_scroll_line
                        }
                        baseview::ScrollDelta::Pixels { x, y } => {
                            egui::vec2(*x, *y) * self.points_per_pixel
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
                    self.pointer_pos_in_points = None;
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
                         // prevent `rustfmt` from breaking this
                    }
                    _ => (),
                }

                if let Some(key) = crate::translate::translate_virtual_key(&event.key) {
                    self.egui_input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: event.repeat,
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
                                    log::error!("Paste error: {}", err);
                                }
                            }
                        }
                    } else if let keyboard_types::Key::Character(written) = &event.key {
                        if !self.egui_input.modifiers.ctrl && !self.egui_input.modifiers.command {
                            self.egui_input
                                .events
                                .push(egui::Event::Text(written.clone()));
                        }
                    }
                }
            }
            baseview::Event::Window(event) => match event {
                baseview::WindowEvent::Resized(window_info) => {
                    self.pixels_per_point = match self.scale_policy {
                        WindowScalePolicy::ScaleFactor(scale) => scale,
                        WindowScalePolicy::SystemScaleFactor => window_info.scale(),
                    } as f32;
                    self.points_per_pixel = self.pixels_per_point.recip();

                    self.physical_width = window_info.physical_size().width;
                    self.physical_height = window_info.physical_size().height;

                    let logical_size = (
                        (self.physical_width as f32 * self.points_per_pixel),
                        (self.physical_height as f32 * self.points_per_pixel),
                    );

                    let screen_rect = Rect::from_min_size(
                        Pos2::new(0f32, 0f32),
                        vec2(logical_size.0, logical_size.1),
                    );

                    self.egui_input.screen_rect = Some(screen_rect);

                    let viewport_info = self
                        .egui_input
                        .viewports
                        .get_mut(&self.viewport_id)
                        .unwrap();
                    viewport_info.native_pixels_per_point = Some(self.pixels_per_point);
                    viewport_info.inner_rect = Some(screen_rect);

                    // Schedule to repaint on the next frame.
                    self.repaint_after = Some(Instant::now());
                }
                baseview::WindowEvent::Focused => {
                    self.egui_input
                        .events
                        .push(egui::Event::WindowFocused(true));
                    self.egui_input
                        .viewports
                        .get_mut(&self.viewport_id)
                        .unwrap()
                        .focused = Some(true);
                }
                baseview::WindowEvent::Unfocused => {
                    self.egui_input
                        .events
                        .push(egui::Event::WindowFocused(false));
                    self.egui_input
                        .viewports
                        .get_mut(&self.viewport_id)
                        .unwrap()
                        .focused = Some(false);
                }
                baseview::WindowEvent::WillClose => {}
            },
        }

        EventStatus::Captured
    }
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
