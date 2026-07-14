//! Deterministic benchmark for large generic point clouds.
//!
//! CPU point cloud data is set up only once, but per-frame GPU upload is part of the benchmark.
//!
//! ## Usage
//!
//! Native:
//! ```sh
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark --frames 200
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark --points 16m --camera near --size world
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark --points 16m --camera away --size world
//! cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --experiment sorted-front-to-back
//! ```
//!
//! Keys:
//! * `1`/`2`/`3`: point count tier
//! * `C`: cycle fixed camera distance
//! * `S`: toggle screen-space/world-space point size
//! * `E`: cycle experiment
//! * `O`: toggle occluder rectangle experiment
//!
//! Command-line arguments:
//! * `--help`: print command-line help
//! * `--benchmark`: run all selected point/camera/size combinations at fixed 1920x1080 resolution
//! * `--frames <N>`: measured frames per combination in benchmark mode
//! * `--points <1m|4m|16m>`: select initial point count, or run only matching point count in benchmark mode
//! * `--camera <near|medium|far|away>`: select initial camera, or run only matching camera in benchmark mode
//! * `--size <screen|world>`: select initial point-size mode, or run only matching point-size mode in benchmark mode
//! * `--experiment <none|occluder-rectangle|sorted-front-to-back>`: select experiment

#![expect(clippy::disallowed_methods)] // allow hardcoded colors

use clap::Parser as _;
use macaw::IsoTransform;
use re_renderer::mesh::GpuMesh;
use re_renderer::renderer::gpu_data::PositionRadius;
use re_renderer::renderer::{GenericSkyboxDrawData, GpuMeshInstance, MeshDrawData};
use re_renderer::view_builder::{self, Projection, ViewBuilder};
use re_renderer::{Color32, PointCloudBuilder, ShapeBuilder, Size};
use std::io::Write as _;
use std::sync::{Arc, OnceLock};
use winit::event::ElementState;
use winit::keyboard::Key;

mod framework;

const POINT_COUNTS: [usize; 3] = [1_000_000, 4_000_000, 16_000_000];
const CAMERA_DISTANCES: [f32; 3] = [1.2, 20.0, 100.0];
const CAMERA_LOOKING_AWAY_INDEX: usize = CAMERA_DISTANCES.len();
const CAMERA_COUNT: usize = CAMERA_DISTANCES.len() + 1;
const DEFAULT_BENCHMARK_FRAMES: u32 = 50;
const BENCHMARK_RESOLUTION: [u32; 2] = [1920, 1080];
const BENCHMARK_WARMUP_FRAMES: u32 = 3;

static COMMAND_LINE: OnceLock<CommandLine> = OnceLock::new();

#[derive(Clone, Debug, clap::Parser)]
#[command(
    name = "point_cloud_bench",
    about = "Deterministic benchmark for large generic point clouds.",
    after_help = "Interactive keys:\n  1, 2, 3                  Select point count tier\n  C                        Cycle camera: near, medium, far, away\n  S                        Toggle point size mode: screen, world\n  E                        Cycle experiment: none, occluder-rectangle, sorted-front-to-back\n  O                        Toggle occluder rectangle experiment\n\nBenchmark notes:\n  Warmup frames ignored per combination: 3\n  CPU point cloud generation is excluded from measured frames.\n  Each measured frame rebuilds PointCloudDrawData and uploads GPU data.\n  Benchmark mode sweeps selected point/camera/size combinations for the selected experiment.\n\nExamples:\n  cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark\n  cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark --frames 200\n  cargo run -p re_renderer_examples --bin point_cloud_bench --release -- --benchmark --points 16m --camera near --size world --experiment sorted-front-to-back"
)]
struct CommandLine {
    /// Run benchmark suite at fixed 1920x1080 using Immediate present mode.
    #[arg(long = "benchmark")]
    run_benchmark: bool,

    /// Measured frames per combination.
    #[arg(long = "frames", default_value_t = DEFAULT_BENCHMARK_FRAMES, value_name = "N")]
    benchmark_frames: u32,

    /// Select point count, or run only matching point count in benchmark mode.
    #[arg(long = "points", value_name = "1m|4m|16m", value_parser = parse_points_arg)]
    selected_tier: Option<usize>,

    /// Select camera, or run only matching camera in benchmark mode.
    #[arg(long = "camera", value_name = "near|medium|far|away", value_parser = parse_camera_arg)]
    selected_camera: Option<usize>,

