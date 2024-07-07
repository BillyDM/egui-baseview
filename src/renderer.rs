#[cfg(feature = "opengl")]
mod opengl_renderer;
#[cfg(feature = "opengl")]
pub use opengl_renderer::Renderer;

#[cfg(feature = "wgpu")]
mod wgpu_renderer;
#[cfg(feature = "wgpu")]
pub use wgpu_renderer::Renderer;
