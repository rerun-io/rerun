//! Build rerun example RRD files and manifest

pub mod example;
pub mod manifest;
pub mod rrd;
pub mod snippets;

use std::io::stdout;
use std::io::IsTerminal;
use std::process::Command;
use std::process::Output;
use std::time::Duration;

pub use example::{Channel, Example};
use indicatif::MultiProgress;
use indicatif::ProgressBar;

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
