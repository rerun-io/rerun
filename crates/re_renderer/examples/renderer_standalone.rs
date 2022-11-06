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

use std::{f32::consts::TAU, io::Read};

use anyhow::Context as _;
use glam::Vec3;
use instant::Instant;
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;
use re_renderer::{
    config::{supported_backends, HardwareTier, RenderContextConfig},
    mesh_manager::{GpuMeshHandle, MeshManager},
    renderer::*,
    view_builder::{TargetConfiguration, ViewBuilder},
    DebugLabel, *,
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

fn split_resolution(
    resolution: [u32; 2],
    nb_rows: usize,
    nb_cols: usize,
) -> impl Iterator<Item = ((f32, f32), (f32, f32))> {
    let total_width = resolution[0] as f32;
    let total_height = resolution[1] as f32;
    let width = total_width / nb_cols as f32;
    let height = total_height / nb_rows as f32;
    (0..nb_rows)
        .flat_map(move |row| (0..nb_cols).map(move |col| (row, col)))
        .map(move |(row, col)| {
            // very quick'n'dirty (uneven) borders
            let y = f32::clamp(row as f32 * height + 2.0, 2.0, total_height - 2.0);
            let x = f32::clamp(col as f32 * width + 2.0, 2.0, total_width - 2.0);
            ((x, y), (width - 4.0, height - 4.0))
        })
}

fn draw_views(
    state: &AppState,
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    backbuffer_view: &wgpu::TextureView,
    resolution: [u32; 2],
) -> impl Iterator<Item = wgpu::CommandBuffer> {
    // Rotate camera around the center at a distance of 5, looking down at 45 deg
    let seconds_since_startup = state.time.seconds_since_startup();
    let pos = Vec3::new(
        seconds_since_startup.sin(),
        0.5,
        seconds_since_startup.cos(),
    ) * 10.0;
    let view_from_world = IsoTransform::look_at_rh(pos, Vec3::ZERO, Vec3::Y).unwrap();
    let mut target_cfg = TargetConfiguration {
        resolution_in_pixel: resolution,
        origin_in_pixel: [0, 0],
        view_from_world,
        fov_y: 70.0 * TAU / 360.0,
        near_plane_distance: 0.01,
        target_identifier: 0,
    };

    let triangle = TestTriangleDrawable::new(re_ctx, device);
    let skybox = GenericSkyboxDrawable::new(re_ctx, device);
    let lines = build_lines(re_ctx, device, queue, seconds_since_startup);
    let point_cloud = PointCloudDrawable::new(re_ctx, device, queue, &state.random_points).unwrap();
    let meshes = build_meshes(re_ctx, device, queue, &state.meshes, seconds_since_startup);

    let splits = split_resolution(resolution, 2, 2).collect::<Vec<_>>();

    // Using a macro here because `Drawable` isn't object safe and a closure cannot be
    // generic over its input type.
    #[rustfmt::skip]
    macro_rules! draw {
        ($name:ident @ split #$n:expr) => {{
            let ((x, y), (width, height)) = splits[$n];
            target_cfg.resolution_in_pixel = [width as u32, height as u32];
            target_cfg.origin_in_pixel = [x as u32, y as u32];
            draw_view(re_ctx, device, queue, &target_cfg,
                      &stringify!($name).into(), &skybox, &$name)
        }};
    }

    let view_builders = [
        draw!(triangle @ split #0),
        draw!(lines @ split #1),
        draw!(meshes @ split #2),
        draw!(point_cloud @ split #3),
    ];

    let mut composite_cmd_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: "composite_encoder".into(),
        });

    let view_cmd_buffers = {
        let mut composite_pass =
            composite_cmd_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: backbuffer_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

        view_builders
            .into_iter()
            .map(|(vb, cmd_buf)| {
                vb.composite(re_ctx, &mut composite_pass)
                    .expect("Failed to composite view main surface");
                cmd_buf
            })
            .collect::<Vec<_>>() // So we don't hold a reference to the render pass!

        // drop the pass so we can finish() the main encoder!
    };

    view_cmd_buffers
        .into_iter()
        .chain(std::iter::once(composite_cmd_encoder.finish()))
}

fn draw_view<'a, D: 'static + Drawable + Sync + Send + Clone>(
    re_ctx: &'a mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target_cfg: &TargetConfiguration,
    label: &DebugLabel,
    skybox: &GenericSkyboxDrawable,
    drawable: &D,
) -> (ViewBuilder, wgpu::CommandBuffer) {
    let mut view_builder = ViewBuilder::default();

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: label.get() });

    view_builder
        .setup_view(re_ctx, device, queue, target_cfg)
        .unwrap()
        .queue_draw(skybox)
        .queue_draw(drawable)
        .draw(re_ctx, &mut encoder)
        .unwrap();

    (view_builder, encoder.finish())
}

