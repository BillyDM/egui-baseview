use baseview::{PhySize, Window};
use egui::FullOutput;
use egui_glow::Painter;
use std::sync::Arc;

use super::OpenGlError;

pub struct Renderer {
    glow_context: Arc<egui_glow::glow::Context>,
    painter: Painter,
}

impl Renderer {
    pub fn new(window: &Window) -> Result<Self, OpenGlError> {
        let context = window.gl_context().ok_or(OpenGlError::NoContext)?;
        unsafe {
            context.make_current();
        }

        #[allow(clippy::arc_with_non_send_sync)]
        let glow_context = Arc::new(unsafe {
            egui_glow::glow::Context::from_loader_function(|s| context.get_proc_address(s))
        });

        let painter = egui_glow::Painter::new(Arc::clone(&glow_context), "", None)
            .map_err(OpenGlError::CreatePainter)?;

        unsafe {
            context.make_not_current();
        }

        Ok(Self {
            glow_context,
            painter,
        })
    }

    pub fn max_texture_side(&self) -> usize {
        self.painter.max_texture_side()
    }

    pub fn render(
        &mut self,
        window: &Window,
        bg_color: egui::Rgba,
        physical_size: PhySize,
        pixels_per_point: f32,
        egui_ctx: &mut egui::Context,
        full_output: &mut FullOutput,
    ) {
        let PhySize {
            width: canvas_width,
            height: canvas_height,
        } = physical_size;

        let shapes = std::mem::take(&mut full_output.shapes);
        let textures_delta = &mut full_output.textures_delta;

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

        for (id, image_delta) in &textures_delta.set {
            self.painter.set_texture(*id, image_delta);
        }

        let clipped_primitives = egui_ctx.tessellate(shapes, pixels_per_point);
        let dimensions: [u32; 2] = [canvas_width, canvas_height];

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