    /// Select point-size mode, or run only matching point-size mode in benchmark mode.
    #[arg(long = "size", value_name = "screen|world", value_parser = parse_size_mode_arg)]
    selected_size_mode: Option<PointSizeMode>,

    /// Select experiment.
    #[arg(long = "experiment", value_name = "none|occluder-rectangle|sorted-front-to-back", value_parser = parse_experiment_arg)]
    selected_experiment: Option<Experiment>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PointSizeMode {
    ScreenSpace,
    WorldSpace,
}

impl PointSizeMode {
    const ALL: [Self; 2] = [Self::ScreenSpace, Self::WorldSpace];

    fn toggle(&mut self) {
        *self = match self {
            Self::ScreenSpace => Self::WorldSpace,
            Self::WorldSpace => Self::ScreenSpace,
        };
    }

    fn base_size(self) -> Size {
        match self {
            Self::ScreenSpace => Size::new_ui_points(2.5),
            Self::WorldSpace => Size::new_scene_units(0.018),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ScreenSpace => "screen",
            Self::WorldSpace => "world",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Experiment {
    None,
    OccluderRectangle,
    SortedFrontToBack,
}

impl Experiment {
    fn next(self) -> Self {
        match self {
            Self::None => Self::OccluderRectangle,
            Self::OccluderRectangle => Self::SortedFrontToBack,
            Self::SortedFrontToBack => Self::None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OccluderRectangle => "occluder-rectangle",
            Self::SortedFrontToBack => "sorted-front-to-back",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Config {
    tier: usize,
    camera: usize,
    size_mode: PointSizeMode,
    experiment: Experiment,
}

struct CpuCloud {
    key: Config,
    positions_and_radii: Vec<PositionRadius>,
    colors: Vec<Color32>,
}

struct BenchmarkRun {
    configs: Vec<Config>,
    current: usize,
    frames_per_config: u32,
    frames_seen: u32,
    accumulated: web_time::Duration,
    frame_times: Vec<web_time::Duration>,
    results: Vec<BenchmarkResult>,
    warmup_frames_seen: u32,
    last_frame_finished_at: Option<web_time::Instant>,
}

struct BenchmarkResult {
    config: Config,
    total: web_time::Duration,
    average: web_time::Duration,
    frame_times: Vec<web_time::Duration>,
}

struct PointCloudBench {
    config: Config,
    benchmark_frames: u32,
    current_cloud: Option<CpuCloud>,
    benchmark: Option<BenchmarkRun>,
    exit_after_benchmark: bool,
    adapter_info: Option<String>,
}

impl PointCloudBench {
    fn log_controls(&self) {
        re_log::info!(
            "Large point cloud benchmark controls: 1/2/3 tiers, C camera, S size mode, E experiment, O occluder. Benchmark: --benchmark --frames <N> [--points <1m|4m|16m>] [--camera <near|medium|far|away>] [--size <screen|world>] [--experiment <none|occluder-rectangle|sorted-front-to-back>]. Current: {}",
            Self::config_label(self.config)
        );
    }

    fn config_label(config: Config) -> String {
        format!(
            "{} pts, {} camera, {} size, {} experiment",
            POINT_COUNTS[config.tier],
            camera_label(config.camera),
            config.size_mode.label(),
            config.experiment.label()
        )
    }

    fn set_config(&mut self, config: Config) {
        self.config = config;
        re_log::info!("Config: {}", Self::config_label(config));
    }

    fn cycle_experiment(&mut self) {
        self.set_config(Config {
            experiment: self.config.experiment.next(),
            ..self.config
        });
    }

    fn toggle_occluder_rectangle(&mut self) {
        self.set_config(Config {
            experiment: if self.config.experiment == Experiment::OccluderRectangle {
                Experiment::None
            } else {
                Experiment::OccluderRectangle
            },
            ..self.config
        });
    }

    fn ensure_current_cpu_cloud(&mut self, config: Config) {
        if self
            .current_cloud
            .as_ref()
            .is_some_and(|cloud| cloud.key == config)
        {
            return;
        }

        let point_count = POINT_COUNTS[config.tier];
        let radius = config.size_mode.base_size();
        re_log::info!(
            "Generating deterministic dense point cloud CPU data: {} points, {} size, {} experiment",
            point_count,
            config.size_mode.label(),
            config.experiment.label()
        );

        let mut positions_and_radii = Vec::with_capacity(point_count);
        let mut colors = Vec::with_capacity(point_count);
        generate_dense_cloud(point_count, radius, &mut positions_and_radii, &mut colors);
        if config.experiment == Experiment::SortedFrontToBack {
            sort_front_to_back(
                &mut positions_and_radii,
                &mut colors,
                camera_position(config.camera),
                camera_target(config.camera),
            );
        }
        self.current_cloud = Some(CpuCloud {
            key: config,
            positions_and_radii,
            colors,
        });
    }

    fn point_cloud_draw_data(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
    ) -> anyhow::Result<re_renderer::renderer::PointCloudDrawData> {
        self.ensure_current_cpu_cloud(self.config);
        let cloud = self
            .current_cloud
            .as_ref()
            .expect("CPU point cloud should have been generated");

        let mut builder = PointCloudBuilder::new(re_ctx);
        builder.reserve(cloud.positions_and_radii.len())?;
        builder
            .batch(format!(
                "dense point cloud ({})",
                cloud.positions_and_radii.len()
            ))
            .add_points(&cloud.positions_and_radii, &cloud.colors, &[]);
        Ok(builder.into_draw_data()?)
    }

    fn current_camera_position(&self) -> glam::Vec3 {
        camera_position(self.config.camera)
    }

    fn current_camera_target(&self) -> glam::Vec3 {
        camera_target(self.config.camera)
    }

    fn occluder_draw_data(re_ctx: &re_renderer::RenderContext) -> anyhow::Result<MeshDrawData> {
        let mut shape_builder = ShapeBuilder::default();
        shape_builder.add_convex_polygon(&[
            glam::vec2(-1.0, -1.0),
            glam::vec2(1.0, -1.0),
            glam::vec2(1.0, 1.0),
            glam::vec2(-1.0, 1.0),
        ]);

        let gpu_mesh = Arc::new(GpuMesh::new(
            re_ctx,
            &shape_builder.into_cpu_mesh("occluder rectangle".to_owned(), re_ctx),
        )?);

        let mut instance = GpuMeshInstance::new(gpu_mesh);
        instance.world_from_mesh = glam::Affine3A::from_scale_rotation_translation(
            glam::vec3(2.0, 1.5, 1.0),
            glam::Quat::IDENTITY,
            glam::vec3(0.0, 0.0, 0.8),
        );

        Ok(MeshDrawData::new(re_ctx, &[instance])?)
    }

    fn benchmark_configs() -> Vec<Config> {
        let command_line = command_line();
        let tiers = matching_tiers(command_line.selected_tier);
        let cameras = matching_cameras(command_line.selected_camera);
        let size_modes = matching_size_modes(command_line.selected_size_mode);
        let experiment = selected_experiment(command_line).unwrap_or(Experiment::None);

        let mut configs = Vec::with_capacity(tiers.len() * cameras.len() * size_modes.len());
        for &tier in &tiers {
            for &camera in &cameras {
                for &size_mode in &size_modes {
                    configs.push(Config {
                        tier,
                        camera,
                        size_mode,
                        experiment,
                    });
                }
            }
        }

        assert!(
            !configs.is_empty(),
            "benchmark filters matched no cases; use --points <1m|4m|16m>, --camera <near|medium|far|away>, --size <screen|world>, --experiment <none|occluder-rectangle|sorted-front-to-back>"
        );

        configs
    }

    fn start_benchmark(&mut self) {
        let configs = Self::benchmark_configs();
        let first_config = configs[0];
        re_log::info!(
            "Starting benchmark: {} combinations, {} measured frames each at {}x{} ({} warmup frames ignored)",
            configs.len(),
            self.benchmark_frames,
            BENCHMARK_RESOLUTION[0],
            BENCHMARK_RESOLUTION[1],
            BENCHMARK_WARMUP_FRAMES
        );
        self.benchmark = Some(BenchmarkRun {
            configs,
            current: 0,
            frames_per_config: self.benchmark_frames,
            frames_seen: 0,
            accumulated: web_time::Duration::ZERO,
            frame_times: Vec::with_capacity(self.benchmark_frames as usize),
            results: Vec::new(),
            warmup_frames_seen: 0,
            last_frame_finished_at: None,
        });
        self.set_config(first_config);
    }

    fn update_benchmark(&mut self, last_frame_duration: web_time::Duration) {
        let Some(mut benchmark) = self.benchmark.take() else {
            return;
        };

        if benchmark.warmup_frames_seen < BENCHMARK_WARMUP_FRAMES {
            benchmark.warmup_frames_seen += 1;
            Self::print_progress(&benchmark);
            self.benchmark = Some(benchmark);
            return;
        }

        benchmark.frames_seen += 1;
        benchmark.accumulated += last_frame_duration;
        benchmark.frame_times.push(last_frame_duration);
        Self::print_progress(&benchmark);

        if benchmark.frames_seen < benchmark.frames_per_config {
            self.benchmark = Some(benchmark);
            return;
        }

        let config = benchmark.configs[benchmark.current];
        let average = benchmark.accumulated / benchmark.frames_per_config;
        benchmark.results.push(BenchmarkResult {
            config,
            total: benchmark.accumulated,
            average,
            frame_times: std::mem::take(&mut benchmark.frame_times),
        });

        benchmark.current += 1;
        if benchmark.current == benchmark.configs.len() {
            println!();
            self.print_benchmark_table(&benchmark.results, benchmark.frames_per_config);
            self.benchmark = None;
            self.exit_after_benchmark = true;
        } else {
            let next_config = benchmark.configs[benchmark.current];
            benchmark.frames_seen = 0;
            benchmark.accumulated = web_time::Duration::ZERO;
            benchmark.frame_times = Vec::with_capacity(benchmark.frames_per_config as usize);
            benchmark.warmup_frames_seen = 0;
            benchmark.last_frame_finished_at = None;
            self.set_config(next_config);
            self.benchmark = Some(benchmark);
        }
    }

    fn print_progress(benchmark: &BenchmarkRun) {
        let frames_per_config_with_warmup =
            (benchmark.frames_per_config + BENCHMARK_WARMUP_FRAMES) as usize;
        let total_frames = frames_per_config_with_warmup * benchmark.configs.len();
        let completed_frames = benchmark.current * frames_per_config_with_warmup
            + benchmark.warmup_frames_seen as usize
            + benchmark.frames_seen as usize;
        let filled = completed_frames * 40 / total_frames;
        let bar = format!(
            "{}{}",
            "#".repeat(filled),
            "-".repeat(40_usize.saturating_sub(filled))
        );
        let config = benchmark.configs[benchmark.current];
        print!(
            "\rBenchmark [{}] {:>5.1}% ({}/{}) warmup {}/{}, measured {}/{} frames, {} pts, {} camera, {} size, {} experiment",
            bar,
            100.0 * completed_frames as f32 / total_frames as f32,
            completed_frames,
            total_frames,
            benchmark.warmup_frames_seen,
            BENCHMARK_WARMUP_FRAMES,
            benchmark.frames_seen,
            benchmark.frames_per_config,
            POINT_COUNTS[config.tier],
            camera_label(config.camera),
            config.size_mode.label(),
            config.experiment.label()
        );
        std::io::stdout().flush().ok();
    }

    fn print_benchmark_table(&self, results: &[BenchmarkResult], frames_per_config: u32) {
        println!("Benchmark complete. Measured frames per combination: {frames_per_config}");
        if let Some(adapter_info) = &self.adapter_info {
            println!("Adapter: {adapter_info}");
        }
        println!(
            "Frame times are CPU wall-clock durations measured after waiting for submitted GPU work to complete."
        );
        println!();
        println!(
            "{:<10} {:<8} {:<22} {:<6} {:>10} {:>10} {:>10} {:>10} {:>10}",
            "points",
            "camera",
            "experiment",
            "size",
            "total ms",
            "avg ms",
            "p50 ms",
            "p90 ms",
            "p99 ms"
        );
        println!("{}", "-".repeat(107));
        for result in results {
            let mut frame_times = result.frame_times.clone();
            frame_times.sort_unstable();
            println!(
                "{:<10} {:<8} {:<22} {:<6} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>10.3}",
                POINT_COUNTS[result.config.tier],
                camera_label(result.config.camera),
                result.config.experiment.label(),
                result.config.size_mode.label(),
                result.total.as_secs_f64() * 1000.0,
                result.average.as_secs_f64() * 1000.0,
                quantile_ms(&frame_times, 0.50),
                quantile_ms(&frame_times, 0.90),
                quantile_ms(&frame_times, 0.99),
            );
        }
    }
}

impl framework::Example for PointCloudBench {
    fn title() -> &'static str {
        "Point cloud bench"
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        let command_line = command_line();
        let mut this = Self {
            config: initial_config(command_line),
            benchmark_frames: command_line.benchmark_frames,
            current_cloud: None,
            benchmark: None,
            exit_after_benchmark: false,
            adapter_info: Some(re_renderer::adapter_info_summary(re_ctx.adapter_info())),
        };
        this.log_controls();
        if command_line.run_benchmark {
            this.start_benchmark();
        }
        this
    }

    fn draw(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        resolution: [u32; 2],
        _time: &framework::Time,
        pixels_per_point: f32,
    ) -> anyhow::Result<Vec<framework::ViewDrawResult>> {
        let benchmark_active = self.benchmark.is_some();

        let resolution = if benchmark_active {
            BENCHMARK_RESOLUTION
        } else {
            resolution
        };
        let draw_data = self.point_cloud_draw_data(re_ctx)?;
        let aspect_ratio = resolution[0] as f32 / resolution[1] as f32;
        let camera_position = self.current_camera_position();
        let camera_target = self.current_camera_target();
        let occluder_draw_data = (self.config.experiment == Experiment::OccluderRectangle)
            .then(|| Self::occluder_draw_data(re_ctx))
            .transpose()?;

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            view_builder::TargetConfiguration {
                name: "Large Point Cloud".into(),
                resolution_in_pixel: resolution,
                view_from_world: IsoTransform::look_at_rh(
                    camera_position,
                    camera_target,
                    glam::Vec3::Y,
                )
                .ok_or_else(|| anyhow::format_err!("invalid camera"))?,
                projection_from_view: Projection::Perspective {
                    vertical_fov: 70.0_f32.to_radians(),
                    near_plane_distance: 0.01,
                    aspect_ratio,
                },
                pixels_per_point,
                ..Default::default()
            },
        )?;

        view_builder.queue_draw(
            re_ctx,
            GenericSkyboxDrawData::new(re_ctx, Default::default()),
        );
        if let Some(occluder_draw_data) = occluder_draw_data {
            // Use an opaque mesh occluder and queue it before the point cloud.
            // The opaque phase sorts by renderer first, and `MeshRenderer` currently sorts before
            // `PointCloudRenderer`, so this gets depth into the buffer before the points are drawn.
            view_builder.queue_draw(re_ctx, occluder_draw_data);
        }
        let command_buffer = view_builder
            .queue_draw(re_ctx, draw_data)
            .draw(re_ctx, re_renderer::Rgba::TRANSPARENT)?;

        Ok(vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: glam::Vec2::ZERO,
        }])
    }

    fn suppress_frame_time_logging(&self) -> bool {
        self.benchmark.is_some()
    }

    fn present_mode(&self) -> wgpu::PresentMode {
        if self.benchmark.is_some() {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::AutoVsync
        }
    }

    fn should_exit(&self) -> bool {
        self.exit_after_benchmark
    }

    #[cfg_attr(target_arch = "wasm32", expect(unused_variables))]
    fn on_frame_finished(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        submission_index: wgpu::SubmissionIndex,
    ) {
        let Some(benchmark) = &mut self.benchmark else {
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        if let Err(err) = re_ctx.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index),
            timeout: None,
        }) {
            re_log::warn_once!("Failed to wait for submitted GPU work: {err}");
        }

        let now = web_time::Instant::now();
        let Some(last_frame_finished_at) = benchmark.last_frame_finished_at.replace(now) else {
            return;
        };
        let frame_duration = now - last_frame_finished_at;
        self.update_benchmark(frame_duration);
    }

