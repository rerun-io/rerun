//! Example framework

use std::sync::Arc;

use anyhow::Context as _;
use web_time::Instant;

use re_renderer::{
    config::{supported_backends, HardwareTier, RenderContextConfig},
    view_builder::ViewBuilder,
    RenderContext,
};

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub struct ViewDrawResult {
    pub view_builder: ViewBuilder,
    pub command_buffer: wgpu::CommandBuffer,
    pub target_location: glam::Vec2,
}

pub trait Example {
    fn title() -> &'static str;

    fn new(re_ctx: &mut RenderContext) -> Self;

    fn draw(
        &mut self,
        re_ctx: &mut RenderContext,
        resolution: [u32; 2],
        time: &Time,
        pixels_from_point: f32,
    ) -> Vec<ViewDrawResult>;

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}

    fn on_cursor_moved(&mut self, _position_in_pixel: glam::UVec2) {}
}

#[allow(dead_code)]
pub struct SplitView {
    pub target_location: glam::Vec2,
    pub resolution_in_pixel: [u32; 2],
}

#[allow(dead_code)]
pub fn split_resolution(
    resolution: [u32; 2],
    num_rows: usize,
    num_cols: usize,
) -> impl Iterator<Item = SplitView> {
    let total_width = resolution[0] as f32;
    let total_height = resolution[1] as f32;
    let width = (total_width / num_cols as f32).floor();
    let height = (total_height / num_rows as f32).floor();
    (0..num_rows)
        .flat_map(move |row| (0..num_cols).map(move |col| (row, col)))
        .map(move |(row, col)| {
            // very quick'n'dirty (uneven) borders
            let y = f32::clamp(row as f32 * height + 2.0, 2.0, total_height - 2.0).floor();
            let x = f32::clamp(col as f32 * width + 2.0, 2.0, total_width - 2.0).floor();
            SplitView {
                target_location: glam::vec2(x, y),
                resolution_in_pixel: [(width - 4.0) as u32, (height - 4.0) as u32],
            }
        })
}

pub struct Time {
    start_time: Instant,
    last_draw_time: Instant,
    pub last_frame_duration: web_time::Duration,
}

impl Time {
    pub fn seconds_since_startup(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }
}

struct Application<E> {
    event_loop: EventLoop<()>,
    window: Window,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    time: Time,

    example: E,

    re_ctx: RenderContext,
}

// Same as egui_wgpu::preferred_framebuffer_format
fn preferred_framebuffer_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    for &format in formats {
        if matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        ) {
            return format;
        }
    }
    formats[0] // take the first
}

