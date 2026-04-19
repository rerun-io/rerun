use std::io::{IsTerminal as _, Write as _};

use anyhow::Context as _;
use itertools::Either;
use re_byte_size::SizeBytes as _;
use re_chunk_store::{ChunkStoreConfig, CompactionOptions, IsStartOfGop};
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
            None, // no compaction for merge
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

    /// Configures the number of extra compaction passes to run on the data.
    ///
    /// Compaction in Rerun is an iterative, convergent process: every single pass will improve the
    /// quality of the compaction (with diminishing returns), until it eventually converges into a
    /// stable state.
    /// The more passes, the better the compaction quality.
    ///
    /// Under the hood, you can think of it as a kind of clustering algorithm: every incoming chunk
    /// finds the most appropriate chunk to merge into, thereby creating a new cluster, which is
    /// itself just a bigger chunk. On the next pass, these new clustered chunks will themselves
    /// look for other clusters to merge into, yielding even bigger clusters, which again are also
    /// just chunks. And so on and so forth.
    ///
    /// If/When the data reaches a stable optimum, the computation will stop immediately, regardless of
    /// how many passes are left.
    #[arg(long = "num-pass", default_value_t = 50)]
    num_extra_passes: u32,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,

    /// Disable rebatching of video stream chunks to GoP (Group of Pictures) boundaries.
    ///
    /// By default, after compaction, video stream chunks are rebatched on GoP
    /// boundaries so that each chunk contains one or more complete GoPs.
    /// This flag disables that behavior.
    ///
    /// Note: GoP rebatching never splits a GoP across chunks, so streams with
    /// long keyframe intervals (e.g. 10+ seconds between I-frames) can produce
    /// chunks much larger than `--max-bytes`.
    #[clap(long = "no-rebatch-videos", default_value_t = false)]
    no_rebatch_videos: bool,
}

impl CompactCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            max_bytes,
            max_rows,
            max_rows_if_unsorted,
            num_extra_passes,
            continue_on_error,
            no_rebatch_videos,
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

        let is_start_of_gop: IsStartOfGop = std::sync::Arc::new(|data, codec| {
            re_video::is_start_of_gop(data, codec.into()).map_err(|err| anyhow::anyhow!(err))
        });

        let compaction_options = CompactionOptions {
            config: store_config.clone(),
            num_extra_passes: Some(*num_extra_passes as usize),
            is_start_of_gop: if *no_rebatch_videos {
                None
            } else {
                Some(is_start_of_gop)
            },
        };

        merge_and_compact(
            *continue_on_error,
            &store_config,
            Some(&compaction_options),
            path_to_input_rrds,
            path_to_output_rrd.as_ref(),
        )
    }
}

