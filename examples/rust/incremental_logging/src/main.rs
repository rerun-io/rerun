//! Showcase how to incrementally log data belonging to the same archetype, and re-use some or all
//! of it across frames.
//!
//! Usage:
//! ```
//! cargo run -p incremental -- --help
//! ```

use rand::{distributions::Uniform, Rng as _};
use rerun::external::re_log;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_incremental_logging")?;
    run(&rec)
}

const README: &str = r#"
# Incremental Logging

This example showcases how to incrementally log data belonging to the same archetype, and re-use some or all of it across frames.

It was logged with the following code:
```rust
let colors = [rerun::Color::from_rgb(255, 0, 0); 10];
let radii = [rerun::Radius(0.1); 10];

// Only log colors and radii once.
rec.set_time_sequence("frame_nr", 0);
rec.log_component_batches("points", false, /* timeless */ [&colors as &dyn rerun::ComponentBatch, &radii])?;

let mut rng = rand::thread_rng();
let dist = Uniform::new(-5., 5.);

// Then log only the points themselves each frame.
//
// They will automatically re-use the colors and radii logged at the beginning.
for i in 0..10 {
    rec.set_time_sequence("frame_nr", i);
    rec.log("points", &rerun::Points3D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist)))))?;
}
```

Move the time cursor around, and notice how the colors and radii from frame 0 are still picked up by later frames, while the points themselves keep changing every frame.
"#;

fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    rec.log_timeless(
        "readme",
        &rerun::TextDocument::new(README).with_media_type(rerun::MediaType::MARKDOWN),
    )?;

    // TODO(#5264): just log one once clamp-to-edge semantics land.
    let colors = [rerun::Color::from_rgb(255, 0, 0); 10];
    let radii = [rerun::Radius(0.1); 10];

    // Only log colors and radii once.
    rec.set_time_sequence("frame_nr", 0);
    rec.log_component_batches(
        "points",
        false, /* timeless */
        [&colors as &dyn rerun::ComponentBatch, &radii],
    )?;
    // Logging timelessly would also work.
    // rec.log_component_batches(
    //     "points",
    //     true, /* timeless */
    //     [&colors as &dyn rerun::ComponentBatch, &radii],
    // )?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-5., 5.);

    // Then log only the points themselves each frame.
    //
    // They will automatically re-use the colors and radii logged at the beginning.
    for i in 0..10 {
        rec.set_time_sequence("frame_nr", i);

        rec.log(
            "points",
            &rerun::Points3D::new(
                (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
            ),
        )?;
    }

    Ok(())
}
