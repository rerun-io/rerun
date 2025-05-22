mod compare;
mod filter;
mod merge_compact;
mod migrate;
mod print;
mod verify;

use self::{
    compare::CompareCommand,
    filter::FilterCommand,
    merge_compact::{CompactCommand, MergeCommand},
    migrate::MigrateCommand,
    print::PrintCommand,
    verify::VerifyCommand,
};

// ---

use anyhow::Context as _;
use clap::Subcommand;

/// Manipulate the contents of .rrd and .rbl files.
#[derive(Debug, Clone, Subcommand)]
pub enum RrdCommands {
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
    /// This will not affect the chunking of the data in any way.
    ///
    /// Example: `rerun rrd merge /my/recordings/*.rrd > output.rrd`
    Merge(MergeCommand),

    /// Migrate one or more .rrd files to the newest Rerun version.
    ///
    /// Example: `rerun rrd migrate foo.rrd`
    /// Results in a `foo.backup.rrd` (copy of the old file) and a new `foo.rrd` (migrated).
    Migrate(MigrateCommand),

    /// Print the contents of one or more .rrd/.rbl files/streams.
    ///
    /// Reads from standard input if no paths are specified.
    ///
    /// Example: `rerun rrd print /my/recordings/*.rrd`
    Print(PrintCommand),

    /// Verify the that the .rrd file can be loaded and correctly interpreted.
    ///
    /// Can be used to ensure that the current Rerun version can load the data.
    Verify(VerifyCommand),
}

impl RrdCommands {
    pub fn run(&self) -> anyhow::Result<()> {
        match self {
            Self::Compare(cmd) => {
                cmd.run()
                    // Print current directory, this can be useful for debugging issues with relative paths.
                    .with_context(|| format!("current directory {:?}", std::env::current_dir()))
            }
            Self::Compact(cmd) => cmd.run(),
            Self::Filter(cmd) => cmd.run(),
            Self::Merge(cmd) => cmd.run(),
            Self::Migrate(cmd) => cmd.run(),
            Self::Print(cmd) => cmd.run(),
            Self::Verify(cmd) => cmd.run(),
        }
    }
}
