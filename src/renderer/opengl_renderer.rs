use baseview::Window;
use egui::Srgba;
use raw_gl_context::GlContext;

pub use raw_gl_context::GlConfig as RenderSettings;

mod painter;
pub use painter::Painter;

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
        bg_color: Srgba,
        jobs: egui::PaintJobs,
        egui_texture: &egui::Texture,
        pixels_per_point: f32,
    ) {
        self.context.make_current();

        self.painter
            .paint_jobs(bg_color, jobs, egui_texture, pixels_per_point);

        self.context.swap_buffers();
        self.context.make_not_current();
    }

    pub fn painter(&mut self) -> &mut Painter {
        &mut self.painter
    }
}
