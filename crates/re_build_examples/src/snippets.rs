use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::create_dir_all;
use std::fs::read_dir;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use indicatif::MultiProgress;
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::ParallelIterator;

use crate::wait_for_output;

/// Collect code snippets from `docs/code-examples` in the repository and run them to produce `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "snippets")]
pub struct Snippets {
    #[argh(positional, description = "directory to output `rrd` files into")]
    output_dir: PathBuf,
}

impl Snippets {
    pub fn run(self) -> anyhow::Result<()> {
        create_dir_all(&self.output_dir)?;

        let snippets_dir = re_build_tools::cargo_metadata()?
            .workspace_root
            .join("docs/code-examples");

        println!("Reading config…");
        let config = read_to_string(snippets_dir.join("snippets.toml"))?;
        let config: Config = toml::from_str(&config)?;

        println!("Collecting snippets…");
        let mut snippets = vec![];
        for snippet in read_dir(snippets_dir.join("all"))? {
            let snippet = snippet?;
            let path = snippet.path();
            let name = path.file_stem().and_then(OsStr::to_str).unwrap();

            if !snippet.path().extension().is_some_and(|p| p == "py") {
                println!("Skipping {}: not a python example", path.display());
                continue;
            }

            if config.opt_out.run.contains_key(name) {
                println!(
                    "Skipping {}: explicit opt-out in `snippets.toml`",
                    path.display()
                );
                continue;
            }

            println!("Adding {}", path.display());
            snippets.push(Snippet {
                extra_args: config.extra_args.get(name).cloned().unwrap_or_default(),
                name: name.to_owned(),
                path,
            });
        }

        println!("Running {} snippets…", snippets.len());
        let progress = MultiProgress::new();
        let results: Vec<anyhow::Result<PathBuf>> = snippets
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

#[derive(Debug)]
struct Snippet {
    path: PathBuf,
    name: String,
    extra_args: Vec<String>,
}

impl Snippet {
    fn build(self, progress: &MultiProgress, output_dir: &Path) -> anyhow::Result<PathBuf> {
        let rrd_path = output_dir.join(&self.name).with_extension("rrd");

        let mut cmd = Command::new("python3");
        cmd.arg(&self.path);
        cmd.args(&self.extra_args);

        let final_args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        cmd.envs([
            ("RERUN_FLUSH_NUM_ROWS", "0"),
            ("RERUN_STRICT", "1"),
            ("RERUN_PANIC_ON_WARN", "1"),
            (
                "_RERUN_TEST_FORCE_SAVE",
                rrd_path.to_string_lossy().as_ref(),
            ),
        ]);

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

/// See `docs/code-examples/snippets.toml` for more info
#[derive(serde::Deserialize)]
struct Config {
    opt_out: OptOut,

    /// example name -> args
    extra_args: HashMap<String, Vec<String>>,
}

#[derive(serde::Deserialize)]
struct OptOut {
    /// example name -> languages
    run: HashMap<String, Vec<String>>,
}
