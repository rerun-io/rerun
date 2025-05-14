use std::time::Duration;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use indicatif::ProgressBar;
use itertools::Itertools as _;
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};

use re_entity_db::EntityDb;
use re_sdk::StoreId;

use crate::commands::{read_rrd_streams_from_file_or_stdin, save_entity_dbs_to_rrd};

#[derive(Debug, Clone, clap::Parser)]
pub struct CompressVideo {
    /// Path to rrd files to migrate
    // TODO: allow folders
    path_to_input_rrds: Vec<Utf8PathBuf>,
}

impl CompressVideo {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            mut path_to_input_rrds,
        } = self.clone();

        let num_files_before = path_to_input_rrds.len();

        path_to_input_rrds.retain(|f| !f.to_string().ends_with(".backup.rrd"));

        let num_files = path_to_input_rrds.len();

        if num_files < num_files_before {
            eprintln!(
                "Ignored {} file(s) that are called .backup.rrd, and are therefore assumed to already have been compressed",
                num_files_before - num_files
            );
        }

        // Sanity-check input:
        for path in &path_to_input_rrds {
            anyhow::ensure!(path.exists(), "No such file: {path}");
        }

        eprintln!("Compressing images in {num_files} .rrd file(s) to videos…");

        let progress =
            ProgressBar::new(path_to_input_rrds.len() as u64).with_message("Migrating rrd:s");
        progress.enable_steady_tick(Duration::from_millis(500));

        let failures: Vec<(Utf8PathBuf, anyhow::Error)> = path_to_input_rrds
            .par_iter()
            .filter_map(|original_path| {
                if let Err(err) = video_compress_file_at(original_path) {
                    progress.inc(1);
                    Some((original_path.clone(), err))
                } else {
                    progress.inc(1);
                    None
                }
            })
            .collect();

        progress.finish_and_clear();

        if failures.is_empty() {
            eprintln!(
                // TODO: report how many files were just untouched and remove backups for those.
                "✅ {} file(s) successfully compressed.",
                path_to_input_rrds.len()
            );
            Ok(())
        } else {
            let num_failures = failures.len();
            eprintln!("❌ Failed to compress {num_failures}/{num_files} file(s):");
            eprintln!();
            for (path, err) in &failures {
                eprintln!("  {path}: {}\n", re_error::format(err));
            }
            anyhow::bail!("Failed to compress {num_failures}/{num_files} file(s)");
        }
    }
}

fn video_compress_file_at(original_path: &Utf8PathBuf) -> anyhow::Result<()> {
    // Rename `old_name.rrd` to `old_name.backup.rrd`:
    let backup_path = original_path.with_extension("backup.rrd");

    if backup_path.exists() {
        eprintln!("Ignoring compression of {original_path}: {backup_path} already exists");
        return Ok(());
    }

    std::fs::rename(original_path, &backup_path)
        .with_context(|| format!("Couldn't rename {original_path:?} to {backup_path:?}"))?;

    if let Err(err) = video_compress_from_to(&backup_path, original_path) {
        // Restore:
        std::fs::rename(&backup_path, original_path).ok();
        Err(err)
    } else {
        Ok(())
    }
}

/// Stream-convert an rrd file
fn video_compress_from_to(from_path: &Utf8PathBuf, to_path: &Utf8PathBuf) -> anyhow::Result<()> {
    let (rx, _rrd_in_size) = read_rrd_streams_from_file_or_stdin(&[from_path.clone()]);

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();

    let mut errors = indexmap::IndexSet::new();

    for (_source, res) in rx {
        match res {
            Ok(msg) => {
                if let Err(err) = entity_dbs
                    .entry(msg.store_id().clone())
                    .or_insert_with(|| re_entity_db::EntityDb::new(msg.store_id().clone()))
                    .add(&msg)
                {
                    errors.insert(err.to_string());
                }
            }

            Err(err) => {
                errors.insert(err.to_string());
            }
        }
    }

    // TODO: compress stuff

    // TODO: report size difference
    let _rrd_out_size = save_entity_dbs_to_rrd(Some(to_path), entity_dbs)?;

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.iter().join("\n"))
    }
}

fn video_compress_entity_db(entity_db: &EntityDb) -> anyhow::Result<()> {
    // TODO: do stuff
    Ok(())
}
