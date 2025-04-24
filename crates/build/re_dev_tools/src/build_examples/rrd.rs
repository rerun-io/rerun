use super::{wait_for_output, Channel, Example};
use indicatif::MultiProgress;
use rayon::prelude::IntoParallelIterator as _;
use rayon::prelude::ParallelIterator as _;
use std::fs::create_dir_all;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Collect examples in the repository and run them to produce `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "rrd")]
pub struct Rrd {
    #[argh(positional, description = "directory to output `rrd` files into")]
    output_dir: PathBuf,

    #[argh(option, description = "include only examples in this channel")]
    channel: Channel,

    #[argh(option, description = "run only these examples")]
    examples: Vec<String>,
}

impl Rrd {
    pub fn run(self) -> anyhow::Result<()> {
        create_dir_all(&self.output_dir)?;

        let workspace_root = re_build_tools::cargo_metadata()?.workspace_root;
        let mut examples = if self.examples.is_empty() {
            self.channel.examples(workspace_root)?
        } else {
            Channel::Nightly
                .examples(workspace_root)?
                .into_iter()
                .filter(|example| self.examples.contains(&example.name))
                .collect()
        };
        examples.sort_by(|a, b| a.name.cmp(&b.name));

        let progress = MultiProgress::new();
        let results: Vec<anyhow::Result<PathBuf>> = examples
            .into_par_iter()
            .map(|example| example.build(&progress, &self.output_dir))
            .collect();

        let mut failed = false;
        for result in results {
            match result {
                Ok(rrd_path) => {
                    if let Ok(metadata) = std::fs::metadata(&rrd_path) {
                        println!(
                            "Output: {} ({})",
                            rrd_path.display(),
                            re_format::format_bytes(metadata.len() as _)
                        );
                    } else {
                        eprintln!("Missing rrd at {}", rrd_path.display());
                        failed = true;
                    }
                }
                Err(err) => {
                    eprintln!("{err}");
                    failed = true;
                }
            }
        }
        if failed {
            anyhow::bail!("Failed to run some examples");
        }

        Ok(())
    }
}

impl Example {
    fn build(self, progress: &MultiProgress, output_dir: &Path) -> anyhow::Result<PathBuf> {
        let tempdir = tempfile::tempdir()?;

        let initial_rrd_path = tempdir.path().join(&self.name).with_extension("rrd");

        {
            let mut cmd = Command::new("python3");
            cmd.arg("-m").arg(&self.name);
            cmd.arg("--save").arg(&initial_rrd_path);
            cmd.args(self.script_args);

            // Configure flushing so that:
            // * the resulting file size is deterministic
            // * the file is chunked into small batches for better streaming
            cmd.env("RERUN_FLUSH_TICK_SECS", 1_000_000_000.to_string());
            cmd.env("RERUN_FLUSH_NUM_BYTES", (128 * 1024).to_string());

            cmd.env("PYTHONWARNINGS", "error"); // raise exception on warnings, e.g. when using a @deprecated function
            cmd.env("RERUN_PANIC_ON_WARN", "1"); // any logged warnings/errors should cause a failure
            cmd.env("RERUN_STRICT", "1"); // any misuse of the API should cause a failure

            wait_for_output(cmd, &self.name, progress)?;
        }

        // Now run compaction on the result:
        let final_rrd_path = output_dir.join(&self.name).with_extension("rrd");

        let mut cmd = Command::new("python3");
        cmd.arg("-m").arg("rerun");
        cmd.arg("rrd");
        cmd.arg("compact");
        // Small chunks for better streaming:
        cmd.arg("--max-bytes").arg((128 * 1024).to_string());
        cmd.arg(&initial_rrd_path);
        cmd.arg("-o").arg(&final_rrd_path);

        wait_for_output(cmd, &format!("{} compaction", self.name), progress)?;

        Ok(final_rrd_path)
    }
}
