mod compact;
mod compare;
mod merge;
mod print;

use self::compact::CompactCommand;
use self::compare::CompareCommand;
use self::merge::MergeCommand;
use self::print::PrintCommand;

// ---

use anyhow::Context as _;
use clap::Subcommand;

#[derive(Debug, Clone, Subcommand)]
pub enum RrdCommands {
    /// Compares the data between 2 .rrd files, returning a successful shell exit code if they
    /// match.
    ///
    /// This ignores the `log_time` timeline.
    Compare(CompareCommand),

    /// Print the contents of one or more .rrd/.rbl files.
    ///
    /// Example: `rerun rrd print /my/recordings/*.rrd`
    Print(PrintCommand),

    /// Compacts the contents of one or more .rrd/.rbl files and writes the result to a new file.
    ///
    /// Use the usual environment variables to control the compaction thresholds:
    /// `RERUN_CHUNK_MAX_ROWS`,
    /// `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED`,
    /// `RERUN_CHUNK_MAX_BYTES`.
    ///
    /// Example: `RERUN_CHUNK_MAX_ROWS=4096 RERUN_CHUNK_MAX_BYTES=1048576 rerun rrd compact /my/recordings/*.rrd -o output.rrd`
    Compact(CompactCommand),

    /// Merges the contents of multiple .rrd/.rbl files, and writes the result to a new file.
    ///
    /// This will not affect the chunking of the data in any way.
    ///
    /// Example: `rerun merge /my/recordings/*.rrd -o output.rrd`
    Merge(MergeCommand),
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
        }
    }
}
