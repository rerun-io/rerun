//! The example from our Getting Started page.

use std::f32::consts::TAU;

use itertools::Itertools;

use rerun::{
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
    AsComponents as _, TimeColumn,
};

const NUM_POINTS: usize = 100;

fn main2() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates_columns")
        .spawn()?;

    let positions = || (0..10).map(|i| (i as f32, 0.0, 0.0));

    // TODO: well, I guess that explains why we don't have this...
    // TODO: so step one
    {
        let frames = TimeColumn::new_sequence("frame", positions().map(|_| 0i64));
        let positions = rerun::Points3D::update_fields().with_positions(positions());

        rec.send_columns_v2("points", [frames], positions.as_serialized_batches())?;
    }

    rec.set_time_sequence("frame", 0);
    rec.log("points", &rerun::Points3D::new(positions()))?;

    for i in 0..10 {
        let colors = (0..10).map(|n| {
            if n < i {
                rerun::Color::from_rgb(20, 200, 20)
            } else {
                rerun::Color::from_rgb(200, 20, 20)
            }
        });
        let radii = (0..10).map(|n| if n < i { 0.6 } else { 0.2 });

        rec.set_time_sequence("frame", i);
        rec.log(
            "points",
            &rerun::Points3D::update_fields()
                .with_radii(radii)
                .with_colors(colors),
        )?;
    }

    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        &rerun::Points3D::clear_fields()
            .with_positions(positions())
            .with_radii([0.3]),
    )?;

    Ok(())
}

fn main3() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_send_columns_arrays").spawn()?;

    // Prepare a point cloud that evolves over time 5 timesteps, changing the number of points in the process.
    let times = TimeColumn::new_seconds("time", 10..15);
    let positions = [
        vec![
            [1.0, 0.0, 1.0], //
            [0.5, 0.5, 2.0],
        ],
        vec![
            [1.5, -0.5, 1.5],
            [1.0, 1.0, 2.5],
            [-0.5, 1.5, 1.0],
            [-1.5, 0.0, 2.0],
        ],
        vec![
            [2.0, 0.0, 2.0],
            [1.5, -1.5, 3.0],
            [0.0, -2.0, 2.5],
            [1.0, -1.0, 3.5],
        ],
        vec![
            [-2.0, 0.0, 2.0], //
            [-1.5, 1.5, 3.0],
            [-1.0, 1.0, 3.5],
        ],
        vec![
            [1.0, -1.0, 1.0],
            [2.0, -2.0, 2.0],
            [3.0, -1.0, 3.0],
            [2.0, 0.0, 4.0],
        ],
    ]
    .into_iter()
    .flatten()
    .collect_vec();

    // At each time stamp, all points in the cloud share the same but changing color.
    let colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF];

    // rerun::Points3D::
    rec.send_columns_v2("points", [times], [])?;

    for i in 0..10 {
        let colors = (0..10).map(|n| {
            if n < i {
                rerun::Color::from_rgb(20, 200, 20)
            } else {
                rerun::Color::from_rgb(200, 20, 20)
            }
        });
        let radii = (0..10).map(|n| if n < i { 0.6 } else { 0.2 });

        rec.set_time_sequence("frame", i);
        rec.log(
            "points",
            &rerun::Points3D::update_fields()
                .with_radii(radii)
                .with_colors(colors),
        )?;
    }

    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        &rerun::Points3D::clear_fields()
            .with_positions(positions())
            .with_radii([0.3]),
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus").spawn()?;

    let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
    let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

    // rec.send_columns(ent_path, timelines, components);

    rec.set_time_seconds("stable_time", 0f64);

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

    let points_interleaved: Vec<[glam::Vec3; 2]> = points1
        .into_iter()
        .interleave(points2)
        .chunks(2)
        .into_iter()
        .map(|chunk| chunk.into_iter().collect_vec().try_into().unwrap())
        .collect_vec();

    rec.log_static(
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
            &rerun::archetypes::Transform3D::from_rotation(rerun::RotationAxisAngle::new(
                glam::Vec3::Z,
                rerun::Angle::from_radians(time / 4.0 * TAU),
            )),
        )?;
    }

    Ok(())
}
