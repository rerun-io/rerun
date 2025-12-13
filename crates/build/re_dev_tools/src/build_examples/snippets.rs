use std::collections::HashMap;
use std::fs::{create_dir_all, read_to_string};
use std::path::{Path, PathBuf};
use std::process::Command;

use camino::Utf8Path;
use indicatif::MultiProgress;
use rayon::prelude::{IntoParallelIterator as _, ParallelIterator as _};

use super::wait_for_output;

/// Collect code snippets from `docs/snippets` in the repository and run them to produce `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "snippets")]
pub struct Snippets {
    #[argh(positional, description = "directory to output `rrd` files into")]
    output_dir: PathBuf,
}

fn install_snippet_deps() {
    // uv sync --inexact --no-install-package rerun-sdk --group snippets
    let mut cmd = Command::new("uv");
    cmd.arg("sync");
    cmd.arg("--inexact");
    cmd.arg("--no-install-package");
    cmd.arg("rerun-sdk");
    cmd.arg("--group");
    cmd.arg("snippets");

    let _ = cmd
        .status()
        .expect("failed to run `uv sync` to install snippet dependencies");
}

impl Snippets {
    pub fn run(self) -> anyhow::Result<()> {
        // Install snippet dependencies by running:
        install_snippet_deps();

        create_dir_all(&self.output_dir)?;

        let snippets_dir = re_build_tools::cargo_metadata()?
            .workspace_root
            .join("docs/snippets");

        println!("Reading config…");
        let config = read_to_string(snippets_dir.join("snippets.toml"))?;
        let config: Config = toml::from_str(&config)?;

        println!("Collecting snippets…");
        let snippet_root = snippets_dir.join("all");
        let snippets = collect_snippets_recursively(&snippet_root, &config, &snippet_root)?;

        // Check for duplicates as this will lead to undefined behavior (multiple threads writing
        // to the same file).
        {
            let mut deduped = std::collections::HashSet::new();
            for snippet in &snippets {
                if !deduped.insert(snippet.name.as_str()) {
                    anyhow::bail!("Snippet '{}' is defined multiple times", snippet.name);
                }
            }
        }

        let progress = MultiProgress::new();

        println!("Running {} snippets…", snippets.len());
        let results: Vec<anyhow::Result<PathBuf>> = snippets
            .into_par_iter()
            .map(|example| example.build(&progress, &self.output_dir))
            .collect();

        let mut num_failed = 0;
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
                        num_failed += 1;
                    }
                }
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

fn collect_snippets_recursively(
    dir: &Utf8Path,
    config: &Config,
    snippet_root_path: &Utf8Path,
) -> anyhow::Result<Vec<Snippet>> {
    let mut snippets = vec![];

    #[expect(clippy::unwrap_used)] // we just use unwrap for string <-> path conversion here
    for snippet in dir.read_dir()? {
        let snippet = snippet?;
        let meta = snippet.metadata()?;
        let path = snippet.path();

        if path.file_name().is_some_and(|p| p == "__init__.py") {
            continue;
        }

        // Compare snippet outputs sometimes leaves orphaned rrd files.
        if path.extension().is_some_and(|p| p == "rrd") {
            continue;
        }

        let name = path
            .strip_prefix(snippet_root_path)?
            .with_extension("")
            .to_string_lossy()
            .to_string();
        let config_key = name.replace('\\', "/");

        let is_opted_out = config
            .opt_out
            .run
            .get(&config_key)
            .is_some_and(|languages| languages.iter().any(|v| v == "py"));
        if is_opted_out {
            println!(
                "Skipping {}: explicit opt-out in `snippets.toml`",
                path.display()
            );
            continue;
        }

        if meta.is_dir() {
            snippets.extend(
                collect_snippets_recursively(
                    Utf8Path::from_path(&path).unwrap(),
                    config,
                    snippet_root_path,
                )?
                .into_iter(),
            );
            continue;
        }

        // We only run python examples, because:
        // - Each snippet should already be available in each language
        // - Python is the easiest to run
        if path.extension().is_none_or(|p| p != "py") {
            println!("Skipping {}: not a python example", path.display());
            continue;
        }

        println!("Adding {}", path.display());
        let extra_args: Vec<String> = config
            .extra_args
            .get(&config_key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| value.replace("$config_dir", snippet_root_path.parent().unwrap().as_str()))
            .collect();
        snippets.push(Snippet {
            path,
            name,
            extra_args,
        });
    }

    Ok(snippets)
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

        if let Some(dir) = rrd_path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let mut cmd = Command::new("python3");
        cmd.arg(&self.path);
        cmd.args(&self.extra_args);

        cmd.envs([
            ("PYTHONWARNINGS", "error"), // raise exception on warnings, e.g. when using a @deprecated function
            ("RERUN_FLUSH_NUM_ROWS", "0"),
            ("RERUN_STRICT", "1"),
            ("RERUN_PANIC_ON_WARN", "1"),
            (
                "_RERUN_TEST_FORCE_SAVE",
                rrd_path.to_string_lossy().as_ref(),
            ),
        ]);

        wait_for_output(cmd, &self.name, progress)?;

        Ok(rrd_path)
    }
}

/// See `docs/snippets/snippets.toml` for more info
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
