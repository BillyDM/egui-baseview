use baseview::Window;
use egui::FullOutput;

#[cfg(feature = "opengl")]
mod opengl_renderer;

#[cfg(feature = "wgpu")]
mod wgpu_renderer;

pub(crate) enum Renderer {
    #[cfg(feature = "opengl")]
    OpenGL(opengl_renderer::Renderer),
    #[cfg(feature = "wgpu")]
    Wgpu(wgpu_renderer::Renderer)
}

impl Renderer {
    pub(crate) fn render(&mut self, window: &Window, bg_color: egui::Rgba,
        dimensions: (u32, u32),
        pixels_per_point: f32,
        egui_ctx: &mut egui::Context,
        full_output: &mut FullOutput) {
            match self {
                #[cfg(feature = "opengl")]
                Renderer::OpenGL(renderer) => renderer.render(window, bg_color, dimensions, pixels_per_point, egui_ctx, full_output),
                #[cfg(feature = "wgpu")]
                Renderer::Wgpu(renderer) => renderer.render(bg_color, dimensions, pixels_per_point, egui_ctx, full_output),
                #[allow(unreachable_patterns)]
                _ => unreachable!()
            }
        }

    pub(crate) fn max_texture_side(&self) -> usize {
        match self {
            #[cfg(feature = "opengl")]
            Renderer::OpenGL(renderer) => renderer.max_texture_side(),
            #[cfg(feature = "wgpu")]
            Renderer::Wgpu(renderer) => renderer.max_texture_side(),
            #[allow(unreachable_patterns)]
            _ => unreachable!()
        }
    }
}

// unreacable!() being here suppresses a compiler error, so the user hopefully realizes that they're supposed to enable a backend
#[allow(unreachable_code)]
pub(crate) fn get_renderer(window: &Window) -> Renderer {
    #[cfg(not(any(feature = "opengl", feature = "wgpu")))]
    compile_error!("No renderer present. Please enable either opengl or wgpu in the crate's features");

    #[cfg(feature = "wgpu")]
    if let Ok(renderer) = wgpu_renderer::Renderer::new(window) {
        return Renderer::Wgpu(renderer);
    }

    #[cfg(feature = "opengl")]
    return Renderer::OpenGL(opengl_renderer::Renderer::new(window));

    unreachable!()
}
