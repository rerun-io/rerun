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

use anyhow::Context as _;
use macaw::IsoTransform;
use re_renderer::context::{RenderContext, RenderContextConfig};
use re_renderer::frame_builder::{FrameBuilder, TargetConfiguration};
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
            let mut frame_builder = FrameBuilder::new();

            let target_cfg = TargetConfiguration {
                resolution_in_pixel: resolution,
                world_from_view: IsoTransform::IDENTITY,
                fov_y: 90.0,
                near_plane_distance: 0.0,
                target_identifier: 0,
            };

            let re_ctx = user_data.get_mut::<RenderContext>().unwrap();
            frame_builder
                .setup_target(re_ctx, device, queue, &target_cfg)
                .unwrap()
                .test_triangle(re_ctx, device)
                .generic_skybox(re_ctx, device)
                .draw(re_ctx, encoder)
                .unwrap();

            frame_builder
        },
        // Setting up the `draw` callback, which will be called once per frame with the
        // renderpass drawing onto the swapchain.
        {
            |user_data, rpass, frame_builder: FrameBuilder| {
                let re_ctx = user_data.get::<RenderContext>().unwrap();
                frame_builder.finish(re_ctx, rpass).unwrap();
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
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface.get_supported_alpha_modes(&adapter)[0],
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
            *control_flow = ControlFlow::Wait;

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
                        [self.surface_config.height, self.surface_config.width],
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
    let window = winit::window::Window::new(&event_loop).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
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
