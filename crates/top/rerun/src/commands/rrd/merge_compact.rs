use std::path::PathBuf;

use anyhow::Context as _;

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

    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: String,

    /// If set, will try to continue in the face of IO and decoding errors.
    #[clap(long, default_value_t = false)]
    best_effort: bool,
}

impl MergeCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            best_effort,
        } = self;

        // NOTE #1: We're doing headless processing, there's no point in running subscribers, it will just
        // (massively) slow us down.
        // NOTE #2: We do not want to modify the configuration of the original data in any way
        // (e.g. by recompacting it differently), so make sure to disable all these features.
        let store_config = ChunkStoreConfig::ALL_DISABLED;

        merge_and_compact(
            *best_effort,
            &store_config,
            path_to_input_rrds,
            path_to_output_rrd,
        )
    }
}

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct CompactCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: String,

    /// What is the threshold, in bytes, after which a Chunk cannot be compacted any further?
    ///
    /// Overrides RERUN_CHUNK_MAX_BYTES if set.
    #[arg(long = "max-bytes")]
    max_bytes: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// Overrides RERUN_CHUNK_MAX_ROWS if set.
    #[arg(long = "max-rows")]
    max_rows: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// This specifically applies to _non_ time-sorted chunks.
    ///
    /// Overrides RERUN_CHUNK_MAX_ROWS_IF_UNSORTED if set.
    #[arg(long = "max-rows-if-unsorted")]
    max_rows_if_unsorted: Option<u64>,

    /// If set, will try to continue in the face of IO and decoding errors.
    #[clap(long, default_value_t = false)]
    best_effort: bool,
}

impl CompactCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            max_bytes,
            max_rows,
            max_rows_if_unsorted,
            best_effort,
        } = self;

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
            *best_effort,
            &store_config,
            path_to_input_rrds,
            path_to_output_rrd,
        )
    }
}

fn merge_and_compact(
    best_effort: bool,
    store_config: &ChunkStoreConfig,
    path_to_input_rrds: &[String],
    path_to_output_rrd: &str,
) -> anyhow::Result<()> {
    let path_to_output_rrd = PathBuf::from(path_to_output_rrd);

    let rrds_in_size = {
        let rrds_in: Result<Vec<_>, _> = path_to_input_rrds
            .iter()
            .map(|path_to_input_rrd| {
                std::fs::File::open(path_to_input_rrd)
                    .with_context(|| format!("{path_to_input_rrd:?}"))
            })
            .collect();
        rrds_in.ok().and_then(|rrds_in| {
            rrds_in
                .iter()
                .map(|rrd_in| rrd_in.metadata().ok().map(|md| md.len()))
                .sum::<Option<u64>>()
        })
    };

    let file_size_to_string = |size: Option<u64>| {
        size.map_or_else(
            || "<unknown>".to_owned(),
            |size| re_format::format_bytes(size as _),
        )
    };

    re_log::info!(
        max_num_rows = %re_format::format_uint(store_config.chunk_max_rows),
        max_num_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
        dst = ?path_to_output_rrd,
        srcs = ?path_to_input_rrds,
        src_size_bytes = %file_size_to_string(rrds_in_size),
        "merge started"
    );

    let now = std::time::Instant::now();

    let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
    let rx = read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();
    let mut is_success = true;

    for res in rx {
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
                    re_log::error!(%err, "couldn't index message");
                    is_success = false;
                }
            }
            Err(err) => {
                re_log::error!(err = re_error::format(err));
                is_success = false;
            }
        }

        if !best_effort && !is_success {
            break;
        }
    }

    anyhow::ensure!(
        !entity_dbs.is_empty(),
        "no recordings found in rrd/rbl file"
    );

    let mut rrd_out = std::fs::File::create(&path_to_output_rrd)
        .with_context(|| format!("{path_to_output_rrd:?}"))?;

    let messages_rbl = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let messages_rrd = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Recording)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
    let version = entity_dbs
        .values()
        .next()
        .and_then(|db| db.store_info())
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);
    re_log_encoding::encoder::encode(
        version,
        encoding_options,
        // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
        // set up the viewport correctly.
        messages_rbl.chain(messages_rrd),
        &mut rrd_out,
    )
    .context("couldn't encode messages")?;

    let rrd_out_size = rrd_out.metadata().ok().map(|md| md.len());

    let compaction_ratio =
        if let (Some(rrds_in_size), Some(rrd_out_size)) = (rrds_in_size, rrd_out_size) {
            format!(
                "{:3.3}%",
                100.0 - rrd_out_size as f64 / (rrds_in_size as f64 + f64::EPSILON) * 100.0
            )
        } else {
            "N/A".to_owned()
        };

    re_log::info!(
        dst = ?path_to_output_rrd,
        dst_size_bytes = %file_size_to_string(rrd_out_size),
        time = ?now.elapsed(),
        compaction_ratio,
        srcs = ?path_to_input_rrds,
        srcs_size_bytes = %file_size_to_string(rrds_in_size),
        "compaction finished"
    );

    if is_success {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "one or more IO and/or decoding failures (check logs)"
        ))
    }
}
