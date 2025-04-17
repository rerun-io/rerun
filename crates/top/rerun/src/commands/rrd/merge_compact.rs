use std::io::{IsTerminal as _, Write as _};

use anyhow::Context as _;
use itertools::Either;

use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk::StoreKind;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct MergeCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: Option<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,
}

impl MergeCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            continue_on_error,
        } = self;

        if path_to_output_rrd.is_none() {
            anyhow::ensure!(
                !std::io::stdout().is_terminal(),
                "you must redirect the output to a file and/or stream"
            );
        }

        // NOTE #1: We're doing headless processing, there's no point in running subscribers, it will just
        // (massively) slow us down.
        // NOTE #2: We do not want to modify the configuration of the original data in any way
        // (e.g. by recompacting it differently), so make sure to disable all these features.
        let store_config = ChunkStoreConfig::ALL_DISABLED;

        merge_and_compact(
            *continue_on_error,
            &store_config,
            path_to_input_rrds,
            path_to_output_rrd.as_ref(),
        )
    }
}

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct CompactCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: Option<String>,

    /// What is the threshold, in bytes, after which a Chunk cannot be compacted any further?
    ///
    /// Overrides `RERUN_CHUNK_MAX_BYTES` if set.
    #[arg(long = "max-bytes")]
    max_bytes: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// Overrides `RERUN_CHUNK_MAX_ROWS` if set.
    #[arg(long = "max-rows")]
    max_rows: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// This specifically applies to _non_ time-sorted chunks.
    ///
    /// Overrides `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` if set.
    #[arg(long = "max-rows-if-unsorted")]
    max_rows_if_unsorted: Option<u64>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,
}

impl CompactCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            max_bytes,
            max_rows,
            max_rows_if_unsorted,
            continue_on_error,
        } = self;

        if path_to_output_rrd.is_none() {
            anyhow::ensure!(
                !std::io::stdout().is_terminal(),
                "you must redirect the output to a file and/or stream"
            );
        }

        let mut store_config = ChunkStoreConfig::from_env().unwrap_or_default();
        // NOTE: We're doing headless processing, there's no point in running subscribers, it will just
        // (massively) slow us down.
        store_config.enable_changelog = false;

        if let Some(max_bytes) = max_bytes {
            store_config.chunk_max_bytes = *max_bytes;
        }
        if let Some(max_rows) = max_rows {
            store_config.chunk_max_rows = *max_rows;
        }
        if let Some(max_rows_if_unsorted) = max_rows_if_unsorted {
            store_config.chunk_max_rows_if_unsorted = *max_rows_if_unsorted;
        }

        merge_and_compact(
            *continue_on_error,
            &store_config,
            path_to_input_rrds,
            path_to_output_rrd.as_ref(),
        )
    }
}

fn merge_and_compact(
    continue_on_error: bool,
    store_config: &ChunkStoreConfig,
    path_to_input_rrds: &[String],
    path_to_output_rrd: Option<&String>,
) -> anyhow::Result<()> {
    let file_size_to_string = |size: Option<u64>| {
        size.map_or_else(
            || "<unknown>".to_owned(),
            |size| re_format::format_bytes(size as _),
        )
    };

    let now = std::time::Instant::now();
    re_log::info!(
        max_rows = %re_format::format_uint(store_config.chunk_max_rows),
        max_rows_if_unsorted = %re_format::format_uint(store_config.chunk_max_rows_if_unsorted),
        max_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
        srcs = ?path_to_input_rrds,
        "merge/compaction started"
    );

    // TODO(cmc): might want to make this configurable at some point.

    let (rx, rx_size_bytes) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();

    for (_source, res) in rx {
        let mut is_success = true;

        match res {
            Ok(msg) => {
                if let Err(err) = entity_dbs
                    .entry(msg.store_id().clone())
                    .or_insert_with(|| {
                        re_entity_db::EntityDb::with_store_config(
                            msg.store_id().clone(),
                            store_config.clone(),
                        )
                    })
                    .add(&msg)
                {
                    re_log::error!(%err, "couldn't index corrupt chunk");
                    is_success = false;
                }
            }

            Err(err) => {
                re_log::error!(err = re_error::format(err));
                is_success = false;
            }
        }

        if !continue_on_error && !is_success {
            anyhow::bail!(
                "one or more IO and/or decoding failures in the input stream (check logs)"
            )
        }
    }

    let mut rrd_out = if let Some(path) = path_to_output_rrd {
        Either::Left(std::io::BufWriter::new(
            std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
        ))
    } else {
        Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
    };

    let messages_rbl = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let messages_rrd = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Recording)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    // TODO(cmc): encoding options should match the original.
    let encoding_options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = entity_dbs
        .values()
        .next()
        .and_then(|db| db.store_info())
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);
    let rrd_out_size = re_log_encoding::encoder::encode(
        version,
        encoding_options,
        // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
        // set up the viewport correctly.
        messages_rbl.chain(messages_rrd),
        &mut rrd_out,
    )
    .context("couldn't encode messages")?;

    rrd_out.flush().context("couldn't flush output")?;

    let rrds_in_size = rx_size_bytes.recv().ok();
    let size_reduction = if let (Some(rrds_in_size), rrd_out_size) = (rrds_in_size, rrd_out_size) {
        format!(
            "-{:3.3}%",
            100.0 - rrd_out_size as f64 / (rrds_in_size as f64 + f64::EPSILON) * 100.0
        )
    } else {
        "N/A".to_owned()
    };

    re_log::info!(
        dst_size_bytes = %file_size_to_string(Some(rrd_out_size)),
        time = ?now.elapsed(),
        size_reduction,
        srcs = ?path_to_input_rrds,
        srcs_size_bytes = %file_size_to_string(rrds_in_size),
        "merge/compaction finished"
    );

    Ok(())
}
