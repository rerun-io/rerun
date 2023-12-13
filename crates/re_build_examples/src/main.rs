//! This build script collects all examples which should be part of our example page
//! and runs them to produce `.rrd` files.
//!
//! It identifies runnable examples by checking if they have `demo: true` set in
//! their `README.md` frontmatter.
//! An example may also specify args to be run with via the frontmatter
//! `build_args` string array.

use std::fs::create_dir_all;
use std::io::stdout;
use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;
use std::process::Output;
use std::time::Duration;

use indicatif::MultiProgress;
use indicatif::ProgressBar;
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::ParallelIterator;
use re_examples::{examples, Example};

const USAGE: &str = "\
Usage: [options] [output_dir]

Options:
    -h, --help   Print help
";

fn main() -> anyhow::Result<()> {
    re_build_tools::set_output_cargo_build_instructions(false);

    let args = Args::from_env();

    create_dir_all(&args.output_dir)?;

    let examples = examples()?;
    let progress = MultiProgress::new();
    let results: Vec<anyhow::Result<()>> = examples
        .into_par_iter()
        .map(|example| example.build(&progress, &args.output_dir))
        .collect();

    let mut failed = false;
    for result in results {
        if let Err(err) = result {
            eprintln!("{err}");
            failed = true;
        }
    }
    if failed {
        anyhow::bail!("Failed to run some examples");
    }

    Ok(())
}

struct Args {
    output_dir: PathBuf,
}

impl Args {
    fn from_env() -> Self {
        let mut output_dir = None;

        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!("{USAGE}");
                    exit(1);
                }
                _ if arg.starts_with('-') => {
                    eprintln!("Unknown argument: {arg:?}");
                    println!("\n{USAGE}");
                    exit(1);
                }
                _ if output_dir.is_some() => {
                    eprintln!("Too many positional arguments");
                    println!("\n{USAGE}");
                    exit(1);
                }
                _ => output_dir = Some(PathBuf::from(arg)),
            }
        }

        let Some(output_dir) = output_dir else {
            eprintln!("Missing argument \"output_dir\"");
            exit(1);
        };

        Args { output_dir }
    }
}

trait Build {
    fn build(self, progress: &MultiProgress, output_dir: &Path) -> anyhow::Result<()>;
}

impl Build for Example {
    fn build(self, progress: &MultiProgress, output_dir: &Path) -> anyhow::Result<()> {
        let mut cmd = Command::new("python3");
        cmd.arg(self.script_path);
        cmd.arg("--save")
            .arg(output_dir.join(&self.name).with_extension("rrd"));
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

        if !output.status.success() {
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

        Ok(())
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
