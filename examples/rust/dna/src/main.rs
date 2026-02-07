//! The example from our Getting Started page.

use std::f32::consts::TAU;

use itertools::Itertools as _;
use rand::Rng as _;
use rerun::demo_util::{bounce_lerp, color_spiral};
use rerun::external::glam;

const NUM_POINTS: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rerun::external::re_log::setup_logging();

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus").spawn()?;

    let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
    let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

    rec.set_duration_secs("stable_time", 0f64);

    rec.log_static(
        "dna/structure/left",
        &rerun::Points3D::new(points1.iter().copied())
            .with_colors(colors1)
            .with_radii([0.08]),
    )?;
    rec.log_static(
        "dna/structure/right",
        &rerun::Points3D::new(points2.iter().copied())
            .with_colors(colors2)
            .with_radii([0.08]),
    )?;

    let lines: Vec<[glam::Vec3; 2]> = points1
        .iter()
        .zip(&points2)
        .map(|(&p1, &p2)| (p1, p2).into())
        .collect_vec();

    rec.log_static(
        "dna/structure/scaffolding",
        &rerun::LineStrips3D::new(lines.iter().cloned())
            .with_colors([rerun::Color::from_rgb(128, 128, 128)]),
    )?;

    let mut rng = rand::rng();
    let offsets = (0..NUM_POINTS).map(|_| rng.random::<f32>()).collect_vec();

    for i in 0..400 {
        let time = i as f32 * 0.01;

        rec.set_duration_secs("stable_time", time as f64);

        let times = offsets.iter().map(|offset| time + offset).collect_vec();
        let beads = lines
            .iter()
            .zip(&times)
            .map(|(&[p1, p2], &time)| bounce_lerp(p1, p2, time))
            .collect_vec();
        let colors = times
            .iter()
            .map(|time| bounce_lerp(80.0, 230.0, time * 2.0) as u8)
            .map(|c| rerun::Color::from_rgb(c, c, c))
            .collect_vec();

        rec.log(
            "dna/structure/scaffolding/beads",
            &rerun::Points3D::new(beads)
                .with_colors(colors)
                .with_radii([0.06]),
        )?;

        rec.log(
            "dna/structure",
            &rerun::archetypes::Transform3D::from_rotation(rerun::RotationAxisAngle::new(
                glam::Vec3::Z,
                rerun::Angle::from_radians(time / 4.0 * TAU),
            )),
        )?;
    }

    Ok(())
}
