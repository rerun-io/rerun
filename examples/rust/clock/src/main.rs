//! Builds an analog clock using Rerun's `Vector3D` primitive.
//!
//! This is a great benchmark for many small events.
//!
//! Usage:
//! ```
//! cargo run -p clock -- --help
//! ```

use std::f32::consts::TAU;

use rerun::{
    archetypes::{Arrows3D, Points3D},
    components::{Box3D, Color, ViewCoordinates},
    coordinates::SignedAxis3,
    external::re_log,
    RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "10000")]
    steps: usize,
}

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    const LENGTH_S: f32 = 20.0;
    const LENGTH_M: f32 = 10.0;
    const LENGTH_H: f32 = 4.0;
    const WIDTH_S: f32 = 0.25;
    const WIDTH_M: f32 = 0.4;
    const WIDTH_H: f32 = 0.6;

    // TODO(#2816): ViewCoordinates archetype
    let view_coords = ViewCoordinates::from_up_and_handedness(
        SignedAxis3::POSITIVE_Y,
        rerun::coordinates::Handedness::Right,
    );
    rec.log_component_lists("world", true, 1, [&view_coords as _])?;

    // TODO(#2786): Box3D archetype
    rec.log_component_lists(
        "world/frame",
        true,
        1,
        [&Box3D::new(LENGTH_S, LENGTH_S, 1.0) as _],
    )?;

    fn pos(angle: f32, length: f32) -> [f32; 3] {
        [length * angle.sin(), length * angle.cos(), 0.0]
    }

    fn color(angle: f32, blue: u8) -> Color {
        let c = (angle * 255.0) as u8;
        Color::from_unmultiplied_rgba(255 - c, c, blue, u8::max(128, blue))
    }

    fn log_hand(
        rec: &RecordingStream,
        name: &str,
        step: usize,
        angle: f32,
        length: f32,
        width: f32,
        blue: u8,
    ) -> anyhow::Result<()> {
        let pos = pos(angle * TAU, length);
        let color = color(angle, blue);

        rec.set_time_seconds("sim_time", step as f64);

        rec.log(
            format!("world/{name}_pt"),
            &Points3D::new([pos]).with_colors([color]),
        )?;
        rec.log(
            format!("world/{name}_hand"),
            &Arrows3D::new([pos])
                .with_colors([color])
                .with_radii([width * 0.5]),
        )?;

        Ok(())
    }

    for step in 0..args.steps {
        #[rustfmt::skip]
        log_hand(rec, "seconds", step, (step % 60) as f32 / 60.0, LENGTH_S, WIDTH_S, 0)?;
        #[rustfmt::skip]
        log_hand(rec, "minutes", step, (step % 3600) as f32 / 3600.0, LENGTH_M, WIDTH_M, 128)?;
        #[rustfmt::skip]
        log_hand(rec, "hours", step, (step % 43200) as f32 / 43200.0, LENGTH_H, WIDTH_H, 255)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun
        .clone()
        .run("rerun_example_clock", default_enabled, move |rec| {
            run(&rec, &args).unwrap();
        })
}
