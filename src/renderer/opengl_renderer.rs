use baseview::gl::GlContext;
use baseview::Window;
use egui::{Color32, Rgba};

pub use baseview::gl::GlConfig as RenderSettings;

mod painter;
use painter::Painter;

pub struct Renderer {
    context: GlContext,
    painter: Painter,
}

impl Renderer {
    pub fn new(window: &Window, canvas_size: (u32, u32)) -> Option<Self> {
        let context = window.gl_context()?;

        unsafe { context.make_current() };

        gl::load_with(|s| context.get_proc_address(s) as _);

        let painter = Painter::new(canvas_size.0, canvas_size.1);

        unsafe { context.make_not_current() };

        Some(Self { context, painter })
    }

    pub fn render(
        &mut self,
        bg_color: Rgba,
        clipped_meshes: Vec<egui::ClippedMesh>,
        egui_font_image: &egui::FontImage,
        pixels_per_point: f32,
    ) {
        unsafe { self.context.make_current() };

        self.painter
            .paint_meshes(bg_color, clipped_meshes, egui_font_image, pixels_per_point);

        self.context.swap_buffers();
        unsafe { self.context.make_not_current() };
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
