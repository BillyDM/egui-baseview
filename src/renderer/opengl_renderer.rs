use baseview::{PhySize, Window};
use egui_glow::Painter;
use std::sync::Arc;

pub struct Renderer {
    glow_context: Arc<egui_glow::glow::Context>,
    painter: Painter,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let context = window
            .gl_context()
            .expect("failed to get baseview gl context");
        unsafe {
            context.make_current();
        }

        let glow_context = Arc::new(unsafe {
            egui_glow::glow::Context::from_loader_function(|s| context.get_proc_address(s))
        });

        let painter = egui_glow::Painter::new(Arc::clone(&glow_context), "", None)
            .map_err(|error| {
                log::error!("Error occurred in initializing painter:\n{}", error);
            })
            .unwrap();

        unsafe {
            context.make_not_current();
        }

        Self {
            glow_context,
            painter,
        }
    }

    pub fn max_texture_side(&self) -> usize {
        self.painter.max_texture_side()
    }

    pub fn render(
        &mut self,
        window: &Window,
        bg_color: egui::Rgba,
        canvas_size: PhySize,
        pixels_per_point: f32,
        egui_ctx: &mut egui::Context,
        shapes: &mut Vec<egui::epaint::ClippedShape>,
        textures_delta: &mut egui::TexturesDelta,
    ) {
        let shapes = std::mem::take(shapes);
        let mut textures_delta = std::mem::take(textures_delta);

        let context = window
            .gl_context()
            .expect("failed to get baseview gl context");
        unsafe {
            context.make_current();
        }

        unsafe {
            use egui_glow::glow::HasContext as _;
            self.glow_context
                .clear_color(bg_color.r(), bg_color.g(), bg_color.b(), bg_color.a());
            self.glow_context.clear(egui_glow::glow::COLOR_BUFFER_BIT);
        }

        for (id, image_delta) in textures_delta.set {
            self.painter.set_texture(id, &image_delta);
        }

        let clipped_primitives = egui_ctx.tessellate(shapes, pixels_per_point);
        let dimensions: [u32; 2] = [canvas_size.width, canvas_size.height];

        self.painter
            .paint_primitives(dimensions, pixels_per_point, &clipped_primitives);

        for id in textures_delta.free.drain(..) {
            self.painter.free_texture(id);
        }

        unsafe {
            context.swap_buffers();
            context.make_not_current();
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.painter.destroy()
    }
}
