//! Out of order transform logging can be challenging for the transform resolver!
//!
//! This manual end-to-end stress-test logs transforms frequently & repeatedly at the same timestamps in a loop.

use std::f32::consts::TAU;

use rerun::external::re_log;
use rerun::{
    Angle, Color, EntityPath, LineStrips3D, Points3D, Position3D, RecordingStream,
    RotationAxisAngle, Transform3D, TransformRelation, ViewCoordinates,
};

const SUN_TO_PLANET_DISTANCE: f32 = 6.0;
const PLANET_TO_MOON_DISTANCE: f32 = 3.0;
const ROTATION_SPEED_PLANET: f32 = 2.0;
const ROTATION_SPEED_MOON: f32 = 5.0;

const NUM_PLANETS: usize = 5;

/// Some basic scene setup.
fn setup_scene(rec: &RecordingStream) -> anyhow::Result<()> {
    rec.log_static("/", &ViewCoordinates::RIGHT_HAND_Z_UP())?;

    // All are in the center of their own space:
    fn log_point(
        rec: &RecordingStream,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        rec.log_static(
            ent_path,
            &Points3D::new([Position3D::ZERO])
                .with_radii([radius])
                .with_colors([Color::from_rgb(color[0], color[1], color[2])]),
        )
        .map_err(Into::into)
    }
    log_point(rec, "/sun", 1.0, [255, 200, 10])?;

    // paths where the planet & moon move
    let create_path = |distance: f32| {
        LineStrips3D::new([(0..=100).map(|i| {
            let angle = i as f32 * 0.01 * TAU;
            (angle.sin() * distance, angle.cos() * distance, 0.0)
        })])
    };

    for planet in 0..NUM_PLANETS {
        log_point(rec, format!("/sun/planet{planet}"), 0.4, [40, 80, 200])?;
        log_point(
            rec,
            format!("/sun/planet{planet}/moon"),
            0.15,
            [180, 180, 180],
        )?;
        rec.log_static(
            format!("/sun/planet{planet}_path"),
            &create_path(SUN_TO_PLANET_DISTANCE * (planet + 1) as f32),
        )?;
        rec.log_static(
            format!("/sun/planet{planet}/moon_path"),
            &create_path(PLANET_TO_MOON_DISTANCE),
        )?;
    }

    Ok(())
}

fn log_transforms(rec: &RecordingStream, sim_time: f32) -> anyhow::Result<()> {
    rec.set_duration_secs("sim_time", sim_time as f64);
    let rotation = 10.0 * sim_time;

    for planet in 0..NUM_PLANETS {
        let distance_factor = (planet + 1) as f32;

        rec.log(
            format!("/sun/planet{planet}"),
            &Transform3D::from_translation_rotation(
                [
                    (rotation * ROTATION_SPEED_PLANET).sin()
                        * SUN_TO_PLANET_DISTANCE
                        * distance_factor,
                    (rotation * ROTATION_SPEED_PLANET).cos()
                        * SUN_TO_PLANET_DISTANCE
                        * distance_factor,
                    0.0,
                ],
                RotationAxisAngle::new(glam::Vec3::X, Angle::from_degrees(20.0)),
            ),
        )?;

        rec.log(
            format!("/sun/planet{planet}/moon"),
            &Transform3D::from_translation([
                (rotation * ROTATION_SPEED_MOON).cos() * PLANET_TO_MOON_DISTANCE,
                (rotation * ROTATION_SPEED_MOON).sin() * PLANET_TO_MOON_DISTANCE,
                0.0,
            ])
            .with_relation(TransformRelation::ChildFromParent),
        )?;
    }

    Ok(())
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "0.008")] // 120fps
    log_frequency_secs: f64,

    #[clap(long, default_value = "2.0")]
    time_per_run_secs: f64,

    #[clap(long, default_value = "20")]
    num_runs: usize,
}

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    setup_scene(rec)?;

    let num_steps = (args.time_per_run_secs / args.log_frequency_secs).ceil() as usize;
    assert!(
        num_steps > 0,
        "time_per_run_secs must be greater than log_frequency_secs"
    );

    for _ in 0..args.num_runs {
        for step in 0..num_steps {
            let time = std::time::Instant::now();

            // Wallclock time and sim time are in sync because why not!
            let sim_time = args.log_frequency_secs * step as f64;
            log_transforms(rec, sim_time as f32)?;
            rec.flush_blocking()?;

            let duration = time.elapsed();
            let time_to_sleep = std::time::Duration::try_from_secs_f64(args.log_frequency_secs)?
                .saturating_sub(duration);

            if time_to_sleep.is_zero() {
                re_log::warn_once!("Can't keep up with desired log frequency.");
            } else {
                // A bit crude, but good enough.
                std::thread::sleep(time_to_sleep);
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args
        .rerun
        .init("rerun_example_test_out_of_order_transforms")?;
    run(&rec, &args)
}
