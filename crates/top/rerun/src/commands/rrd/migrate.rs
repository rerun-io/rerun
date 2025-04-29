use std::time::Duration;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use indicatif::ProgressBar;
use itertools::Itertools as _;
use rayon::prelude::*;
use re_build_info::CrateVersion;
use re_log_encoding::EncodingOptions;

#[derive(Debug, Clone, clap::Parser)]
pub struct MigrateCommand {
    /// Paths to rrd files to migrate
    path_to_input_rrds: Vec<Utf8PathBuf>,
}

impl MigrateCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            mut path_to_input_rrds,
        } = self.clone();

        let num_files_before = path_to_input_rrds.len();

        path_to_input_rrds.retain(|f| !f.to_string().ends_with(".backup.rrd"));

        let num_files = path_to_input_rrds.len();

        if num_files < num_files_before {
            eprintln!("Ignored {} file(s) that are called .backup.rrd, and are therefore assumed to already have been migrated", num_files_before - num_files);
        }

        // Sanity-check input:
        for path in &path_to_input_rrds {
            anyhow::ensure!(path.exists(), "No such file: {path}");
        }

        eprintln!("Migrating {num_files} .rrd file(s)…");

        let progress =
            ProgressBar::new(path_to_input_rrds.len() as u64).with_message("Migrating rrd:s");
        progress.enable_steady_tick(Duration::from_millis(500));

        let failures: Vec<(Utf8PathBuf, anyhow::Error)> = path_to_input_rrds
            .par_iter()
            .filter_map(|path| {
                if let Err(err) = migrate_file(path) {
                    progress.inc(1);
                    Some((path.clone(), err))
                } else {
                    progress.inc(1);
                    None
                }
            })
            .collect();

        progress.finish_and_clear();

        if failures.is_empty() {
            eprintln!(
                "✅ {} file(s) successfully migrated.",
                path_to_input_rrds.len()
            );
            Ok(())
        } else {
            let num_failures = failures.len();
            eprintln!("❌ Failed to migrate {num_failures}/{num_files} file(s):");
            eprintln!();
            for (path, err) in &failures {
                eprintln!("  {path}: {err}\n");
            }
            anyhow::bail!("Failed to migrate {num_failures}/{num_files} file(s)");
        }
    }
}

fn migrate_file(original_path: &Utf8PathBuf) -> anyhow::Result<()> {
    // Rename `old_name.rrd` to `old_name.backup.rrd`:
    let backup_path = original_path.with_extension("backup.rrd");

    if backup_path.exists() {
        anyhow::bail!("Aborting migration of {original_path}: {backup_path} already exists");
    }

    std::fs::rename(original_path, &backup_path)
        .with_context(|| format!("Couldn't rename {original_path:?} to {backup_path:?}"))?;

    // Stream convert it:
    let old_file = std::fs::File::open(&backup_path)
        .with_context(|| format!("Failed to open {backup_path:?}"))?;

    let decoder = re_log_encoding::decoder::Decoder::new(std::io::BufReader::new(old_file))
        .with_context(|| format!("Failed to decode {original_path:?}"))?;

    let mut errors = indexmap::IndexSet::new();

    let messages = decoder.into_iter().filter_map(|result| match result {
        Ok(msg) => Some(Ok(msg)),
        Err(err) => {
            errors.insert(err.to_string());
            None
        }
    });

    let new_file = std::fs::File::create(original_path)
        .with_context(|| format!("Failed to create {original_path:?}"))?;

    let mut buffered_writer = std::io::BufWriter::new(new_file);

    re_log_encoding::encoder::encode(
        CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        messages,
        &mut buffered_writer,
    )
    .with_context(|| format!("Failed to write new .rrd file to {original_path:?}"))?;

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.iter().join("\n"))
    }
}