    fn on_key_event(&mut self, event: winit::event::KeyEvent) {
        if event.state != ElementState::Released {
            return;
        }

        match event.logical_key {
            Key::Character(key) if key == "1" => self.set_config(Config {
                tier: 0,
                ..self.config
            }),
            Key::Character(key) if key == "2" => self.set_config(Config {
                tier: 1,
                ..self.config
            }),
            Key::Character(key) if key == "3" => self.set_config(Config {
                tier: 2,
                ..self.config
            }),
            Key::Character(key) if key.eq_ignore_ascii_case("c") => self.set_config(Config {
                camera: (self.config.camera + 1) % CAMERA_COUNT,
                ..self.config
            }),
            Key::Character(key) if key.eq_ignore_ascii_case("s") => {
                let mut size_mode = self.config.size_mode;
                size_mode.toggle();
                self.set_config(Config {
                    size_mode,
                    ..self.config
                });
            }
            Key::Character(key) if key.eq_ignore_ascii_case("e") && self.benchmark.is_none() => {
                self.cycle_experiment();
            }
            Key::Character(key) if key.eq_ignore_ascii_case("o") && self.benchmark.is_none() => {
                self.toggle_occluder_rectangle();
            }
            _ => {}
        }
    }
}

fn generate_dense_cloud(
    point_count: usize,
    radius: Size,
    positions_and_radii: &mut Vec<PositionRadius>,
    colors: &mut Vec<Color32>,
) {
    let side = (point_count as f32).cbrt().ceil() as u32;
    let inv_side = 1.0 / side as f32;

    for i in 0..point_count as u32 {
        let x = i % side;
        let y = (i / side) % side;
        let z = i / (side * side);

        // Tiny deterministic jitter breaks perfect grid aliasing while keeping reproducible positions.
        let jitter = glam::Vec3::new(
            hash_to_unit_float(i.wrapping_mul(0x9E37_79B9)),
            hash_to_unit_float(i.wrapping_mul(0x85EB_CA6B)),
            hash_to_unit_float(i.wrapping_mul(0xC2B2_AE35)),
        ) - 0.5;

        // Compact volume + large radii intentionally create heavy overdraw.
        let pos = (glam::Vec3::new(x as f32, y as f32, z as f32) + 0.35 * jitter) * inv_side
            - glam::Vec3::splat(0.5);
        let pos = glam::Vec3::new(pos.x * 1.6, pos.y * 1.6, pos.z * 0.8);

        let radius_factor = 0.55 + 1.45 * hash_to_unit_float(i.wrapping_mul(0x27D4_EB2D));
        let radius = radius * radius_factor;

        positions_and_radii.push(PositionRadius { pos, radius });
        colors.push(Color32::from_rgb(
            (64.0 + 191.0 * (pos.x + 0.8) / 1.6) as u8,
            (64.0 + 191.0 * (pos.y + 0.8) / 1.6) as u8,
            (64.0 + 191.0 * (pos.z + 0.4) / 0.8) as u8,
        ));
    }

    // Randomize storage order so front/back traversal is not correlated with generation order.
    for i in (1..positions_and_radii.len()).rev() {
        let j = hash_u32(i as u32) as usize % (i + 1);
        positions_and_radii.swap(i, j);
        colors.swap(i, j);
    }
}

fn sort_front_to_back(
    positions_and_radii: &mut [PositionRadius],
    colors: &mut [Color32],
    camera_position: glam::Vec3,
    camera_target: glam::Vec3,
) {
    let camera_forward = (camera_target - camera_position).normalize();

    let mut sorted_indices = (0..positions_and_radii.len()).collect::<Vec<_>>();
    sorted_indices.sort_unstable_by(|a, b| {
        let depth_a = (positions_and_radii[*a].pos - camera_position).dot(camera_forward);
        let depth_b = (positions_and_radii[*b].pos - camera_position).dot(camera_forward);
        depth_a.total_cmp(&depth_b)
    });

    let mut old_to_new_index = vec![0; sorted_indices.len()];
    for (new_index, old_index) in sorted_indices.into_iter().enumerate() {
        old_to_new_index[old_index] = new_index;
    }

    for index in 0..old_to_new_index.len() {
        while old_to_new_index[index] != index {
            let swap_with = old_to_new_index[index];
            positions_and_radii.swap(index, swap_with);
            colors.swap(index, swap_with);
            old_to_new_index.swap(index, swap_with);
        }
    }
}

fn command_line() -> &'static CommandLine {
    COMMAND_LINE
        .get()
        .expect("command line should be parsed before starting the example")
}