fn build_meshes(
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh_handles: &[GpuMeshHandle],
    seconds_since_startup: f32,
) -> MeshDrawable {
    let mesh_instances = lorenz_points(10.0)
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            mesh_handles.iter().map(move |mesh| MeshInstance {
                mesh: *mesh,
                world_from_mesh: macaw::Conformal3::from_scale_rotation_translation(
                    0.025 + (i % 10) as f32 * 0.01,
                    glam::Quat::from_rotation_y(i as f32 + seconds_since_startup * 5.0),
                    *p,
                ),
            })
        })
        .collect_vec();
    MeshDrawable::new(re_ctx, device, queue, &mesh_instances).unwrap()
}

fn lorenz_points(seconds_since_startup: f32) -> Vec<glam::Vec3> {
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
    .collect()
}

fn build_lines(
    re_ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    seconds_since_startup: f32,
) -> LineDrawable {
    // Calculate some points that look nice for an animated line.
    let lorenz_points = lorenz_points(seconds_since_startup);
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
                flags: LineStripFlags::empty(),
            },
            // Green Zig-Zag
            LineStrip {
                points: vec![
                    glam::vec3(0.0, -1.0, 0.0),
                    glam::vec3(1.0, 0.0, 0.0),
                    glam::vec3(2.0, -1.0, 0.0),
                    glam::vec3(3.0, 0.0, 0.0),
                ],
                radius: 0.1,
                color: [50, 255, 50, 255],
                flags: LineStripFlags::CAP_END_TRIANGLE,
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
                flags: LineStripFlags::CAP_END_TRIANGLE,
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
    state: AppState,

    pub re_ctx: RenderContext,
}

impl Application {
    async fn new(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(supported_backends());
        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("failed to find an appropriate adapter")?;

        let hardware_tier = HardwareTier::Web;
        hardware_tier.check_downlevel_capabilities(&adapter.get_downlevel_capabilities())?;
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

        let mut re_ctx = RenderContext::new(
            &device,
            &queue,
            RenderContextConfig {
                output_format_color: swapchain_format,
                hardware_tier,
            },
        );

        let state = AppState::new(&mut re_ctx, &device, &queue);

        Ok(Self {
            event_loop,
            window,
            device,
            queue,
            surface,
            surface_config,
            re_ctx,
            state,
        })
    }

    fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            // Keep our example busy.
            // Not how one should generally do it, but great for animated content and
            // checking on perf.
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
                    // native debug build
                    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                    let frame = match self.surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(wgpu::SurfaceError::Outdated) => {
                            // We haven't been able to present anything to the swapchain for
                            // a while, because the pipeline is poisoned.
                            // Recreate a sane surface to restart the cycle and see if the
                            // user has fixed the issue.
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

                    let cmd_buffers = draw_views(
                        &self.state,
                        &mut self.re_ctx,
                        &self.device,
                        &self.queue,
                        &view,
                        [self.surface_config.width, self.surface_config.height],
                    );

                    self.queue.submit(cmd_buffers);
                    frame.present();

                    self.re_ctx.frame_maintenance(&self.device);

                    // Note that this measures time spent on CPU, not GPU
                    // However, iff we're GPU bound (likely for this sample) and GPU times are somewhat stable,
                    // we eventually end up waiting for GPU in `get_current_texture`
                    // (wgpu has a swap chain with a limited amount of buffers, the exact count is dependent on `present_mode` and backend!).
                    // It's important to keep in mind that depending on the `present_mode`, the GPU might be waiting on the screen in turn.
                    let current_time = Instant::now();
                    let time_passed = current_time - self.state.time.last_draw_time;
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
                        re_log::info!("{time_info_str}");
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
        self.start_time.elapsed().as_secs_f32()
    }
}

struct AppState {
    time: Time,

    /// Lazily loaded mesh.
    meshes: Vec<GpuMeshHandle>,

    // Want to have a large cloud of random points, but doing rng for all of them every frame is too slow
    random_points: Vec<PointCloudPoint>,
}

impl AppState {
    fn new(re_ctx: &mut RenderContext, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
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
            .collect_vec();

        let meshes = {
            let reader = std::io::Cursor::new(include_bytes!("rerun.obj.zip"));
            let mut zip = zip::ZipArchive::new(reader).unwrap();
            let mut zipped_obj = zip.by_name("rerun.obj").unwrap();
            let mut obj_data = Vec::new();
            zipped_obj.read_to_end(&mut obj_data).unwrap();
            importer::obj::load_obj_from_buffer(&obj_data)
                .unwrap()
                .meshes
                .iter()
                .map(|mesh_data| {
                    MeshManager::new_long_lived_mesh(re_ctx, device, queue, mesh_data).unwrap()
                })
                .collect()
        };

        Self {
            time: Time {
                start_time: Instant::now(),
                last_draw_time: Instant::now(),
            },
            meshes,
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
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info,wgpu_core=off");
        }
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
        // Make sure panics are logged using `console.error`.
        console_error_panic_hook::set_once();
        // Redirect tracing to console.log and friends:
        tracing_wasm::set_as_global_default();

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
