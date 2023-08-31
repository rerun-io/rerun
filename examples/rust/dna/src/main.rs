//! The example from our Getting Started page.

use std::f32::consts::TAU;

use itertools::Itertools as _;

use rerun::{
    components::{Color, LineStrip3D, Point3D, Radius, Transform3D},
    datatypes::Vec3D,
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
    MsgSender, MsgSenderError, RecordingStream,
};

const NUM_POINTS: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store_info = rerun::new_store_info("DNA Abacus");
    rerun::native_viewer::spawn(store_info, Default::default(), |rec| {
        run(&rec).unwrap();
    })?;
    Ok(())
}

fn run(rec: &RecordingStream) -> Result<(), MsgSenderError> {
    let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
    let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

    rec.set_time_seconds("stable_time", 0f64);

    MsgSender::new("dna/structure/left")
        .with_component(&points1.iter().copied().map(Point3D::from).collect_vec())?
        .with_component(&colors1.iter().copied().map(Color::from).collect_vec())?
        .with_splat(Radius(0.08))?
        .send(rec)?;

    MsgSender::new("dna/structure/right")
        .with_component(&points2.iter().copied().map(Point3D::from).collect_vec())?
        .with_component(&colors2.iter().copied().map(Color::from).collect_vec())?
        .with_splat(Radius(0.08))?
        .send(rec)?;

    let scaffolding = points1
        .iter()
        .interleave(points2.iter())
        .copied()
        .map(Vec3D::from)
        .map(Into::into)
        .chunks(2)
        .into_iter()
        .map(|positions| LineStrip3D(positions.collect_vec()))
        .collect_vec();
    MsgSender::new("dna/structure/scaffolding")
        .with_component(&scaffolding)?
        .with_splat(Color::from([128, 128, 128, 255]))?
        .send(rec)?;

    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let offsets = (0..NUM_POINTS).map(|_| rng.gen::<f32>()).collect_vec();

    for i in 0..400 {
        let time = i as f32 * 0.01;

        rec.set_time_seconds("stable_time", time as f64);

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
                    Color::from_rgb(c, c, c),
                )
            })
            .unzip();
        MsgSender::new("dna/structure/scaffolding/beads")
            .with_component(&beads)?
            .with_component(&colors)?
            .with_splat(Radius(0.06))?
            .send(rec)?;

        MsgSender::new("dna/structure")
            .with_component(&[Transform3D::new(rerun::transform::RotationAxisAngle::new(
                glam::Vec3::Z,
                rerun::transform::Angle::Radians(time / 4.0 * TAU),
            ))])?
            .send(rec)?;
    }

    Ok(())
}