fn initial_config(command_line: &CommandLine) -> Config {
    Config {
        tier: command_line.selected_tier.unwrap_or(0),
        camera: command_line.selected_camera.unwrap_or(1),
        size_mode: command_line
            .selected_size_mode
            .unwrap_or(PointSizeMode::ScreenSpace),
        experiment: selected_experiment(command_line).unwrap_or(Experiment::None),
    }
}

fn selected_experiment(command_line: &CommandLine) -> Option<Experiment> {
    command_line.selected_experiment
}

fn matching_tiers(selected_tier: Option<usize>) -> Vec<usize> {
    match selected_tier {
        Some(tier) => vec![tier],
        None => (0..POINT_COUNTS.len()).collect(),
    }
}

fn matching_cameras(selected_camera: Option<usize>) -> Vec<usize> {
    match selected_camera {
        Some(camera) => vec![camera],
        None => (0..CAMERA_COUNT).collect(),
    }
}

fn matching_size_modes(selected_size_mode: Option<PointSizeMode>) -> Vec<PointSizeMode> {
    match selected_size_mode {
        Some(size_mode) => vec![size_mode],
        None => PointSizeMode::ALL.to_vec(),
    }
}

fn parse_points_arg(value: &str) -> Result<usize, String> {
    match value.to_ascii_lowercase().replace('_', "").as_str() {
        "1" | "1000000" | "1m" => Ok(0),
        "2" | "4000000" | "4m" => Ok(1),
        "3" | "16000000" | "16m" => Ok(2),
        _ => Err(format!(
            "invalid --points value {value:?}; expected 1m, 4m, or 16m"
        )),
    }
}

