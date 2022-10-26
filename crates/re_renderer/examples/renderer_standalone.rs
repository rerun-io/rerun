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
use glam::Vec3;
use instant::Instant;
use log::info;
use macaw::IsoTransform;
use re_renderer::{
    context::{RenderContext, RenderContextConfig},
    renderer::{
        lines::{LineDrawable, LineStrip},
        GenericSkyboxDrawable, TestTriangleDrawable,
    },
    view_builder::{TargetConfiguration, ViewBuilder},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

// ---

// Rendering things using Rerun's renderer.

async fn run(event_loop: EventLoop<()>, window: Window) {
    let app = Application::new(event_loop, window).await.unwrap();
    app.run();
}

/// Uses a [`re_renderer::ViewBuilder`] to draw an example scene.
fn draw_view(
    time: &Time,
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    resolution: [u32; 2],
) -> ViewBuilder {
    let mut view_builder = ViewBuilder::new();

    // Rotate camera around the center at a distance of 5, looking down at 45 deg
    let seconds_since_startup = time.seconds_since_startup();
    let pos = Vec3::new(
        seconds_since_startup.sin(),
        0.5,
        seconds_since_startup.cos(),
    ) * 20.0;
    let view_from_world = IsoTransform::look_at_rh(pos, Vec3::ZERO, Vec3::Y).unwrap();
    let target_cfg = TargetConfiguration {
        resolution_in_pixel: resolution,
        view_from_world,
        fov_y: 70.0 * TAU / 360.0,
        near_plane_distance: 0.01,
        target_identifier: 0,
    };

    let triangle = TestTriangleDrawable::new(re_ctx, device);
    let skybox = GenericSkyboxDrawable::new(re_ctx, device);
    let lines = build_lines(re_ctx, device, queue, seconds_since_startup);

    view_builder
        .setup_view(re_ctx, device, queue, &target_cfg)
        .unwrap()
        .queue_draw(&triangle)
        .queue_draw(&skybox)
        .queue_draw(&lines)
        .draw(re_ctx, encoder)
        .unwrap();

    view_builder
}

fn build_lines(
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    seconds_since_startup: f32,
) -> LineDrawable {
    // Calculate some points that look nice for an animated line.
    let lorenz_points = {
        // Lorenz attractor https://en.wikipedia.org/wiki/Lorenz_system
        fn lorenz_integrate(cur: glam::Vec3, dt: f32) -> glam::Vec3 {
            let sigma: f32 = 10.0;
            let rho: f32 = 28.0;
            let beta: f32 = 8.0 / 3.0;

            cur + glam::vec3(
                sigma * (cur.y - cur.x),
                cur.x * (rho - cur.z) - cur.y,
                cur.x * cur.y - beta * cur.z,
            ) * dt
        }

        // slow buildup and reset
        let num_points = (((seconds_since_startup * 0.05).fract() * 10000.0) as u32).max(1);

        let mut latest_point = glam::vec3(-0.1, 0.001, 0.0);
        std::iter::repeat_with(move || {
            latest_point = lorenz_integrate(latest_point, 0.005);
            latest_point
        })
        // lorenz system is sensitive to start conditions (.. that's the whole point), so transform after the fact
        .map(|p| (p + glam::vec3(-5.0, 0.0, -23.0)) * 0.6)
        .take(num_points as _)
        .collect::<Vec<_>>()
    };

    LineDrawable::new(
        re_ctx,
        device,
        queue,
        &[
            // Complex orange line.
            LineStrip {
                points: lorenz_points,
                radius: 0.05,
                color: [255, 191, 0, 255],
            },
            // Yellow Zig-Zag
            LineStrip {
                points: vec![
                    glam::vec3(0.0, -1.0, 0.0),
                    glam::vec3(1.0, 0.0, 0.0),
                    glam::vec3(2.0, -1.0, 0.0),
                    glam::vec3(3.0, 0.0, 0.0),
                ],
                radius: 0.1,
                color: [50, 255, 50, 255],
            },
            // A blue spiral
            LineStrip {
                points: (0..1000)
                    .map(|i| {
                        glam::vec3(
                            (i as f32 * 0.01).sin() * 2.0,
                            i as f32 * 0.01 - 6.0,
                            (i as f32 * 0.01).cos() * 2.0,
                        )
                    })
                    .collect(),
                radius: 0.1,
                color: [50, 50, 255, 255],
            },
        ],
    )
    .unwrap()
}

// ---

// Usual winit + wgpu initialization stuff

struct Application {
    event_loop: EventLoop<()>,
    window: Window,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    time: Time,

    pub re_ctx: RenderContext,
}

impl Application {
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
            // TODO(andreas): It seems at least on Metal M1 this still does not discard command buffers that come in too fast (even when using `Immediate` explicitly).
            //                  Quick look into wgpu looks like it does it correctly there. OS limitation? iOS has this limitation, so wouldn't be surprising!
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &surface_config);

        let re_ctx = RenderContext::new(
            &device,
            &queue,
            RenderContextConfig {
                output_format_color: swapchain_format,
            },
        );
        Ok(Self {
            event_loop,
            window,
            device,
            queue,
            surface,
            surface_config,
            re_ctx,
            time: Time {
                start_time: Instant::now(),
                last_draw_time: Instant::now(),
            },
        })
    }

    fn run(mut self) {
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
                Event::WindowEvent {
                    event:
                        WindowEvent::ScaleFactorChanged {
                            scale_factor: _,
                            new_inner_size,
                        },
                    ..
                } => {
                    self.surface_config.width = new_inner_size.width;
                    self.surface_config.height = new_inner_size.height;
                    self.surface.configure(&self.device, &self.surface_config);
                    self.window.request_redraw();
                }
                Event::RedrawRequested(_) => {
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

                    let view_builder = draw_view(
                        &self.time,
                        &mut self.re_ctx,
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        [self.surface_config.width, self.surface_config.height],
                    );

                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                        view_builder
                            .composite(&self.re_ctx, &mut pass)
                            .expect("Failed to composite view main surface");
                    }

                    self.queue.submit(Some(encoder.finish()));
                    frame.present();

                    self.re_ctx.frame_maintenance(&self.device);

                    // Note that this measures time spent on CPU, not GPU
                    // However, iff we're GPU bound (likely for this sample) and GPU times are somewhat stable,
                    // we eventually end up waiting for GPU in `get_current_texture`
                    // (wgpu has a swap chain with a limited amount of buffers, the exact count is dependent on `present_mode` and backend!).
                    // It's important to keep in mind that depending on the `present_mode`, the GPU might be waiting on the screen in turn.
                    let current_time = Instant::now();
                    let time_passed = Instant::now() - self.time.last_draw_time;
                    self.time.last_draw_time = current_time;

                    // TODO(andreas): Display a median over n frames and while we're on it also stddev thereof.
                    // Repeatedly setting the title causes issues on some platforms
                    // Do it only every second.
                    let time_until_next_report = 1.0 - self.time.seconds_since_startup().fract();
                    if time_until_next_report - time_passed.as_secs_f32() < 0.0 {
                        let time_info_str = format!(
                            "{:.2} ms ({:.2} fps)",
                            time_passed.as_secs_f32() * 1000.0,
                            1.0 / time_passed.as_secs_f32()
                        );
                        self.window.set_title(&time_info_str);
                        info!("{time_info_str}");
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

struct Time {
    start_time: Instant,
    last_draw_time: Instant,
}

impl Time {
    fn seconds_since_startup(&self) -> f32 {
        (Instant::now() - self.start_time).as_secs_f32()
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
        env_logger::init_from_env(env_logger::Env::default().filter_or(
            env_logger::DEFAULT_FILTER_ENV,
            "wgpu=info,renderer_standalone",
        ));
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
