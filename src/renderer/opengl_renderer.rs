use baseview::Window;
use egui::{Color32, Rgba};
use raw_gl_context::GlContext;

pub use raw_gl_context::GlConfig as RenderSettings;

mod painter;
use painter::Painter;

pub struct Renderer {
    context: GlContext,
    painter: Painter,
}

impl Renderer {
    pub fn new(window: &Window, render_settings: RenderSettings, canvas_size: (u32, u32)) -> Self {
        let context = GlContext::create(window, render_settings).unwrap();

        context.make_current();

        gl::load_with(|s| context.get_proc_address(s) as _);

        let painter = Painter::new(canvas_size.0, canvas_size.1);

        context.make_not_current();

        Self { context, painter }
    }

    pub fn render(
        &mut self,
        bg_color: Rgba,
        clipped_meshes: Vec<egui::ClippedMesh>,
        egui_texture: &egui::Texture,
        pixels_per_point: f32,
    ) {
        self.context.make_current();

        self.painter
            .paint_meshes(bg_color, clipped_meshes, egui_texture, pixels_per_point);

        self.context.swap_buffers();
        self.context.make_not_current();
    }

    pub fn new_user_texture(
        &mut self,
        size: (usize, usize),
        srgba_pixels: &[Color32],
        filtering: bool,
    ) -> egui::TextureId {
        self.painter.new_user_texture(size, srgba_pixels, filtering)
    }

    pub fn update_user_texture_data(&mut self, texture_id: egui::TextureId, pixels: &[Color32]) {
        self.painter.update_user_texture_data(texture_id, pixels)
    }

    pub fn update_window_size(&mut self, width: u32, height: u32) {
        self.painter.set_canvas_size(width, height);
    }
}
