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
            eprintln!(
                "Ignored {} file(s) that are called .backup.rrd, and are therefore assumed to already have been migrated",
                num_files_before - num_files
            );
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
            .filter_map(|original_path| {
                if let Err(err) = migrate_file_at(original_path) {
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
                "✅ {} file(s) successfully migrated.",
                path_to_input_rrds.len()
            );
            Ok(())
        } else {
            let num_failures = failures.len();
            eprintln!("❌ Failed to migrate {num_failures}/{num_files} file(s):");
            eprintln!();
            for (path, err) in &failures {
                eprintln!("  {path}: {}\n", re_error::format(err));
            }
            anyhow::bail!("Failed to migrate {num_failures}/{num_files} file(s)");
        }
    }
}

fn migrate_file_at(original_path: &Utf8PathBuf) -> anyhow::Result<()> {
    // Rename `old_name.rrd` to `old_name.backup.rrd`:
    let backup_path = original_path.with_extension("backup.rrd");

    if backup_path.exists() {
        eprintln!("Ignoring migration of {original_path}: {backup_path} already exists");
        return Ok(());
    }

    std::fs::rename(original_path, &backup_path)
        .with_context(|| format!("Couldn't rename {original_path:?} to {backup_path:?}"))?;

    if let Err(err) = migrate_from_to(&backup_path, original_path) {
        // Restore:
        std::fs::rename(&backup_path, original_path).ok();
        Err(err)
    } else {
        Ok(())
    }
}

/// Stream-convert an rrd file
fn migrate_from_to(from_path: &Utf8PathBuf, to_path: &Utf8PathBuf) -> anyhow::Result<()> {
    let from_file =
        std::fs::File::open(from_path).with_context(|| format!("Failed to open {from_path:?}"))?;

    let decoder = re_log_encoding::decoder::Decoder::new(std::io::BufReader::new(from_file))?;

    let mut errors = indexmap::IndexSet::new();

    let messages = decoder.into_iter().filter_map(|result| match result {
        Ok(msg) => match msg {
            re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg) => {
                match re_sorbet::SorbetBatch::try_from_record_batch(
                    &arrow_msg.batch,
                    re_sorbet::BatchType::Chunk,
                ) {
                    Ok(batch) => {
                        let batch = arrow::array::RecordBatch::from(&batch);
                        Some(Ok(re_log_types::LogMsg::ArrowMsg(
                            store_id,
                            re_log_types::ArrowMsg {
                                chunk_id: arrow_msg.chunk_id,
                                timepoint_max: arrow_msg.timepoint_max.clone(),
                                batch,
                                on_release: None,
                            },
                        )))
                    }
                    Err(err) => {
                        errors.insert(err.to_string());
                        None
                    }
                }
            }
            re_log_types::LogMsg::BlueprintActivationCommand(..)
            | re_log_types::LogMsg::SetStoreInfo(..) => Some(Ok(msg)),
        },
        Err(err) => {
            errors.insert(err.to_string());
            None
        }
    });

    let new_file =
        std::fs::File::create(to_path).with_context(|| format!("Failed to create {to_path:?}"))?;

    let mut buffered_writer = std::io::BufWriter::new(new_file);

    re_log_encoding::encoder::encode(
        CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        messages,
        &mut buffered_writer,
    )
    .with_context(|| format!("Failed to write new .rrd file to {to_path:?}"))?;

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.iter().join("\n"))
    }
}
