//! Example of using `re_renderer` in standalone mode.
//!
//! To try it natively:
//! ```
//! cargo run -p re_renderer --example renderer_standalone
//! ```
//!
//! To try on the web:
//! ```
//! cargo run-wasm --example renderer_standalone
//! ```

use std::f32::consts::TAU;

use anyhow::Context as _;
use glam::{Affine3A, Quat, Vec3};
use macaw::IsoTransform;
use re_renderer::{
    context::{RenderContext, RenderContextConfig},
    renderer::{
        lines::{LineDrawable, LineStrip},
        GenericSkyboxDrawable, TestTriangleDrawable,
    },
    view_builder::{TargetConfiguration, ViewBuilder},
};
use type_map::concurrent::TypeMap;
use wgpu::{
    CommandEncoder, Device, Queue, RenderPass, Surface, SurfaceConfiguration, TextureFormat,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

// ---

// Rendering things using Rerun's renderer.

async fn run(event_loop: EventLoop<()>, window: Window) {
    let mut wgpu_ctx = WgpuContext::new(event_loop, window).await.unwrap();

    let re_ctx = RenderContext::new(
        &wgpu_ctx.device,
        &wgpu_ctx.queue,
        RenderContextConfig {
            output_format_color: wgpu_ctx.swapchain_format,
        },
    );

    // Store our `RenderContext` into the `WgpuContext` so that lifetime issues will
    // be handled for us.
    wgpu_ctx.user_data.insert(re_ctx);

    wgpu_ctx.run(
        // Setting up the `prepare` callback, which will be called once per frame with
        // a ready-to-be-filled `CommandEncoder`.
        |user_data, device, queue, encoder, resolution| {
            let mut view_builder = ViewBuilder::new();

            let pos = Vec3::new(0.0, 0.0, 3.0);
            let iso = IsoTransform::from_rotation_translation(
                Quat::from_affine3(&Affine3A::look_at_rh(pos, Vec3::ZERO, Vec3::Y).inverse()),
                pos,
            );
            let target_cfg = TargetConfiguration {
                resolution_in_pixel: resolution,
                world_from_view: iso,
                fov_y: 70.0 * TAU / 360.0,
                near_plane_distance: 0.01,
                target_identifier: 0,
            };

            let re_ctx = user_data.get_mut::<RenderContext>().unwrap();

            let triangle = TestTriangleDrawable::new(re_ctx, device);
            let skybox = GenericSkyboxDrawable::new(re_ctx, device);
            let lines = LineDrawable::new(
                re_ctx,
                device,
                queue,
                &[LineStrip {
                    points: vec![glam::vec3(0.0, 0.0, 0.0), glam::vec3(1.0, 1.0, 0.0)],
                    radius: 1.0,
                    color: [255, 255, 0],
                }],
            )
            .unwrap();

            view_builder
                .setup_view(re_ctx, device, queue, &target_cfg)
                .unwrap()
                //.queue_draw(&triangle)
                .queue_draw(&skybox)
                .queue_draw(&lines)
                .draw(re_ctx, encoder)
                .unwrap();

            view_builder
        },
        // Setting up the `draw` callback, which will be called once per frame with the
        // renderpass drawing onto the swapchain.
        {
            |user_data, rpass, frame_builder: ViewBuilder| {
                let re_ctx = user_data.get::<RenderContext>().unwrap();
                frame_builder.composite(re_ctx, rpass).unwrap();
            }
        },
    );
}

// ---

// Usual winit + wgpu initialization stuff

struct WgpuContext {
    event_loop: EventLoop<()>,
    window: Window,
    device: Device,
    queue: Queue,
    swapchain_format: TextureFormat,
    surface: Surface,
    surface_config: SurfaceConfiguration,

    pub user_data: TypeMap,
}

impl WgpuContext {
    async fn new(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("failed to find an appropriate adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .context("failed to create device")?;

        let swapchain_format = if cfg!(target_arch = "wasm32") {
            wgpu::TextureFormat::Rgba8Unorm
        } else {
            wgpu::TextureFormat::Bgra8Unorm
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            // Not the best setting in general, but nice for quick & easy performance checking.
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &surface_config);

        Ok(Self {
            event_loop,
            window,
            device,
            queue,
            swapchain_format,
            surface,
            surface_config,
            user_data: TypeMap::new(),
        })
    }

    fn run<DrawData, Prepare, Draw>(mut self, mut prepare: Prepare, mut draw: Draw)
    where
        Prepare: FnMut(&mut TypeMap, &Device, &Queue, &mut CommandEncoder, [u32; 2]) -> DrawData
            + 'static,
        Draw: for<'a, 'b> FnMut(&'b mut TypeMap, &'a mut RenderPass<'b>, DrawData) + 'static,
    {
        self.event_loop.run(move |event, _, control_flow| {
            // Keep our example busy.
            // Not how one should generally do it, but great for animated content and checking on perf.
            *control_flow = ControlFlow::Poll;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    self.surface_config.width = size.width;
                    self.surface_config.height = size.height;
                    self.surface.configure(&self.device, &self.surface_config);
                    self.window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    let start_time = std::time::Instant::now();

                    let frame = self
                        .surface
                        .get_current_texture()
                        .expect("failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = self
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    let prepared = prepare(
                        &mut self.user_data,
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        [self.surface_config.width, self.surface_config.height],
                    );

                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });

                        draw(&mut self.user_data, &mut rpass, prepared);
                    }

                    self.queue.submit(Some(encoder.finish()));
                    frame.present();

                    self.user_data
                        .get_mut::<RenderContext>()
                        .unwrap()
                        .frame_maintenance(&self.device);

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // Note that this measures time spent on CPU, not GPU
                        // However, iff we're GPU bound (likely for this sample) and GPU times are somewhat stable,
                        // we eventually end up waiting for GPU in `get_current_texture`
                        // (wgpu has a swap chain with a limited amount of buffers, the exact count is dependent on `present_mode` and backend!).
                        // It's important to keep in mind that depending on the `present_mode`, the GPU might be waiting on the screen in turn.
                        let time_passed = std::time::Instant::now() - start_time;
                        // TODO(andreas): Display a median over n frames and while we're on it also stddev thereof.
                        self.window.set_title(&format!(
                            "{:.2} ms ({:.2} fps)",
                            time_passed.as_secs_f32() * 1000.0,
                            1.0 / time_passed.as_secs_f32()
                        ));
                    }
                }
                Event::MainEventsCleared => {
                    self.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        });
    }
}

// ---

fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Rerun Viewer")
        .build(&event_loop)
        .unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Set size to a common physical resolution as a comparable start-up default.
        window.set_inner_size(winit::dpi::PhysicalSize {
            width: 1920,
            height: 1080,
        });

        // Enable wgpu info messages by default
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "wgpu=info"),
        );
        pollster::block_on(run(event_loop, window));
    }

    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
