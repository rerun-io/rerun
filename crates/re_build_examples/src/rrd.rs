use crate::wait_for_output;
use crate::{Channel, Example};
use indicatif::MultiProgress;
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::ParallelIterator;
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
        let examples = if self.examples.is_empty() {
            self.channel.examples(workspace_root)?
        } else {
            Channel::Nightly
                .examples(workspace_root)?
                .into_iter()
                .filter(|example| self.examples.contains(&example.name))
                .collect()
        };
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
        let rrd_path = output_dir.join(&self.name).with_extension("rrd");

        let mut cmd = Command::new("python3");
        cmd.arg(self.script_path);
        cmd.arg("--save").arg(&rrd_path);
        cmd.args(self.script_args);

        let final_args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        // Configure flushing so that:
        // * the resulting file size is deterministic
        // * the file is chunked into small batches for better streaming
        cmd.env("RERUN_FLUSH_TICK_SECS", 1_000_000_000.to_string());
        cmd.env("RERUN_FLUSH_NUM_BYTES", (128 * 1024).to_string());

        let output = wait_for_output(cmd, &self.name, progress)?;

        if output.status.success() {
            Ok(rrd_path)
        } else {
            anyhow::bail!(
                "Failed to run `python3 {}`: \
                \nstdout: \
                \n{} \
                \nstderr: \
                \n{}",
                final_args.join(" "),
                String::from_utf8(output.stdout)?,
                String::from_utf8(output.stderr)?,
            );
        }
    }
}
