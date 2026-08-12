//! Renders a gaussian splat `.ply` with `re_renderer`'s gaussian splat renderer from a fixed
//! camera, offscreen, and compares the result against a reference image (e.g. from brush).
//!
//! Run with no arguments to render the checked-in `cactus.ply` from a fixed camera, so
//! `cargo run -p test_gaussian_compare` just works as a quick demo / smoke test.

use std::path::PathBuf;

use clap::Parser as _;

use re_renderer::renderer::SH_TEXELS_PER_GAUSSIAN;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder};
use re_renderer::{
    GaussianShCoefficient, GaussianSplatBuilder, RenderConfig, RenderContext, Rgba, Rgba32Unmul,
    ScreenshotProcessor, device_caps,
};
use re_sdk_types::archetypes::GaussianSplats3D;
use re_types_core::Loggable as _;

/// Defaults render the checked-in `cactus.ply` from a fixed camera.
#[derive(clap::Parser)]
struct Args {
    /// Input `.ply` file. Defaults to the checked-in `cactus.ply`.
    #[arg(long)]
    ply: Option<PathBuf>,

    /// Output `.png` path.
    #[arg(long, default_value = "cactus.png")]
    out: PathBuf,

    /// Camera position, as `x y z`.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], default_values_t = [2.0, 1.5, 2.5])]
    pos: Vec<f32>,

    /// Camera look-at target, as `x y z`.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], default_values_t = [0.0, -0.7, 0.1])]
    target: Vec<f32>,

    /// Vertical field of view, in degrees.
    #[arg(long, default_value_t = 45.0)]
    fov_y_deg: f32,

    /// Output resolution, as `width height`.
    #[arg(long, num_args = 2, value_names = ["W", "H"], default_values_t = [512, 512])]
    resolution: Vec<u32>,

    /// Optional reference `.png` to compare the render against.
    #[arg(long)]
    reference: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args {
        ply,
        out: out_path,
        pos,
        target,
        fov_y_deg,
        resolution,
        reference: reference_path,
    } = Args::parse();