fn parse_camera_arg(value: &str) -> Result<usize, String> {
    match value.to_ascii_lowercase().as_str() {
        "near" | "1" => Ok(0),
        "medium" | "mid" | "2" => Ok(1),
        "far" | "3" => Ok(2),
        "away" | "looking-away" | "lookingaway" | "4" => Ok(CAMERA_LOOKING_AWAY_INDEX),
        _ => Err(format!(
            "invalid --camera value {value:?}; expected near, medium, far, or away"
        )),
    }
}

fn parse_size_mode_arg(value: &str) -> Result<PointSizeMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "screen" | "screen-space" | "screenspace" => Ok(PointSizeMode::ScreenSpace),
        "world" | "world-space" | "worldspace" => Ok(PointSizeMode::WorldSpace),
        _ => Err(format!(
            "invalid --size value {value:?}; expected screen or world"
        )),
    }
}

fn parse_experiment_arg(value: &str) -> Result<Experiment, String> {
    match value.to_ascii_lowercase().as_str() {
        "none" | "baseline" => Ok(Experiment::None),
        "occluder" | "occluder-rectangle" | "occluderrectangle" => {
            Ok(Experiment::OccluderRectangle)
        }
        "sorted"
        | "front-to-back"
        | "fronttoback"
        | "sorted-front-to-back"
        | "sortedfronttoback" => Ok(Experiment::SortedFrontToBack),
        _ => Err(format!(
            "invalid --experiment value {value:?}; expected none, occluder-rectangle, or sorted-front-to-back"
        )),
    }
}

