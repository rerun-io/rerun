use std::io::{IsTerminal as _, stdout};
use std::process::Command;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};

/// Returns an error on non-zero returncode.
pub fn wait_for_output(
    mut cmd: Command,
    name: &str,
    progress: &MultiProgress,
) -> anyhow::Result<()> {
    // Remember what we tried to run, for a better error message:
    let program = cmd.get_program().to_string_lossy().to_string();
    let args = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

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

    if !output.status.success() {
        let args = args.join(" ");
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        anyhow::bail!(
            "Failed to run `{program} {args}`: \
                \nstdout: \
                \n{stdout} \
                \nstderr: \
                \n{stderr}",
        );
    }

    Ok(())
}
