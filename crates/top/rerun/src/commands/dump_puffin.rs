use std::path::PathBuf;

/// Dump a puffin profiler recording (`.puffin` file) as JSON on stdout.
///
/// The output can be large; redirect it to a file and query it with e.g. `jq`.
#[derive(Debug, Clone, clap::Parser)]
pub struct DumpPuffinCommand {
    /// Path to the `.puffin` file to dump.
    path: PathBuf,
}

impl DumpPuffinCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        re_dump_puffin::dump_to_json_writer(&self.path, std::io::stdout().lock())
    }
}
