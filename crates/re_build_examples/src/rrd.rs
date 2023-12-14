use crate::{Channel, Example};
use indicatif::MultiProgress;
use indicatif::ProgressBar;
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::ParallelIterator;
use std::fs::create_dir_all;
use std::io::stdout;
use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::time::Duration;

/// Collect examples in the repository and run them to produce `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "rrd")]
pub struct Rrd {
    #[argh(positional, description = "directory to output `rrd` files into")]
    output_dir: PathBuf,

    #[argh(option, description = "include only examples in this channel")]
    channel: Channel,
}

impl Rrd {
    pub fn run(self) -> anyhow::Result<()> {
        create_dir_all(&self.output_dir)?;

        let examples = self.channel.examples()?;
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

trait Build {
    /// Returns the path to the resulting `.rrd` file.
    fn build(self, progress: &MultiProgress, output_dir: &Path) -> anyhow::Result<PathBuf>;
}

impl Build for Example {
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

fn wait_for_output(
    mut cmd: Command,
    name: &str,
    progress: &MultiProgress,
) -> anyhow::Result<Output> {
    let progress = progress.add(ProgressBar::new_spinner().with_message(name.to_owned()));
    progress.enable_steady_tick(Duration::from_millis(100));

    let output = cmd.output()?;

    let elapsed = progress.elapsed().as_secs_f64();
    let tick = if output.status.success() {
        "✔"
    } else {
        "✘"
    };
    let message = format!("{tick} {name} ({elapsed:.3}s)");

    if stdout().is_terminal() {
        progress.set_message(message);
        progress.finish();
    } else {
        println!("{message}");
    }

    Ok(output)
}