fn merge_and_compact(
    continue_on_error: bool,
    store_config: &ChunkStoreConfig,
    compaction_options: Option<&CompactionOptions>,
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

    let (rx, rx_size_bytes) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();

    re_log::info!("processing input…");
    let mut num_chunks_before = 0u64;
    let mut last_checkpoint = std::time::Instant::now();
    for (msg_nr, (_source, res)) in rx.iter().enumerate() {
        let mut is_success = true;

        match res {
            Ok(msg) => {
                num_chunks_before += matches!(msg, re_log_types::LogMsg::ArrowMsg(_, _)) as u64;
                let db = entity_dbs.entry(msg.store_id().clone()).or_insert_with(|| {
                    let enable_viewer_indexes = false; // that would just slow us down for no reason
                    re_entity_db::EntityDb::with_store_config(
                        msg.store_id().clone(),
                        enable_viewer_indexes,
                        store_config.clone(),
                    )
                });
                if let Err(err) = db.add_log_msg(&msg) {
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

        let msg_count = msg_nr + 1;
        let check_in_interval = 10_000;
        if msg_count % check_in_interval == 0 {
            let msg_per_second = check_in_interval as f64 / last_checkpoint.elapsed().as_secs_f64();
            last_checkpoint = std::time::Instant::now();
            re_log::info!(
                "processed {msg_count} messages so far, current speed is {msg_per_second:.2} msg/s"
            );
            re_tracing::reexports::puffin::GlobalProfiler::lock().new_frame();
        }
    }

    if let Some(compaction_options) = compaction_options {
        let now = std::time::Instant::now();

        let num_chunks_before = entity_dbs
            .values()
            .map(|db| db.storage_engine().store().num_physical_chunks() as u64)
            .sum::<u64>();

        for db in entity_dbs.values() {
            // Safety: we are the only owners of that data, it's fine.
            #[expect(unsafe_code)]
            let engine = unsafe { db.storage_engine_raw() };

            let compacted = engine.read().store().compacted(compaction_options)?;
            *engine.write().store() = compacted;
        }

        let num_chunks_after = entity_dbs
            .values()
            .map(|db| db.storage_engine().store().num_physical_chunks() as u64)
            .sum::<u64>();

        let num_chunks_reduction = format!(
            "-{:3.3}%",
            100.0 - num_chunks_after as f64 / (num_chunks_before as f64 + f64::EPSILON) * 100.0
        );

        re_log::info!(
            num_chunks_before, num_chunks_after, num_chunks_reduction, time=?now.elapsed(),
            "compaction completed",
        );
    }

    log_chunk_size_stats(&entity_dbs, "post-compaction");

    let mut rrd_out = if let Some(path) = path_to_output_rrd {
        Either::Left(std::io::BufWriter::new(
            std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
        ))
    } else {
        Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
    };

    re_log::info!("preparing output…");
    let messages_rbl = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let mut num_chunks_after = 0u64;
    let messages_rrd = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Recording)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */))
        .inspect(|msg| {
            num_chunks_after += matches!(msg, Ok(re_log_types::LogMsg::ArrowMsg(_, _))) as u64;
        });

    // TODO(cmc): encoding options should match the original.
    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = entity_dbs
        .values()
        .next()
        .and_then(|db| db.store_info())
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    re_log::info!("encoding…");
    let rrd_out_size = re_log_encoding::Encoder::encode_into(
        version,
        encoding_options,
        // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
        // set up the viewport correctly.
        messages_rbl.chain(messages_rrd),
        &mut rrd_out,
    )
    .context("couldn't encode messages")?;

    rrd_out.flush().context("couldn't flush output")?;

    let rrds_in_size = rx_size_bytes.recv().ok().map(|(size, _footers)| size);
    let num_chunks_reduction = format!(
        "-{:3.3}%",
        100.0 - num_chunks_after as f64 / (num_chunks_before as f64 + f64::EPSILON) * 100.0
    );
    let size_reduction = if let (Some(rrds_in_size), rrd_out_size) = (rrds_in_size, rrd_out_size) {
        format!(
            "-{:3.3}%",
            100.0 - rrd_out_size as f64 / (rrds_in_size as f64 + f64::EPSILON) * 100.0
        )
    } else {
        "N/A".to_owned()
    };

    re_log::info!(
        srcs = ?path_to_input_rrds,
        time = ?now.elapsed(),
        "merge/compaction finished. Chunk count {} -> {} ({num_chunks_reduction}), size {} -> {} ({size_reduction})",
        re_format::format_uint(num_chunks_before),
        re_format::format_uint(num_chunks_after),
        file_size_to_string(rrds_in_size),
        file_size_to_string(Some(rrd_out_size)),
    );

    Ok(())
}

fn log_chunk_size_stats(entity_dbs: &std::collections::HashMap<StoreId, EntityDb>, label: &str) {
    let mut min_bytes = u64::MAX;
    let mut max_bytes = 0u64;
    let mut total_bytes = 0u64;
    let mut num_chunks = 0u64;

    for db in entity_dbs.values() {
        for chunk in db.storage_engine().store().iter_physical_chunks() {
            let size = chunk.heap_size_bytes();
            min_bytes = min_bytes.min(size);
            max_bytes = max_bytes.max(size);
            total_bytes += size;
            num_chunks += 1;
        }
    }

    if num_chunks == 0 {
        return;
    }

    let avg_bytes = total_bytes / num_chunks;

    re_log::info!(
        num_chunks,
        min = %re_format::format_bytes(min_bytes as _),
        max = %re_format::format_bytes(max_bytes as _),
        avg = %re_format::format_bytes(avg_bytes as _),
        total = %re_format::format_bytes(total_bytes as _),
        "{label} chunk size stats",
    );
}
