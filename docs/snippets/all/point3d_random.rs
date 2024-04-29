//! Log some random points with color and radii.

use rand::{distributions::Uniform, Rng as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d_random")
        .save("/tmp/5686.rrd")?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-10., 10.);

    for i in 0..100 {
        rec.set_time_sequence("frame", i);
        rec.log(
            "random",
            &rerun::Points3D::new(
                (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
            )
            .with_colors((0..10).map(|_| rerun::Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
        )?;
    }

    Ok(())
}
