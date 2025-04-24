//! Builds an analog clock using Rerun's `Vector3D` primitive.
//!
//! This is a great benchmark for many small events.
//!
//! Usage:
//! ```
//! cargo run -p clock -- --help
//! ```

use std::f32::consts::TAU;

use rerun::external::re_log;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "10000")]
    steps: usize,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_clock")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    const LENGTH_S: f32 = 20.0;
    const LENGTH_M: f32 = 10.0;
    const LENGTH_H: f32 = 4.0;
    const WIDTH_S: f32 = 0.25;
    const WIDTH_M: f32 = 0.4;
    const WIDTH_H: f32 = 0.6;

    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Y_UP())?;

    rec.log_static(
        "world/frame",
        &rerun::Boxes3D::from_half_sizes([(LENGTH_S, LENGTH_S, 1.0)]),
    )?;

    fn tip(angle: f32, length: f32) -> [f32; 3] {
        [length * angle.sin(), length * angle.cos(), 0.0]
    }

    fn color(angle: f32, blue: u8) -> rerun::Color {
        let c = (angle * 255.0) as u8;
        rerun::Color::from_unmultiplied_rgba(255 - c, c, blue, u8::max(128, blue))
    }

    fn log_hand(
        rec: &rerun::RecordingStream,
        name: &str,
        step: usize,
        angle: f32,
        length: f32,
        width: f32,
        blue: u8,
    ) -> anyhow::Result<()> {
        let pos = tip(angle * TAU, length);
        let color = color(angle, blue);

        rec.set_duration_secs("sim_time", step as f64);

        rec.log(
            format!("world/{name}_pt"),
            &rerun::Points3D::new([pos]).with_colors([color]),
        )?;
        rec.log(
            format!("world/{name}_hand"),
            &rerun::Arrows3D::from_vectors([pos])
                .with_origins([(0.0, 0.0, 0.0)])
                .with_colors([color])
                .with_radii([width * 0.5]),
        )?;

        Ok(())
    }

    #[rustfmt::skip]
    for step in 0..args.steps {
        log_hand(rec, "seconds", step, (step % 60) as f32 / 60.0, LENGTH_S, WIDTH_S, 0)?;
        log_hand(rec, "minutes", step, (step % 3600) as f32 / 3600.0, LENGTH_M, WIDTH_M, 128)?;
        log_hand(rec, "hours", step, (step % 43200) as f32 / 43200.0, LENGTH_H, WIDTH_H, 255)?;
    };

    Ok(())
}
