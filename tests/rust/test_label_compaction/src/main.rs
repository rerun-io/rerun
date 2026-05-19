//! Test for label compaction in 3D spatial views.
//!
//! Logs many labeled 3D points to test that multi-line labels
//! are compacted to their first line when there are many on screen.
//!
//! ## Usage
//! ```
//! cargo r -p test_label_compaction
//! ```

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

    let (rec, _serve_guard) = args.rerun.init("rerun_example_label_compaction")?;

    // 20 points with multi-line labels, arranged in a grid
    for i in 0..20 {
        let x = (i % 5) as f32 * 3.0;
        let y = (i / 5) as f32 * 3.0;
        rec.log(
            format!("points/{i}"),
            &rerun::Points3D::new([(x, y, 0.0)]).with_labels([format!(
                "Point {i}\n\
                 Position: ({x:.1}, {y:.1}, 0.0)\n\
                 Status: active\n\
                 Category: test-label-{}\n\
                 Priority: {}",
                i % 4,
                ["low", "medium", "high", "critical"][i as usize % 4],
            )]),
        )?;
    }

    Ok(())
}
