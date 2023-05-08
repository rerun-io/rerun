//! The example from our Getting Started page.

use std::f32::consts::TAU;

use itertools::Itertools as _;

use rerun::{
    components::{ColorRGBA, LineStrip3D, Point3D, Radius, Transform3D, Vec3D},
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
    time::{Time, TimeType, Timeline},
    MsgSender, MsgSenderError, RecordingStream,
};

const NUM_POINTS: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recording_info = rerun::new_recording_info("DNA Abacus");
    rerun::native_viewer::spawn(recording_info, Default::default(), |rec_stream| {
        run(&rec_stream).unwrap();
    })?;
    Ok(())
}

fn run(rec_stream: &RecordingStream) -> Result<(), MsgSenderError> {
    let stable_time = Timeline::new("stable_time", TimeType::Time);

    let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
    let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

    MsgSender::new("dna/structure/left")
        .with_time(stable_time, 0)
        .with_component(&points1.iter().copied().map(Point3D::from).collect_vec())?
        .with_component(&colors1.iter().copied().map(ColorRGBA::from).collect_vec())?
        .with_splat(Radius(0.08))?
        .send(rec_stream)?;

    MsgSender::new("dna/structure/right")
        .with_time(stable_time, 0)
        .with_component(&points2.iter().copied().map(Point3D::from).collect_vec())?
        .with_component(&colors2.iter().copied().map(ColorRGBA::from).collect_vec())?
        .with_splat(Radius(0.08))?
        .send(rec_stream)?;

    let scaffolding = points1
        .iter()
        .interleave(points2.iter())
        .copied()
        .map(Vec3D::from)
        .chunks(2)
        .into_iter()
        .map(|positions| LineStrip3D(positions.collect_vec()))
        .collect_vec();
    MsgSender::new("dna/structure/scaffolding")
        .with_time(stable_time, 0)
        .with_component(&scaffolding)?
        .with_splat(ColorRGBA::from([128, 128, 128, 255]))?
        .send(rec_stream)?;

    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let offsets = (0..NUM_POINTS).map(|_| rng.gen::<f32>()).collect_vec();

    for i in 0..400 {
        let time = i as f32 * 0.01;

        let times = offsets.iter().map(|offset| time + offset).collect_vec();
        let (beads, colors): (Vec<_>, Vec<_>) = points1
            .iter()
            .interleave(points2.iter())
            .copied()
            .chunks(2)
            .into_iter()
            .enumerate()
            .map(|(n, mut points)| {
                let (p1, p2) = (points.next().unwrap(), points.next().unwrap());
                let c = bounce_lerp(80.0, 230.0, times[n] * 2.0) as u8;
                (
                    Point3D::from(bounce_lerp(p1, p2, times[n])),
                    ColorRGBA::from_rgb(c, c, c),
                )
            })
            .unzip();
        MsgSender::new("dna/structure/scaffolding/beads")
            .with_time(stable_time, Time::from_seconds_since_epoch(time as _))
            .with_component(&beads)?
            .with_component(&colors)?
            .with_splat(Radius(0.06))?
            .send(rec_stream)?;

        MsgSender::new("dna/structure")
            .with_time(stable_time, Time::from_seconds_since_epoch(time as _))
            .with_component(&[Transform3D::Affine3D(
                rerun::transform::Affine3D::from_rotation(
                    rerun::transform::AxisAngleRotation::new(
                        glam::Vec3::Z,
                        rerun::transform::Angle::Radians(time / 4.0 * TAU),
                    ),
                ),
            )])?
            .send(rec_stream)?;
    }

    Ok(())
}
