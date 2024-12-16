//! Example framework

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]
use std::sync::Arc;

use anyhow::Context as _;
use web_time::Instant;

use re_renderer::{
    config::{supported_backends, DeviceCaps},
    view_builder::ViewBuilder,
    RenderContext,
};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

pub struct ViewDrawResult {
    pub view_builder: ViewBuilder,
    pub command_buffer: wgpu::CommandBuffer,
    pub target_location: glam::Vec2,
}

pub trait Example {
    fn title() -> &'static str;

    fn new(re_ctx: &RenderContext) -> Self;

    fn draw(
        &mut self,
        re_ctx: &RenderContext,
        resolution: [u32; 2],
        time: &Time,
        pixels_per_point: f32,
    ) -> anyhow::Result<Vec<ViewDrawResult>>;

    fn on_key_event(&mut self, _event: winit::event::KeyEvent) {}

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
    window: Arc<Window>,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface<'static>,
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
    async fn new(window: Window) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: supported_backends(),
            flags: wgpu::InstanceFlags::default()
                // Run without validation layers, they can be annoying on shader reload depending on the backend.
                .intersection(wgpu::InstanceFlags::VALIDATION.complement()),
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("failed to find an appropriate adapter")?;

        let device_caps = DeviceCaps::from_adapter(&adapter)?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: device_caps.limits(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .context("failed to create device")?;
        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let output_format_color =
            preferred_framebuffer_format(&surface.get_capabilities(&adapter).formats);

        let re_ctx = RenderContext::new(&adapter, device, queue, output_format_color)
            .map_err(|err| anyhow::format_err!("{err}"))?;

        let example = E::new(&re_ctx);

        let app = Self {
            window,
            adapter,
            surface,
            re_ctx,
            time: Time {
                start_time: Instant::now(),
                last_draw_time: Instant::now(),
                last_frame_duration: web_time::Duration::from_secs(0),
            },

            example,
        };

        app.configure_surface(app.window.inner_size());
        Ok(app)
    }

    fn configure_surface(&self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        let surface_config = wgpu::SurfaceConfiguration {
            // Use AutoNoVSync if you want to do quick perf checking.
            // Otherwise, use AutoVsync is much more pleasant to use - laptops don't heat up and desktop won't have annoying coil whine on trivial examples.
            present_mode: wgpu::PresentMode::AutoVsync,
            format: self.re_ctx.output_format_color(),
            view_formats: vec![self.re_ctx.output_format_color()],
            ..self
                .surface
                .get_default_config(&self.adapter, size.width, size.height)
                .expect("The surface isn't supported by this adapter")
        };
        self.surface.configure(&self.re_ctx.device, &surface_config);
        self.window.request_redraw();
    }

    fn on_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.configure_surface(size);
            }

            WindowEvent::KeyboardInput { event, .. } => self.example.on_key_event(event),

            WindowEvent::CursorMoved { position, .. } => self
                .example
                // Don't round the position: The entire range from 0 to excluding 1 should fall into pixel coordinate 0!
                .on_cursor_moved(glam::uvec2(position.x as u32, position.y as u32)),

            WindowEvent::RedrawRequested => {
                self.re_ctx.begin_frame();

                // native debug build
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                let frame = match self.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Outdated) => {
                        // We haven't been able to present anything to the swapchain for
                        // a while, because the pipeline is poisoned.
                        // Recreate a sane surface to restart the cycle and see if the
                        // user has fixed the issue.
                        self.configure_surface(self.window.inner_size());
                        return;
                    }
                    Err(err) => {
                        re_log::warn!("Dropped frame: {err}");
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

                let draw_results = self
                    .example
                    .draw(
                        &self.re_ctx,
                        [frame.texture.width(), frame.texture.height()],
                        &self.time,
                        self.window.scale_factor() as f32,
                    )
                    .expect("Failed to draw example");

                let mut composite_cmd_encoder =
                    self.re_ctx
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: "composite_encoder".into(),
                        });

                {
                    let mut composite_pass =
                        composite_cmd_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                    for draw_result in &draw_results {
                        composite_pass.set_viewport(
                            draw_result.target_location.x,
                            draw_result.target_location.y,
                            draw_result.view_builder.resolution_in_pixel()[0] as f32,
                            draw_result.view_builder.resolution_in_pixel()[1] as f32,
                            0.0,
                            1.0,
                        );
                        draw_result
                            .view_builder
                            .composite(&self.re_ctx, &mut composite_pass);
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

                self.window.request_redraw(); // Busy-painting
            }

            _ => {}
        }
    }
}

#[allow(dead_code)]
pub fn load_rerun_mesh(
    re_ctx: &RenderContext,
) -> anyhow::Result<Vec<re_renderer::renderer::GpuMeshInstance>> {
    let reader = std::io::Cursor::new(include_bytes!("../../../tests/assets/rerun.obj.zip"));
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut zipped_obj = zip.by_name("rerun.obj")?;
    let mut obj_data = Vec::new();
    std::io::Read::read_to_end(&mut zipped_obj, &mut obj_data)?;
    Ok(
        re_renderer::importer::obj::load_obj_from_buffer(&obj_data, re_ctx)?
            .into_gpu_meshes(re_ctx)?,
    )
}

struct WrapApp<E: Example + 'static> {
    app: Option<Application<E>>,
}

impl<E: Example + 'static> ApplicationHandler for WrapApp<E> {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let attributes = winit::window::WindowAttributes::default()
                .with_title(format!("re_renderer sample - {}", E::title()))
                .with_inner_size(winit::dpi::PhysicalSize {
                    width: 1920,
                    height: 1080,
                });
            let window = _event_loop
                .create_window(attributes)
                .expect("Failed to create window");
            self.app = Some(pollster::block_on(Application::new(window)).unwrap());
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            event_loop.exit();
        }

        if let Some(app) = &mut self.app {
            app.on_window_event(event);
        }
    }
}

pub fn start<E: Example + 'static>() {
    re_log::setup_logging();

    let event_loop = EventLoop::new().unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut wrap_app = WrapApp::<E> { app: None };
        event_loop.run_app(&mut wrap_app).unwrap();
    }

    #[cfg(target_arch = "wasm32")]
    {
        async fn run<E: Example + 'static>(event_loop: EventLoop<()>, window: Window) {
            let app = Application::<E>::new(window).await.unwrap();
            let mut wrap_app = WrapApp::<E> { app: Some(app) };
            event_loop.run_app(&mut wrap_app).unwrap();
        }

        // Make sure panics are logged using `console.error`.
        console_error_panic_hook::set_once();

        re_log::setup_logging();

        let window = winit::window::WindowAttributes::default()
            .with_title(format!("re_renderer sample - {}", E::title()))
            .with_inner_size(winit::dpi::PhysicalSize {
                width: 1920,
                height: 1080,
            });

        // TODO(emilk): port this to the winit 0.30 API, using maybe https://docs.rs/winit/latest/winit/platform/web/trait.EventLoopExtWebSys.html ?
        #[allow(deprecated)]
        let window = event_loop.create_window(window).unwrap();

        use winit::platform::web::WindowExtWebSys;
        let canvas = window.canvas().expect("Couldn't get canvas");
        canvas.style().set_css_text("height: 100%; width: 100%;");
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| body.append_child(&canvas).ok())
            .expect("couldn't append canvas to document body");
        wasm_bindgen_futures::spawn_local(run::<E>(event_loop, window));
    }
}

// This allows treating the framework as a standalone example,
// thus avoiding listing the example names in `Cargo.toml`.
#[allow(dead_code)]
fn main() {}
