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
use rand::Rng;
use re_renderer::{
    context::{RenderContext, RenderContextConfig},
    renderer::{
        lines::{LineDrawable, LineStrip},
        point_cloud::PointCloudPoint,
        GenericSkyboxDrawable, PointCloudDrawable, TestTriangleDrawable,
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
    state: &AppState,
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    resolution: [u32; 2],
) -> ViewBuilder {
    let mut view_builder = ViewBuilder::new();

    // Rotate camera around the center at a distance of 5, looking down at 45 deg
    let seconds_since_startup = state.time.seconds_since_startup();
    let pos = Vec3::new(
        seconds_since_startup.sin(),
        0.5,
        seconds_since_startup.cos(),
    ) * 15.0;
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
    let point_cloud = PointCloudDrawable::new(re_ctx, device, queue, &state.random_points).unwrap();

    view_builder
        .setup_view(re_ctx, device, queue, &target_cfg)
        .unwrap()
        .queue_draw(&triangle)
        .queue_draw(&skybox)
        .queue_draw(&point_cloud)
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

// Special error handling datastructures for debug builds (never crash!)

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
use error_handling::*;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
mod error_handling {
    use ahash::HashSet;
    use parking_lot::Mutex;
    use std::sync::{atomic::AtomicUsize, Arc};
    use wgpu_core::error::ContextError;

    fn type_of_wgpu_error(
        error: &(dyn std::error::Error + Send + Sync + 'static),
    ) -> Option<std::any::TypeId> {
        fn type_of_var<T: 'static>(_: &T) -> std::any::TypeId {
            std::any::TypeId::of::<T>()
        }

        macro_rules! try_downcast {
            [$typ:ty $(,)*] => {
                if let Some(inner) = error.downcast_ref::<$typ>() {
                    return Some(type_of_var(inner));
                }
            };
            [$typ:ty, $($rest:ty),+ $(,)*] => {
                try_downcast![$typ];
                try_downcast![$($rest),+];
            };
        }

        try_downcast![
            wgpu_core::command::ClearError,
            wgpu_core::command::CommandEncoderError,
            wgpu_core::command::ComputePassError,
            wgpu_core::command::CopyError,
            wgpu_core::command::DispatchError,
            wgpu_core::command::DrawError,
            wgpu_core::command::ExecutionError,
            wgpu_core::command::PassErrorScope,
            wgpu_core::command::QueryError,
            wgpu_core::command::QueryUseError,
            wgpu_core::command::RenderBundleError,
            wgpu_core::command::RenderCommandError,
            wgpu_core::command::RenderPassError,
            wgpu_core::command::ResolveError,
            wgpu_core::command::TransferError,
            wgpu_core::binding_model::BindError,
            wgpu_core::binding_model::BindingTypeMaxCountError,
            wgpu_core::binding_model::CreateBindGroupError,
            wgpu_core::binding_model::CreatePipelineLayoutError,
            wgpu_core::binding_model::GetBindGroupLayoutError,
            wgpu_core::binding_model::PushConstantUploadError,
            wgpu_core::device::CreateDeviceError,
            wgpu_core::device::DeviceError,
            wgpu_core::device::RenderPassCompatibilityError,
            wgpu_core::pipeline::ColorStateError,
            wgpu_core::pipeline::CreateComputePipelineError,
            wgpu_core::pipeline::CreateRenderPipelineError,
            wgpu_core::pipeline::CreateShaderModuleError,
            wgpu_core::pipeline::DepthStencilStateError,
            wgpu_core::pipeline::ImplicitLayoutError,
        ];

        None
    }

    #[derive(Debug)]
    pub struct WrappedContextError(Box<ContextError>);
    impl std::hash::Hash for WrappedContextError {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.label.hash(state); // e.g. "composite_encoder"
            self.0.label_key.hash(state); // e.g. "encoder"
            self.0.string.hash(state); // e.g. "a RenderPass"

            if let Some(type_id) = type_of_wgpu_error(&*self.0.cause) {
                type_id.hash(state);
            } else {
                re_log::warn!(cause=?self.0.cause, "unknown error cause");
            }
        }
    }
    impl PartialEq for WrappedContextError {
        fn eq(&self, rhs: &Self) -> bool {
            self.0.label.eq(&rhs.0.label)
                && self.0.label_key.eq(rhs.0.label_key)
                && self.0.string.eq(rhs.0.string)
                && type_of_wgpu_error(&*self.0.cause).eq(&type_of_wgpu_error(&*rhs.0.cause))
        }
    }
    impl Eq for WrappedContextError {}

    #[derive(Default, Clone)]
    pub struct ErrorTracker {
        frame_nr: Arc<AtomicUsize>,
        errors: Arc<Mutex<HashSet<WrappedContextError>>>,
    }
    impl ErrorTracker {
        pub fn tick(&self) {
            self.frame_nr
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        pub fn clear(&self) {
            self.errors.lock().clear();
            re_log::debug!("cleared WGPU error tracker");
        }

        pub fn handle_error(&self, error: wgpu::Error) {
            match error {
                wgpu::Error::OutOfMemory { source: _ } => panic!("{error}"),
                wgpu::Error::Validation {
                    source,
                    description,
                } => {
                    match source.downcast::<ContextError>() {
                        Ok(ctx_err) => {
                            if ctx_err
                                .cause
                                .downcast_ref::<wgpu_core::command::CommandEncoderError>()
                                .is_some()
                            {
                                // Actual command encoder errors never carry any meaningful
                                // information.
                                return;
                            }
                            let ctx_err = WrappedContextError(ctx_err);
                            if !self.errors.lock().insert(ctx_err) {
                                return;
                            }
                            re_log::error!(
                                frame_nr = self.frame_nr.load(std::sync::atomic::Ordering::Relaxed),
                                %description,
                                "WGPU error",
                            );
                        }
                        Err(err) => panic!("{err}"),
                    };
                }
            }
        }
    }
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
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
    err_tracker: ErrorTracker,
    state: AppState,

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

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        let err_tracker = {
            let err_tracker = ErrorTracker::default();
            device.on_uncaptured_error({
                let err_tracker = err_tracker.clone();
                move |err| err_tracker.handle_error(err)
            });
            err_tracker
        };

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
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
            err_tracker,
            re_ctx,
            state: AppState::new(),
        })
    }

    fn run(mut self) {
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
        let mut clear_countdown = usize::MAX;

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
                    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                    // native debug build
                    self.err_tracker.tick();

                    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
                    let frame = match self.surface.get_current_texture() {
                        Ok(frame) => {
                            clear_countdown = clear_countdown.saturating_sub(1);
                            if clear_countdown == 0 {
                                clear_countdown = usize::MAX;
                                self.err_tracker.clear();
                            }
                            frame
                        }
                        Err(wgpu::SurfaceError::Outdated) => {
                            // TODO: explain
                            clear_countdown = 10;
                            self.surface.configure(&self.device, &self.surface_config);
                            return;
                        }
                        Err(err) => {
                            re_log::warn!(%err, "dropped frame");
                            return;
                        }
                    };
                    #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
                    let frame = self
                        .surface
                        .get_current_texture()
                        .expect("failed to acquire next swap chain texture");

                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder =
                        self.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: "composite_encoder".into(),
                            });

                    let view_builder = draw_view(
                        &self.state,
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
                    let time_passed = Instant::now() - self.state.time.last_draw_time;
                    self.state.time.last_draw_time = current_time;

                    // TODO(andreas): Display a median over n frames and while we're on it also stddev thereof.
                    // Repeatedly setting the title causes issues on some platforms
                    // Do it only every second.
                    let time_until_next_report =
                        1.0 - self.state.time.seconds_since_startup().fract();
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

struct AppState {
    time: Time,

    // Want to have a large cloud of random points, but doing rng for all of them every frame is too slow
    random_points: Vec<PointCloudPoint>,
}

impl AppState {
    fn new() -> Self {
        let mut rnd = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(42);
        let random_point_range = -2.0_f32..2.0_f32;
        let random_points = (0..500000)
            .map(|_| PointCloudPoint {
                position: glam::vec3(
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                ),
                radius: rnd.gen_range(0.005..0.025),
                srgb_color: [rnd.gen(), rnd.gen(), rnd.gen(), 255],
            })
            .collect::<Vec<_>>();

        Self {
            time: Time {
                start_time: Instant::now(),
                last_draw_time: Instant::now(),
            },
            random_points,
        }
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
        tracing_subscriber::fmt::init();

        // Set size to a common physical resolution as a comparable start-up default.
        window.set_inner_size(winit::dpi::PhysicalSize {
            width: 1920,
            height: 1080,
        });

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
