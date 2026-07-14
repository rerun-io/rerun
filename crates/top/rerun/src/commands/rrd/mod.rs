mod compare;
mod filter;
mod merge_optimize;
mod migrate;
mod print;
mod route;
mod split;
mod stats;
mod verify;

// ---
use anyhow::Context as _;
use clap::Subcommand;

use self::compare::CompareCommand;
use self::filter::FilterCommand;
use self::merge_optimize::{CompactCommand, MergeCommand, OptimizeCommand};
use self::migrate::MigrateCommand;
use self::print::PrintCommand;
use self::route::RouteCommand;
use self::split::SplitCommand;
use self::stats::StatsCommand;
use self::verify::VerifyCommand;

/// Manipulate the contents of .rrd and .rbl files.
#[derive(Debug, Clone, Subcommand)]
pub enum RrdCommands {
    /// Compares the data between 2 .rrd files, returning a successful shell exit code if they
    /// match.
    ///
    /// This ignores the `log_time` timeline.
    Compare(CompareCommand),

    /// Filters out data from .rrd/.rbl files/streams, and writes the result to standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// This will not affect the chunking of the data in any way.
    ///
    /// Example: `rerun rrd filter --drop-timeline log_tick /my/recordings/*.rrd > output.rrd`
    Filter(FilterCommand),

    /// Merges the contents of multiple .rrd/.rbl files/streams, and writes the result to standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// ⚠️ This will automatically migrate the data to the latest version of the RRD protocol, if needed. ⚠️
    ///
    /// Example: `rerun rrd merge /my/recordings/*.rrd > output.rrd`
    Merge(MergeCommand),

    /// Migrate one or more .rrd files to the newest Rerun version.
    ///
    /// Example: `rerun rrd migrate foo.rrd`
    /// Results in a `foo.backup.rrd` (copy of the old file) and a new `foo.rrd` (migrated).
    Migrate(MigrateCommand),

    /// Optimizes the contents of one or more .rrd/.rbl files/streams by compacting chunks, and writes the result to standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// If any input is a directory, the command switches to **directory mirror mode**:
    /// every `.rrd`/`.rbl` file under the input is optimized independently, and written
    /// to the output path while preserving the input folder structure. In this mode the
    /// output (`-o`) must be set and is treated as a directory root.
    ///
    /// Uses the usual environment variables to control the compaction thresholds:
    /// `RERUN_CHUNK_MAX_ROWS`,
    /// `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED`,
    /// `RERUN_CHUNK_MAX_BYTES`.
    ///
    /// Unless explicit flags are passed, in which case they will override environment values.
    ///
    /// Video stream chunks are also rebatched on GoP (keyframe) boundaries so that each
    /// chunk holds one or more complete GoPs. Pass `--no-rebatch-videos` to disable that.
    ///
    /// ⚠️ This will automatically migrate the data to the latest version of the RRD protocol, if needed. ⚠️
    ///
    /// Examples:
    ///
    /// * Optimize a single recording into one optimized file (`-o`):
    ///   `rerun rrd optimize my.rrd -o my-compacted.rrd`
    ///
    /// * Merge many recordings into one optimized file:
    ///   `rerun rrd optimize --max-size 2MiB /my/recordings/*.rrd -o output.rrd`
    ///
    /// * Pipe through stdin/stdout, overriding both row and size thresholds:
    ///   `cat my.rrd | rerun rrd optimize --max-rows 4096 --max-size 2MiB > output.rrd`
    ///
    /// * Directory mirror mode — optimize every `.rrd`/`.rbl` under a tree, preserving structure:
    ///   `rerun rrd optimize --max-size 2MiB /my/recordings -o /my/recordings-compacted`
    Optimize(OptimizeCommand),

    /// Deprecated: renamed to `optimize`.
    #[command(hide = true)]
    Compact(CompactCommand),

    /// Print the contents of one or more .rrd/.rbl files/streams.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// Example: `rerun rrd print /my/recordings/*.rrd`
    Print(PrintCommand),

    /// Manipulates the metadata of log message streams without decoding the payloads.
    ///
    /// This can be used to combine multiple .rrd files into a single recording.
    /// Example: `rerun rrd route --recording-id my_recording /my/recordings/*.rrd > output.rrd`
    ///
    /// Note: Because the payload of the messages is never decoded, no migration or verification will performed.
    Route(RouteCommand),

    /// Optimally splits a recording on a specified timeline.
    ///
    /// The sum of the generated splits will always exactly match the original recording.
    ///
    /// Example: `rerun rrd split --output-dir ./splits --timeline log_tick --time 33 --time 66 ./my_video.rrd`
    Split(SplitCommand),

    /// Compute important statistics for one or more .rrd/.rbl files/streams.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// Example: `rerun rrd stats /my/recordings/*.rrd`
    Stats(StatsCommand),

    /// Verify the that the .rrd file can be loaded and correctly interpreted.
    ///
    /// Can be used to ensure that the current Rerun version can load the data.
    Verify(VerifyCommand),
}

impl RrdCommands {
    pub fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Compare(cmd) => {
                cmd.run()
                    // Print current directory, this can be useful for debugging issues with relative paths.
                    .with_context(|| format!("current directory {:?}", std::env::current_dir()))
            }
            Self::Optimize(cmd) => cmd.run(),
            Self::Compact(cmd) => cmd.run(),
            Self::Filter(cmd) => cmd.run(),
            Self::Split(cmd) => cmd.run(),
            Self::Merge(cmd) => cmd.run(),
            Self::Migrate(cmd) => cmd.run(),
            Self::Print(cmd) => cmd.run(),
            Self::Route(cmd) => cmd.run(),
            Self::Stats(cmd) => cmd.run(),
            Self::Verify(cmd) => cmd.run(),
        }
    }
}
