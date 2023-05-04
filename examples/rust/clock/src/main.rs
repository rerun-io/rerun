//! Builds an analog clock using Rerun's `Arrow3D` primitive.
//!
//! This is a great benchmark for many small events.
//!
//! Usage:
//! ```
//! cargo run -p clock -- --help
//! ```

use std::f32::consts::TAU;

use rerun::components::{Arrow3D, Box3D, ColorRGBA, Radius, Vec3D, ViewCoordinates};
use rerun::coordinates::SignedAxis3;
use rerun::time::{Time, TimePoint, TimeType, Timeline};
use rerun::{external::re_log, MsgSender, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "10000")]
    steps: usize,
}

fn run(rec_stream: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    const LENGTH_S: f32 = 20.0;
    const LENGTH_M: f32 = 10.0;
    const LENGTH_H: f32 = 4.0;
    const WIDTH_S: f32 = 0.25;
    const WIDTH_M: f32 = 0.4;
    const WIDTH_H: f32 = 0.6;

    let view_coords = ViewCoordinates::from_up_and_handedness(
        SignedAxis3::POSITIVE_Y,
        rerun::coordinates::Handedness::Right,
    );
    MsgSender::new("world")
        .with_timeless(true)
        .with_component(&[view_coords])?
        .send(rec_stream)?;

    MsgSender::new("world/frame")
        .with_timeless(true)
        .with_component(&[Box3D::new(LENGTH_S, LENGTH_S, 1.0)])?
        .send(rec_stream)?;

    fn sim_time(at: f64) -> TimePoint {
        let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);
        let time = Time::from_seconds_since_epoch(at);
        [(timeline_sim_time, time.into())].into()
    }

    fn pos(angle: f32, length: f32) -> Vec3D {
        Vec3D::new(length * angle.sin(), length * angle.cos(), 0.0)
    }

    fn color(angle: f32, blue: u8) -> ColorRGBA {
        let c = (angle * 255.0) as u8;
        ColorRGBA::from_unmultiplied_rgba(255 - c, c, blue, u8::max(128, blue))
    }

    fn log_hand(
        rec_stream: &RecordingStream,
        name: &str,
        step: usize,
        angle: f32,
        length: f32,
        width: f32,
        blue: u8,
    ) -> anyhow::Result<()> {
        let point = pos(angle * TAU, length);
        let color = color(angle, blue);
        MsgSender::new(format!("world/{name}_pt"))
            .with_timepoint(sim_time(step as _))
            .with_component(&[point])?
            .with_component(&[color])?
            .send(rec_stream)?;
        MsgSender::new(format!("world/{name}_hand"))
            .with_timepoint(sim_time(step as _))
            .with_component(&[Arrow3D {
                origin: glam::Vec3::ZERO.into(),
                vector: point,
            }])?
            .with_component(&[color])?
            .with_component(&[Radius(width * 0.5)])?
            .send(rec_stream)?;

        Ok(())
    }

    for step in 0..args.steps {
        #[rustfmt::skip]
        log_hand(rec_stream, "seconds", step, (step % 60) as f32 / 60.0, LENGTH_S, WIDTH_S, 0)?;
        #[rustfmt::skip]
        log_hand(rec_stream, "minutes", step, (step % 3600) as f32 / 3600.0, LENGTH_M, WIDTH_M, 128)?;
        #[rustfmt::skip]
        log_hand(rec_stream, "hours", step, (step % 43200) as f32 / 43200.0, LENGTH_H, WIDTH_H, 255)?;
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
        .run("clock", default_enabled, move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        })
}
