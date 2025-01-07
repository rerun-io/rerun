//! TODO

use rand::{distributions::Uniform, Rng as _};

// TODO: we need to open an issue about this. and test and clean up all of that in general...

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-5., 5.);

    // Before
    #[cfg(TODO)]
    {
        let rec =
        // rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates").spawn()?;
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates_before").stdout()?;

        rec.set_time_sequence("frame", 0);
        rec.log(
            "points",
            &rerun::Points3D::new(
                (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
            ),
        )?;

        for i in 0..10 {
            let colors: Vec<rerun::components::Color> = (0..10)
                .map(|n| {
                    if n < i {
                        rerun::Color::from_rgb(20, 200, 20)
                    } else {
                        rerun::Color::from_rgb(200, 20, 20)
                    }
                })
                .collect();
            let radii: Vec<rerun::components::Radius> = (0..10)
                .map(|n| if n < i { 0.6 } else { 0.2 })
                .map(Into::into)
                .collect();

            rec.set_time_sequence("frame", i);
            rec.log("points", &[&colors as &dyn rerun::ComponentBatch, &radii])?;
        }
    }

    // After
    {
        let rec =
        // rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates").spawn()?;
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates_after").stdout()?;

        let positions = (0..10)
            .map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist)))
            .collect::<Vec<_>>();

        rec.set_time_sequence("frame", 0);
        rec.log("points", &rerun::Points3D::new(&positions))?;

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
                .with_positions(positions)
                .with_radii([0.3]),
        )?;
    }

    Ok(())
}
