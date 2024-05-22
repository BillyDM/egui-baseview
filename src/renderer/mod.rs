#[cfg(feature = "opengl")]
mod opengl_renderer;
#[cfg(feature = "opengl")]
pub(crate) use opengl_renderer::Renderer;

#[cfg(feature = "wgpu")]
mod wgpu_renderer;
#[cfg(feature = "wgpu")]
pub(crate) use wgpu_renderer::Renderer;
