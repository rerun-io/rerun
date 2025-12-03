use std::fs;
use std::process::Command;

use indicatif::MultiProgress;
use rayon::prelude::{IntoParallelIterator as _, ParallelIterator as _};

use super::{Channel, Example, wait_for_output};

/// Collect examples in the repository and run them to produce `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "notebook")]
pub struct Notebook {
    #[argh(option, description = "include only examples in this channel")]
    channel: Channel,

    #[argh(option, description = "run only these examples")]
    examples: Vec<String>,
}

impl Notebook {
    pub fn run(self) -> anyhow::Result<()> {
        let workspace_root = re_build_tools::cargo_metadata()?.workspace_root;
        let mut examples = if self.examples.is_empty() {
            self.channel.notebooks(workspace_root)?
        } else {
            Channel::Nightly
                .notebooks(workspace_root)?
                .into_iter()
                .filter(|example| self.examples.contains(&example.name))
                .collect()
        };
        examples.sort_by(|a, b| a.name.cmp(&b.name));

        let progress = MultiProgress::new();
        let results: Vec<anyhow::Result<()>> = examples
            .into_par_iter()
            .map(|example| example.build_notebook(&progress))
            .collect();

        let mut num_failed = 0;
        for result in results {
            match result {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("{err}");
                    num_failed += 1;
                }
            }
        }
        if 0 < num_failed {
            anyhow::bail!("Failed to run {num_failed} example(s)");
        }

        Ok(())
    }
}

impl Example {
    fn build_notebook(self, progress: &MultiProgress) -> anyhow::Result<()> {
        let tempdir = tempfile::tempdir()?;

        // Gather all files in self.dir with self.language.extension()
        let extension = self.language.extension();
        let entries = fs::read_dir(&self.dir)?;
        let notebook_files: Vec<_> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() && path.extension()? == extension {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        for notebook_path in notebook_files {
            let mut cmd = Command::new("jupyter");
            cmd.arg("nbconvert");
            cmd.arg("--execute");
            cmd.arg("--to").arg("notebook");
            cmd.arg("--output-dir").arg(tempdir.path());
            cmd.arg(&notebook_path);

            if self.allow_warnings {
                cmd.env("PYTHONWARNINGS", "default");
            } else {
                // raise exception on warnings, e.g. when using a @deprecated function
                cmd.env("PYTHONWARNINGS", "error");
            }

            cmd.env("RERUN_PANIC_ON_WARN", "1"); // any logged warnings/errors should cause a failure
            cmd.env("RERUN_STRICT", "1"); // any misuse of the API should cause a failure
            // Don't crash on Jupyter deprecation warning.
            cmd.env("JUPYTER_PLATFORM_DIRS", "1"); // use platform dirs for jupyter config/cache

            wait_for_output(cmd, &self.name, progress)?;
        }

        Ok(())
    }
}