fn camera_position(camera: usize) -> glam::Vec3 {
    let distance = CAMERA_DISTANCES[camera.min(CAMERA_DISTANCES.len() - 1)];
    glam::Vec3::new(distance * 0.35, distance * 0.20, distance)
}

fn camera_target(camera: usize) -> glam::Vec3 {
    let camera_position = camera_position(camera);
    if camera == CAMERA_LOOKING_AWAY_INDEX {
        camera_position * 2.0
    } else {
        glam::Vec3::ZERO
    }
}

fn camera_label(camera: usize) -> &'static str {
    match camera {
        0 => "near",
        1 => "medium",
        2 => "far",
        CAMERA_LOOKING_AWAY_INDEX => "away",
        _ => "unknown",
    }
}

fn quantile_ms(sorted_frame_times: &[web_time::Duration], quantile: f64) -> f64 {
    if sorted_frame_times.is_empty() {
        return f64::NAN;
    }

    let rank = (sorted_frame_times.len() as f64 * quantile).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_frame_times.len() - 1);
    sorted_frame_times[index].as_secs_f64() * 1000.0
}

fn hash_to_unit_float(value: u32) -> f32 {
    (hash_u32(value) as f32) / (u32::MAX as f32)
}

fn hash_u32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    value
}

fn main() {
    let command_line = CommandLine::parse();
    assert!(
        COMMAND_LINE.set(command_line).is_ok(),
        "command line should only be parsed once"
    );
    framework::start::<PointCloudBench>();
}
