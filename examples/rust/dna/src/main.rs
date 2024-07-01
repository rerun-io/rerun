//! The example from our Getting Started page.

use std::f32::consts::TAU;

use itertools::Itertools as _;

use rerun::{
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
};

const NUM_POINTS: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus")
        .save("/tmp/helix_chunks.rrd")?;

    let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
    let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

    rec.set_time_seconds("stable_time", 0f64);

    rec.log(
        "dna/structure/left",
        &rerun::Points3D::new(points1.iter().copied())
            .with_colors(colors1)
            .with_radii([0.08]),
    )?;
    rec.log(
        "dna/structure/right",
        &rerun::Points3D::new(points2.iter().copied())
            .with_colors(colors2)
            .with_radii([0.08]),
    )?;

    let points_interleaved: Vec<[glam::Vec3; 2]> = points1
        .into_iter()
        .interleave(points2)
        .chunks(2)
        .into_iter()
        .map(|chunk| chunk.into_iter().collect_vec().try_into().unwrap())
        .collect_vec();

    rec.log(
        "dna/structure/scaffolding",
        &rerun::LineStrips3D::new(points_interleaved.iter().cloned())
            .with_colors([rerun::Color::from([128, 128, 128, 255])]),
    )?;

    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let offsets = (0..NUM_POINTS).map(|_| rng.gen::<f32>()).collect_vec();

    for i in 0..400 {
        let time = i as f32 * 0.01;

        rec.set_time_seconds("stable_time", time as f64);

        let times = offsets.iter().map(|offset| time + offset).collect_vec();
        let (beads, colors): (Vec<_>, Vec<_>) = points_interleaved
            .iter()
            .enumerate()
            .map(|(n, &[p1, p2])| {
                let c = bounce_lerp(80.0, 230.0, times[n] * 2.0) as u8;
                (
                    rerun::Position3D::from(bounce_lerp(p1, p2, times[n])),
                    rerun::Color::from_rgb(c, c, c),
                )
            })
            .unzip();

        rec.log(
            "dna/structure/scaffolding/beads",
            &rerun::Points3D::new(beads)
                .with_colors(colors)
                .with_radii([0.06]),
        )?;

        rec.log(
            "dna/structure",
            &rerun::archetypes::Transform3D::new(rerun::RotationAxisAngle::new(
                glam::Vec3::Z,
                rerun::Angle::Radians(time / 4.0 * TAU),
            )),
        )?;
    }

    Ok(())
}
