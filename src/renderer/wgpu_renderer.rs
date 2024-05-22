use std::{
    num::{NonZeroIsize, NonZeroU32},
    ptr::NonNull,
    sync::Arc,
};

use baseview::Window;
use egui_wgpu::{
    wgpu::{
        Color, CommandEncoderDescriptor, Extent3d, Instance, InstanceDescriptor,
        RenderPassColorAttachment, RenderPassDescriptor, Surface, SurfaceConfiguration,
        SurfaceTargetUnsafe, TextureDescriptor, TextureDimension, TextureUsages, TextureView,
        TextureViewDescriptor,
    },
    RenderState, ScreenDescriptor, WgpuConfiguration,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use raw_window_handle_06::{
    AppKitDisplayHandle, AppKitWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
    XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};

const MSAA_SAMPLES: u32 = 4;

pub struct Renderer {
    render_state: Arc<RenderState>,
    surface: Surface<'static>,
    configuration: WgpuConfiguration,
    msaa_texture_view: Option<TextureView>,
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let instance = Instance::new(InstanceDescriptor::default());

        let raw_display_handle = window.raw_display_handle();
        let raw_window_handle = window.raw_window_handle();

        let target = SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: match raw_display_handle {
                raw_window_handle::RawDisplayHandle::AppKit(_) => {
                    raw_window_handle_06::RawDisplayHandle::AppKit(AppKitDisplayHandle::new())
                }
                raw_window_handle::RawDisplayHandle::Xlib(handle) => {
                    raw_window_handle_06::RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                        NonNull::new(handle.display),
                        handle.screen,
                    ))
                }
                raw_window_handle::RawDisplayHandle::Xcb(handle) => {
                    raw_window_handle_06::RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                        NonNull::new(handle.connection),
                        handle.screen,
                    ))
                }
                raw_window_handle::RawDisplayHandle::Windows(_) => {
                    raw_window_handle_06::RawDisplayHandle::Windows(WindowsDisplayHandle::new())
                }
                _ => todo!(),
            },
            raw_window_handle: match raw_window_handle {
                raw_window_handle::RawWindowHandle::AppKit(handle) => {
                    raw_window_handle_06::RawWindowHandle::AppKit(AppKitWindowHandle::new(
                        NonNull::new(handle.ns_view).unwrap(),
                    ))
                }
                raw_window_handle::RawWindowHandle::Xlib(handle) => {
                    raw_window_handle_06::RawWindowHandle::Xlib(XlibWindowHandle::new(
                        handle.window,
                    ))
                }
                raw_window_handle::RawWindowHandle::Xcb(handle) => {
                    raw_window_handle_06::RawWindowHandle::Xcb(XcbWindowHandle::new(
                        NonZeroU32::new(handle.window).unwrap(),
                    ))
                }
                // will this work? i have no idea!
                raw_window_handle::RawWindowHandle::Win32(handle) => {
                    let mut raw_handle = Win32WindowHandle::new(NonZeroIsize::new(handle.hwnd as isize).unwrap());

                    raw_handle.hinstance = handle.hinstance.is_null().then_some(NonZeroIsize::new(handle.hinstance as isize).unwrap());

                    
                    raw_window_handle_06::RawWindowHandle::Win32(raw_handle)
                }
                _ => todo!(),
            },
        };

        let surface = unsafe { instance.create_surface_unsafe(target) }.unwrap();
        let configuration = WgpuConfiguration::default();

        let state = Arc::new(
            pollster::block_on(RenderState::create(
                &configuration,
                &instance,
                &surface,
                None,
                MSAA_SAMPLES,
            ))
            .unwrap(),
        );

        Self {
            render_state: state,
            surface,
            configuration,
            msaa_texture_view: None,
            width: 0,
            height: 0,
        }
    }

    pub fn max_texture_side(&self) -> usize {
        self.render_state
            .as_ref()
            .device
            .limits()
            .max_texture_dimension_2d as usize
    }

    fn configure_surface(&self, width: u32, height: u32) {
        let usage = TextureUsages::RENDER_ATTACHMENT;

        let mut surf_config = SurfaceConfiguration {
            usage,
            format: self.render_state.target_format,
            present_mode: self.configuration.present_mode,
            view_formats: vec![self.render_state.target_format],
            ..self
                .surface
                .get_default_config(&self.render_state.adapter, width, height)
                .expect("Unsupported surface")
        };

        if let Some(desired_maximum_frame_latency) =
            self.configuration.desired_maximum_frame_latency
        {
            surf_config.desired_maximum_frame_latency = desired_maximum_frame_latency;
        }

        self.surface
            .configure(&self.render_state.device, &surf_config);
    }

    fn resize_and_generate_msaa_view(&mut self, width: u32, height: u32) {
        let render_state = self.render_state.as_ref();

        self.width = width;
        self.height = height;

        self.configure_surface(width, height);

        let texture_format = render_state.target_format;
        self.msaa_texture_view = Some(
            render_state
                .device
                .create_texture(&TextureDescriptor {
                    label: Some("egui_msaa_texture"),
                    size: Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: MSAA_SAMPLES,
                    dimension: TextureDimension::D2,
                    format: texture_format,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[texture_format],
                })
                .create_view(&TextureViewDescriptor::default()),
        );
    }

    pub fn render(
        &mut self,
        _window: &Window,
        bg_color: egui::Rgba,
        canvas_width: u32,
        canvas_height: u32,
        pixels_per_point: f32,
        egui_ctx: &mut egui::Context,
        shapes: &mut Vec<egui::epaint::ClippedShape>,
        textures_delta: &mut egui::TexturesDelta,
    ) {
        let shapes = std::mem::take(shapes);

        let clipped_primitives = egui_ctx.tessellate(shapes, pixels_per_point);

        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [canvas_width, canvas_height],
            pixels_per_point,
        };

        let user_cmd_bufs = {
            let mut renderer = self.render_state.renderer.write();
            for (id, image_delta) in &textures_delta.set {
                renderer.update_texture(
                    &self.render_state.device,
                    &self.render_state.queue,
                    *id,
                    image_delta,
                );
            }

            renderer.update_buffers(
                &self.render_state.device,
                &self.render_state.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            )
        };

        if self.width != canvas_width
            || self.height != canvas_height
            || self.msaa_texture_view.is_none()
        {
            self.resize_and_generate_msaa_view(canvas_width, canvas_height);
        }

        let output_frame = { self.surface.get_current_texture() };

        let output_frame = match output_frame {
            Ok(frame) => frame,
            Err(err) => match (self.configuration.on_surface_error)(err) {
                egui_wgpu::SurfaceErrorAction::SkipFrame => return,
                egui_wgpu::SurfaceErrorAction::RecreateSurface => {
                    self.configure_surface(self.width, self.height);
                    return;
                }
            },
        };

        {
            let renderer = self.render_state.renderer.read();
            let frame_view = output_frame
                .texture
                .create_view(&TextureViewDescriptor::default());

            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui_render"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: self.msaa_texture_view.as_ref().unwrap(),
                    resolve_target: Some(&frame_view),
                    ops: egui_wgpu::wgpu::Operations {
                        load: egui_wgpu::wgpu::LoadOp::Clear(Color {
                            r: bg_color[0] as f64,
                            g: bg_color[1] as f64,
                            b: bg_color[2] as f64,
                            a: bg_color[3] as f64,
                        }),
                        store: egui_wgpu::wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        {
            let mut renderer = self.render_state.renderer.write();
            for id in &textures_delta.free {
                renderer.free_texture(id);
            }
        }

        let encoded = encoder.finish();

        self.render_state
            .queue
            .submit(user_cmd_bufs.into_iter().chain([encoded]));

        output_frame.present();
    }
}
