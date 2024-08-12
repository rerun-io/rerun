mod compare;
mod filter;
mod merge_compact;
mod print;

use self::compare::CompareCommand;
use self::filter::FilterCommand;
use self::merge_compact::{CompactCommand, MergeCommand};
use self::print::PrintCommand;

// ---

use anyhow::Context as _;
use clap::Subcommand;

/// Manipulate the contents of .rrd and .rbl files.
#[derive(Debug, Clone, Subcommand)]
pub enum RrdCommands {
    /// Compares the data between 2 .rrd files, returning a successful shell exit code if they
    /// match.
    ///
    /// This ignores the `log_time` timeline.
    Compare(CompareCommand),

    /// Print the contents of one or more .rrd/.rbl files/streams.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// Example: `rerun rrd print /my/recordings/*.rrd`
    Print(PrintCommand),

    /// Compacts the contents of one or more .rrd/.rbl files/streams and writes the result standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// Uses the usual environment variables to control the compaction thresholds:
    /// `RERUN_CHUNK_MAX_ROWS`,
    /// `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED`,
    /// `RERUN_CHUNK_MAX_BYTES`.
    ///
    /// Unless explicit flags are passed, in which case they will override environment values.
    ///
    /// Examples:
    ///
    /// * `RERUN_CHUNK_MAX_ROWS=4096 RERUN_CHUNK_MAX_BYTES=1048576 rerun rrd compact /my/recordings/*.rrd -o output.rrd`
    ///
    /// * `rerun rrd compact --max-rows 4096 --max-bytes=1048576 /my/recordings/*.rrd > output.rrd`
    Compact(CompactCommand),

    /// Merges the contents of multiple .rrd/.rbl files/streams, and writes the result to standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// This will not affect the chunking of the data in any way.
    ///
    /// Example: `rerun merge /my/recordings/*.rrd > output.rrd`
    Merge(MergeCommand),

    /// Filters out data from .rrd/.rbl files/streams, and writes the result to standard output.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// This will not affect the chunking of the data in any way.
    ///
    /// Example: `rerun filter --drop-timeline log_tick /my/recordings/*.rrd > output.rrd`
    Filter(FilterCommand),
}

impl RrdCommands {
    pub fn run(&self) -> anyhow::Result<()> {
        match self {
            Self::Compare(compare_command) => {
                compare_command
                    .run()
                    // Print current directory, this can be useful for debugging issues with relative paths.
                    .with_context(|| format!("current directory {:?}", std::env::current_dir()))
            }
            Self::Print(print_command) => print_command.run(),
            Self::Compact(compact_command) => compact_command.run(),
            Self::Merge(merge_command) => merge_command.run(),
            Self::Filter(drop_command) => drop_command.run(),
        }
    }
}
