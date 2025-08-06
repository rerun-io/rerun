//! Logs a `VisibleTimeRanges` archetype for roundtrip checks.

use rerun::{
    RecordingStream,
    datatypes::{TimeInt, TimeRange, TimeRangeBoundary, VisibleTimeRange},
    external::re_types::blueprint::archetypes::VisibleTimeRanges,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "visible_time_ranges",
        &VisibleTimeRanges::new([
            VisibleTimeRange {
                timeline: "timeline0".into(),
                range: TimeRange {
                    start: TimeRangeBoundary::Infinite,
                    end: TimeRangeBoundary::CursorRelative(TimeInt(-10)),
                },
            },
            VisibleTimeRange {
                timeline: "timeline1".into(),
                range: TimeRange {
                    start: TimeRangeBoundary::CursorRelative(TimeInt(20)),
                    end: TimeRangeBoundary::Infinite,
                },
            },
            VisibleTimeRange {
                timeline: "timeline2".into(),
                range: TimeRange {
                    start: TimeRangeBoundary::Absolute(TimeInt(20)),
                    end: TimeRangeBoundary::Absolute(TimeInt(40)),
                },
            },
        ]),
    )?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args
        .rerun
        .init("rerun_example_roundtrip_visible_time_ranges")?;
    run(&rec, &args)
}