    let ply_path = ply.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            // `tests/rust/test_gaussian_compare` -> `tests/rust` -> `tests` -> repo-root.
            .ancestors()
            .nth(3)
            .expect("workspace root is three ancestors up")
            .join("tests/assets/gaussian_splats/cactus.ply")
    });
    let pos = glam::Vec3::from_slice(&pos);
    let target = glam::Vec3::from_slice(&target);
    let resolution = [resolution[0], resolution[1]];
    let fov_y = fov_y_deg.to_radians();

    // --- Load the .ply through our actual loader ---
    let gaussians = GaussianSplats3D::from_ply_file_path(&ply_path)?;

    let field = |name: &str, opt: &Option<re_sdk_types::SerializedComponentBatch>| {
        opt.as_ref()
            .map(|col| col.array.clone())
            .ok_or_else(|| anyhow::anyhow!("PLY has no {name}"))
    };

    let centers: Vec<glam::Vec3> =
        re_sdk_types::components::Position3D::from_arrow(&field("centers", &gaussians.centers)?)?
            .into_iter()
            .map(|p| glam::Vec3::from_array(p.0.0))
            .collect();
    let scales: Vec<glam::Vec3> =
        re_sdk_types::components::Scale3D::from_arrow(&field("scales", &gaussians.scales)?)?
            .into_iter()
            .map(|s| glam::Vec3::from_array(s.0.0))
            .collect();
    let rotations: Vec<glam::Quat> = re_sdk_types::components::RotationQuat::from_arrow(&field(
        "quaternions",
        &gaussians.quaternions,
    )?)?
    .into_iter()
    .map(|q| glam::Quat::from_array(q.0.0))
    .collect();
    let colors: Vec<Rgba32Unmul> =
        re_sdk_types::components::Color::from_arrow(&field("colors", &gaussians.colors)?)?
            .into_iter()
            .map(|c| Rgba32Unmul::from_rgba_unmul_array(c.to_array()))
            .collect();
    let sh_coefficients: Vec<[GaussianShCoefficient; 15]> = gaussians
        .sh_coefficients
        .as_ref()
        .map(|sh| {
            re_sdk_types::components::SphericalHarmonics3Rgb::from_arrow(&sh.array).map(|v| {
                v.into_iter()
                    .map(|sh| std::array::from_fn(|i| GaussianShCoefficient::from_rgb(sh.0.0[i])))
                    .collect::<Vec<_>>()
            })
        })
        .transpose()?
        .unwrap_or_default();
    println!(
        "loaded {} splats ({} with SH)",
        centers.len(),
        sh_coefficients.len()
    );

    // --- Headless render context ---
    let instance = wgpu::Instance::new(device_caps::testing_instance_descriptor());
    let adapter = pollster::block_on(device_caps::select_testing_adapter(&instance));
    let caps = device_caps::DeviceCaps::from_adapter(&adapter)?;
    let (device, queue) = pollster::block_on(adapter.request_device(&caps.device_descriptor()))?;
    let mut ctx = RenderContext::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        |_| RenderConfig::testing(),
    )?;

    ctx.begin_frame();

    let mut view_builder = ViewBuilder::new(
        &ctx,
        TargetConfiguration {
            name: "gaussian_compare".into(),
            resolution_in_pixel: resolution,
            view_from_world: macaw::IsoTransform::look_at_rh(pos, target, glam::Vec3::Z)
                .ok_or_else(|| anyhow::anyhow!("invalid camera"))?,
            projection_from_view: Projection::Perspective {
                vertical_fov: fov_y,
                near_plane_distance: 0.01,
                aspect_ratio: resolution[0] as f32 / resolution[1] as f32,
            },
            ..Default::default()
        },
        re_renderer::ViewBuilderId::new(0),
    )?;

    let mut splat_builder = GaussianSplatBuilder::new(&ctx);
    splat_builder.batch("gaussians").add_gaussians(
        &centers,
        &scales,
        &rotations,
        &colors,
        &sh_coefficients,
        SH_TEXELS_PER_GAUSSIAN,
        &[],
    );

    view_builder.queue_draw(&ctx, splat_builder.into_draw_data()?);
    view_builder.schedule_screenshot(&ctx, 42, ())?;

    let command_buffer = view_builder.draw(&ctx, Rgba::BLACK)?;
    ctx.before_submit();
    ctx.queue.submit([command_buffer]);
    ctx.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(30)),
    })?;

    // Pump frames until the screenshot readback arrives.
    let mut screenshot: Option<(Vec<u8>, [u32; 2])> = None;
    for _ in 0..10 {
        ctx.begin_frame();
        ScreenshotProcessor::next_readback_result::<()>(&ctx, 42, |data, extent, ()| {
            screenshot = Some((data.to_vec(), [extent.x, extent.y]));
        });
        if screenshot.is_some() {
            break;
        }
        ctx.before_submit();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let (rgba, [w, h]) = screenshot.ok_or_else(|| anyhow::anyhow!("no screenshot received"))?;

    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();
    let our_img =
        image::RgbImage::from_raw(w, h, rgb).ok_or_else(|| anyhow::anyhow!("bad image"))?;
    our_img.save(&out_path)?;
    println!("saved {}", out_path.display());

    // --- Compare against the reference ---
    if let Some(reference_path) = reference_path {
        let reference = image::open(reference_path)?.to_rgb8();
        anyhow::ensure!(reference.dimensions() == (w, h), "resolution mismatch");

        let mut sum_sq_err = 0.0f64;
        let mut max_err = 0u8;
        let mut diff = image::RgbImage::new(w, h);
        for (a, (b, d)) in std::iter::zip(
            our_img.pixels(),
            std::iter::zip(reference.pixels(), diff.pixels_mut()),
        ) {
            for c in 0..3 {
                let err = a.0[c].abs_diff(b.0[c]);
                sum_sq_err += (err as f64) * (err as f64);
                max_err = max_err.max(err);
                d.0[c] = err.saturating_mul(4); // amplified for visibility
            }
        }
        let mse = sum_sq_err / (w as f64 * h as f64 * 3.0);
        let psnr = 10.0 * (255.0f64 * 255.0 / mse).log10();
        println!("MSE: {mse:.2}, PSNR: {psnr:.2} dB, max channel error: {max_err}");

        let diff_path = format!("{}.diff.png", out_path.display());
        diff.save(&diff_path)?;
        println!("diff (4x amplified) saved to {diff_path}");
    }

    Ok(())
}