impl<E: Example + 'static> Application<E> {
    async fn new(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: supported_backends(),
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
        });
        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("failed to find an appropriate adapter")?;

        let hardware_tier = HardwareTier::from_adapter(&adapter);
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
        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let swapchain_format =
            preferred_framebuffer_format(&surface.get_capabilities(&adapter).formats);
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
            view_formats: [swapchain_format].to_vec(),
        };
        surface.configure(&device, &surface_config);

        let mut re_ctx = RenderContext::new(
            &adapter,
            device,
            queue,
            RenderContextConfig {
                output_format_color: swapchain_format,
                hardware_tier,
            },
        );

        let example = E::new(&mut re_ctx);

        Ok(Self {
            event_loop,
            window,
            surface,
            surface_config,
            re_ctx,
            time: Time {
                start_time: Instant::now(),
                last_draw_time: Instant::now(),
                last_frame_duration: web_time::Duration::from_secs(0),
            },

            example,
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
                    self.surface
                        .configure(&self.re_ctx.device, &self.surface_config);
                    self.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { input, .. },
                    ..
                } => self.example.on_keyboard_input(input),
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => self
                    .example
                    // Don't round the position: The entire range from 0 to excluding 1 should fall into pixel coordinate 0!
                    .on_cursor_moved(glam::uvec2(position.x as u32, position.y as u32)),
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
                    self.surface
                        .configure(&self.re_ctx.device, &self.surface_config);
                    self.window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    self.re_ctx.begin_frame();

                    // native debug build
                    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                    let frame = match self.surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(wgpu::SurfaceError::Outdated) => {
                            // We haven't been able to present anything to the swapchain for
                            // a while, because the pipeline is poisoned.
                            // Recreate a sane surface to restart the cycle and see if the
                            // user has fixed the issue.
                            self.surface
                                .configure(&self.re_ctx.device, &self.surface_config);
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

                    let draw_results = self.example.draw(
                        &mut self.re_ctx,
                        [self.surface_config.width, self.surface_config.height],
                        &self.time,
                        self.window.scale_factor() as f32,
                    );

                    let mut composite_cmd_encoder = self.re_ctx.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: "composite_encoder".into(),
                        },
                    );

                    {
                        let mut composite_pass =
                            composite_cmd_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: true,
                                    },
                                })],
                                depth_stencil_attachment: None,
                            });

                        for draw_result in &draw_results {
                            draw_result.view_builder.composite(
                                &self.re_ctx,
                                &mut composite_pass,
                                draw_result.target_location,
                            );
                        }
                    };

                    self.re_ctx.before_submit();
                    self.re_ctx.queue.submit(
                        draw_results
                            .into_iter()
                            .map(|d| d.command_buffer)
                            .chain(std::iter::once(composite_cmd_encoder.finish())),
                    );
                    frame.present();

                    // Note that this measures time spent on CPU, not GPU
                    // However, iff we're GPU bound (likely for this sample) and GPU times are somewhat stable,
                    // we eventually end up waiting for GPU in `get_current_texture`
                    // (wgpu has a swap chain with a limited amount of buffers, the exact count is dependent on `present_mode` and backend!).
                    // It's important to keep in mind that depending on the `present_mode`, the GPU might be waiting on the screen in turn.
                    let current_time = Instant::now();
                    let time_passed = current_time - self.time.last_draw_time;
                    self.time.last_draw_time = current_time;
                    self.time.last_frame_duration = time_passed;

                    // TODO(andreas): Display a median over n frames and while we're on it also stddev thereof.
                    // Do it only every second.
                    let time_until_next_report = 1.0 - self.time.seconds_since_startup().fract();
                    if time_until_next_report - time_passed.as_secs_f32() < 0.0 {
                        let time_info_str = format!(
                            "{:.2} ms ({:.2} fps)",
                            time_passed.as_secs_f32() * 1000.0,
                            1.0 / time_passed.as_secs_f32()
                        );
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

#[allow(dead_code)]
pub fn load_rerun_mesh(re_ctx: &mut RenderContext) -> Vec<re_renderer::renderer::MeshInstance> {
    let reader = std::io::Cursor::new(include_bytes!("rerun.obj.zip"));
    let mut zip = zip::ZipArchive::new(reader).unwrap();
    let mut zipped_obj = zip.by_name("rerun.obj").unwrap();
    let mut obj_data = Vec::new();
    std::io::Read::read_to_end(&mut zipped_obj, &mut obj_data).unwrap();
    re_renderer::importer::obj::load_obj_from_buffer(
        &obj_data,
        re_renderer::resource_managers::ResourceLifeTime::LongLived,
        re_ctx,
    )
    .unwrap()
}

async fn run<E: Example + 'static>(event_loop: EventLoop<()>, window: Window) {
    let app = Application::<E>::new(event_loop, window).await.unwrap();
    app.run();
}

pub fn start<E: Example + 'static>() {
    let event_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(format!("re_renderer sample - {}", E::title()))
        .build(&event_loop)
        .unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        re_log::setup_native_logging();

        // Set size to a common physical resolution as a comparable start-up default.
        window.set_inner_size(winit::dpi::PhysicalSize {
            width: 1920,
            height: 1080,
        });

        pollster::block_on(run::<E>(event_loop, window));
    }

    #[cfg(target_arch = "wasm32")]
    {
        // Make sure panics are logged using `console.error`.
        console_error_panic_hook::set_once();

        re_log::setup_web_logging();

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
        wasm_bindgen_futures::spawn_local(run::<E>(event_loop, window));
    }
}

// This allows treating the framework as a standalone example,
// thus avoiding listing the example names in `Cargo.toml`.
#[allow(dead_code)]
fn main() {}
