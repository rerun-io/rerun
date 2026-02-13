//! Showcase how to incrementally log data belonging to the same archetype, and re-use some or all
//! of it across frames.
//!
//! Usage:
//! ```
//! cargo run -p incremental -- --help
//! ```

use rand::Rng as _;
use rand::distr::Uniform;
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
// Only log colors and radii once.
// Logging statically would also work (i.e. `log_static`).
rec.set_time_sequence("frame_nr", 0);
rec.log(
    "points",
    &rerun::Points3D::update_fields()
        .with_colors([(255, 0, 0)])
        .with_radii([0.1]),
)?;

let mut rng = rand::rng();
let dist = Uniform::new(-5., 5.)?;

// Then log only the points themselves each frame.
//
// They will automatically re-use the colors and radii logged at the beginning.
for i in 0..10 {
    rec.set_time_sequence("frame_nr", i);

    rec.log(
        "points",
        &rerun::Points3D::update_fields().with_positions(
            (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
        ),
    )?;
}
```

Move the time cursor around, and notice how the colors and radii from frame 0 are still picked up by later frames, while the points themselves keep changing every frame.
"#;

fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    rec.log_static("readme", &rerun::TextDocument::from_markdown(README))?;

    // Only log colors and radii once.
    // Logging statically would also work (i.e. `log_static`).
    rec.set_time_sequence("frame_nr", 0);
    rec.log(
        "points",
        &rerun::Points3D::update_fields()
            .with_colors([(255, 0, 0)])
            .with_radii([0.1]),
    )?;

    let mut rng = rand::rng();
    let dist = Uniform::new(-5., 5.)?;

    // Then log only the points themselves each frame.
    //
    // They will automatically re-use the colors and radii logged at the beginning.
    for i in 0..10 {
        rec.set_time_sequence("frame_nr", i);

        rec.log(
            "points",
            &rerun::Points3D::update_fields().with_positions(
                (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
            ),
        )?;
    }

    Ok(())
}
