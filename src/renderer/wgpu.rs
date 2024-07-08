use std::sync::Arc;

use egui_wgpu::SurfaceErrorAction;
use wgpu::{Adapter, Backends, DeviceDescriptor, PowerPreference, PresentMode, SurfaceError};

pub mod renderer;

pub struct WgpuConfiguration {
    pub supported_backends: Backends,
    pub device_descriptor: Arc<dyn Fn(&Adapter) -> DeviceDescriptor<'static> + Send + Sync>,
    pub present_mode: PresentMode,
    pub desired_maximum_frame_latency: Option<u32>,
    pub power_preference: PowerPreference,
    pub on_surface_error: Arc<dyn Fn(SurfaceError) -> SurfaceErrorAction + Send + Sync>,
}

impl Into<egui_wgpu::WgpuConfiguration> for WgpuConfiguration {
    fn into(self) -> egui_wgpu::WgpuConfiguration {
        let Self {
            supported_backends,
            device_descriptor,
            present_mode,
            desired_maximum_frame_latency,
            power_preference,
            on_surface_error,
        } = self;

        egui_wgpu::WgpuConfiguration {
            supported_backends,
            device_descriptor,
            present_mode,
            desired_maximum_frame_latency,
            power_preference,
            on_surface_error,
        }
    }
}

// this is absolutely disgusting, this is a copy paste of egui's default wgpu configuration. rustc has forced me hand.
impl Default for WgpuConfiguration {
    fn default() -> Self {
        Self {
            // Add GL backend, primarily because WebGPU is not stable enough yet.
            // (note however, that the GL backend needs to be opted-in via the wgpu feature flag "webgl")
            supported_backends: wgpu::util::backend_bits_from_env()
                .unwrap_or(wgpu::Backends::PRIMARY | wgpu::Backends::GL),

            device_descriptor: Arc::new(|adapter| {
                let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                };

                wgpu::DeviceDescriptor {
                    label: Some("egui wgpu device"),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits {
                        // When using a depth buffer, we have to be able to create a texture
                        // large enough for the entire surface, and we want to support 4k+ displays.
                        max_texture_dimension_2d: 8192,
                        ..base_limits
                    },
                }
            }),

            present_mode: wgpu::PresentMode::AutoVsync,

            desired_maximum_frame_latency: None,

            power_preference: wgpu::util::power_preference_from_env()
                .unwrap_or(wgpu::PowerPreference::HighPerformance),

            on_surface_error: Arc::new(|err| {
                if err == wgpu::SurfaceError::Outdated {
                    // This error occurs when the app is minimized on Windows.
                    // Silently return here to prevent spamming the console with:
                    // "The underlying surface has changed, and therefore the swap chain must be updated"
                } else {
                    log::warn!("Dropped frame with error: {err}");
                }
                SurfaceErrorAction::SkipFrame
            }),
        }
    }
}
