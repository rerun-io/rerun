use std::io::{IsTerminal as _, Write as _};

use anyhow::Context as _;
use itertools::Either;
use re_byte_size::SizeBytes as _;
use re_chunk_store::{ChunkStoreConfig, CompactionOptions, IsStartOfGop, OptimizationProfile};
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

/// Parse a human-readable size string (e.g. `2MiB`, `512KiB`, `1GB`) into a byte count.
///
/// Accepts both binary (`KiB`/`MiB`/`GiB`/`TiB`) and decimal (`kB`/`MB`/`GB`/`TB`) units,
/// as well as a plain `B` suffix (e.g. `1024B`).
fn parse_size(s: &str) -> Result<u64, String> {
    let bytes = re_format::parse_bytes(s).ok_or_else(|| {
        format!(
            "invalid size {s:?}; expected a value with a unit suffix, e.g. `2MiB`, `1GB`, `1024B`"
        )
    })?;
    u64::try_from(bytes).map_err(|err| format!("size {s:?} must be non-negative: {err}"))
}

// ---

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ProfileArg {
    /// Small chunks tuned for the live Viewer workflow.
    Live,

    /// Larger chunks tuned for object-store-backed query and streaming.
    ObjectStore,
}

impl ProfileArg {
    fn to_profile(self) -> OptimizationProfile {
        match self {
            Self::Live => OptimizationProfile::LIVE,
            Self::ObjectStore => OptimizationProfile::OBJECT_STORE,
        }
    }
}

#[derive(Debug, Clone, clap::Parser)]
pub struct OptimizeCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write the optimized recording to.
    ///
    /// In single-file mode (the default), this is the output file path. If unspecified,
    /// the recording is written to standard output.
    ///
    /// In directory mirror mode (when any input is a directory), this must be set and
    /// is treated as the output directory root: the input folder structure is mirrored
    /// underneath it, with each `.rrd`/`.rbl` file optimized independently.
    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: Option<String>,

    /// Optimization profile to start from.
    ///
    /// Per-knob flags and `RERUN_CHUNK_MAX_*` env vars override the profile's
    /// values. `RERUN_STORE_ENABLE_CHANGELOG` is ignored by this command —
    /// `rerun rrd optimize` is always headless.
    #[arg(long = "profile", value_enum, default_value_t = ProfileArg::ObjectStore)]
    profile: ProfileArg,

    /// Threshold after which a Chunk cannot be compacted any further.
    ///
    /// Accepts a size string with a unit suffix, e.g. `2MiB`, `512KiB`, `1GB`, `1024B`.
    /// Both binary (`KiB`/`MiB`/`GiB`/`TiB`) and decimal (`kB`/`MB`/`GB`/`TB`) units are accepted.
    ///
    /// Overrides the profile's value and `RERUN_CHUNK_MAX_BYTES` if set.
    #[arg(long = "max-size", value_parser = parse_size)]
    max_size: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// Overrides the profile's value and `RERUN_CHUNK_MAX_ROWS` if set.
    #[arg(long = "max-rows")]
    max_rows: Option<u64>,

    /// What is the threshold, in rows, after which a Chunk cannot be compacted any further?
    ///
    /// This specifically applies to _non_ time-sorted chunks.
    ///
    /// Overrides the profile's value and `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` if set.
    #[arg(long = "max-rows-if-unsorted")]
    max_rows_if_unsorted: Option<u64>,

    /// Configures the number of extra compaction passes to run on the data.
    /// Overrides the profile's value. Default per profile: 50.
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
    #[arg(long = "num-pass")]
    num_extra_passes: Option<u32>,

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
    /// chunks much larger than `--max-size`.
    #[clap(long = "no-rebatch-videos", default_value_t = false)]
    no_rebatch_videos: bool,

    /// If set, split chunks so no two archetype groups sharing a chunk differ in
    /// byte size by more than this factor. Values should be `>= 1`; at `1.0`,
    /// every archetype is forced into its own chunk.
    ///
    /// This keeps "thick" columns (images, videos, blobs) out of the same chunk as
    /// "thin" columns (scalars, transforms, text), so the viewer can fetch just the
    /// thin data without dragging along the thick payload. Components belonging to
    /// the same archetype are always kept together.
    ///
    /// A good starting value is 10.0. If unset, the profile's value is used.
    #[arg(long = "split-size-ratio")]
    split_size_ratio: Option<f64>,
}

impl OptimizeCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            profile,
            max_size,
            max_rows,
            max_rows_if_unsorted,
            num_extra_passes,
            continue_on_error,
            no_rebatch_videos,
            split_size_ratio,
        } = self;

        if path_to_output_rrd.is_none() {
            anyhow::ensure!(
                !std::io::stdout().is_terminal(),
                "you must redirect the output to a file and/or stream"
            );
        }

        let profile = profile.to_profile();

        // Seed from profile, then env, then CLI flags. Force enable_changelog=false
        // last (optimize is headless; we never want subscribers).
        let mut store_config = profile.to_chunk_store_config();
        store_config = store_config.apply_env()?;

        if let Some(max_size) = max_size {
            store_config.chunk_max_bytes = *max_size;
        }
        if let Some(max_rows) = max_rows {
            store_config.chunk_max_rows = *max_rows;
        }
        if let Some(max_rows_if_unsorted) = max_rows_if_unsorted {
            store_config.chunk_max_rows_if_unsorted = *max_rows_if_unsorted;
        }

        store_config.enable_changelog = false;

        let num_extra_passes = num_extra_passes.unwrap_or(profile.num_extra_passes);

        let gop_batching = !*no_rebatch_videos && profile.gop_batching;

        let split_size_ratio = split_size_ratio.or(profile.split_size_ratio);

        let is_start_of_gop: IsStartOfGop = std::sync::Arc::new(|data, codec| {
            re_video::is_start_of_gop(data, codec.into()).map_err(|err| anyhow::anyhow!(err))
        });

        let compaction_options = CompactionOptions {
            config: store_config.clone(),
            num_extra_passes: Some(num_extra_passes as usize),
            is_start_of_gop: gop_batching.then_some(is_start_of_gop),
            split_size_ratio,
        };

        // Directory mirror mode: if any input is a directory, recursively expand it
        // to its `*.rrd`/`*.rbl` files and optimize each one independently, mirroring
        // the input folder structure under the output path.
        let any_input_is_dir = path_to_input_rrds
            .iter()
            .any(|p| std::path::Path::new(p).is_dir());

        if any_input_is_dir {
            let output_root = path_to_output_rrd.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "directory inputs require an output path (`-o <dir>`); cannot mirror to stdout"
                )
            })?;
            return optimize_dir_mirror(
                *continue_on_error,
                &store_config,
                &compaction_options,
                path_to_input_rrds,
                output_root,
            );
        }

        merge_and_compact(
            *continue_on_error,
            &store_config,
            Some(&compaction_options),
            path_to_input_rrds,
            path_to_output_rrd.as_ref(),
        )
    }
}

/// Walk every input (file or directory), pair each `*.rrd`/`*.rbl` with an output
/// path under `output_root` that mirrors the input folder structure, and optimize
/// each pair independently.
fn optimize_dir_mirror(
    continue_on_error: bool,
    store_config: &ChunkStoreConfig,
    compaction_options: &CompactionOptions,
    inputs: &[String],
    output_root: &str,
) -> anyhow::Result<()> {
    let output_root = std::path::PathBuf::from(output_root);
    if output_root.exists() && !output_root.is_dir() {
        anyhow::bail!(
            "output path {output_root:?} must be a directory when any input is a directory"
        );
    }

    let mut pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();

    for input in inputs {
        let input_path = std::path::Path::new(input);
        if input_path.is_dir() {
            for entry in walkdir::WalkDir::new(input_path).follow_links(false) {
                let entry = entry.with_context(|| format!("walking {input_path:?}"))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                if !is_rrd_like(entry.path()) {
                    continue;
                }
                let relative = entry
                    .path()
                    .strip_prefix(input_path)
                    .with_context(|| format!("strip_prefix({input_path:?}, {:?})", entry.path()))?;
                pairs.push((entry.path().to_path_buf(), output_root.join(relative)));
            }
        } else if input_path.is_file() {
            let file_name = input_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("input path has no file name: {input_path:?}"))?;
            pairs.push((input_path.to_path_buf(), output_root.join(file_name)));
        } else {
            anyhow::bail!("input path does not exist or is not a file/directory: {input_path:?}");
        }
    }

    if pairs.is_empty() {
        anyhow::bail!(
            "no `.rrd`/`.rbl` files found under any of: {inputs:?}\n\
             (directory mirror mode skips other extensions)"
        );
    }

    re_log::info!(
        num_files = pairs.len(),
        output_root = %output_root.display(),
        "optimizing files in directory mirror mode",
    );

    let total = pairs.len();
    let done = std::sync::atomic::AtomicUsize::new(0);

    use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};
    pairs
        .par_iter()
        .try_for_each(|(src, dst)| -> anyhow::Result<()> {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating output dir {parent:?}"))?;
            }
            let idx = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            re_log::info!(
                "[{idx}/{total}] optimizing {} -> {}",
                src.display(),
                dst.display(),
            );
            let src_str = src.to_string_lossy().into_owned();
            let dst_str = dst.to_string_lossy().into_owned();
            merge_and_compact(
                continue_on_error,
                store_config,
                Some(compaction_options),
                std::slice::from_ref(&src_str),
                Some(&dst_str),
            )
        })?;

    Ok(())
}

fn is_rrd_like(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("rrd" | "rbl"),
    )
}

// ---

/// Stub for the old `rerun rrd compact` name. Accepts any arguments and errors out with a
/// message pointing at the new name, so users who've scripted the old name get a clear hint.
#[derive(Debug, Clone, clap::Parser)]
pub struct CompactCommand {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    _ignored: Vec<String>,
}

impl CompactCommand {
    #[expect(clippy::unused_self)]
    pub fn run(&self) -> anyhow::Result<()> {
        anyhow::bail!(
            "`rerun rrd compact` has been renamed to `rerun rrd optimize`. \
             Please run `rerun rrd optimize --help` for usage."
        )
    }
}

// ---

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
        config = %store_config,
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

    log_chunk_size_stats(&entity_dbs, store_config, "post-compaction");

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

fn log_chunk_size_stats(
    entity_dbs: &std::collections::HashMap<StoreId, EntityDb>,
    store_config: &ChunkStoreConfig,
    label: &str,
) {
    let max_rows_limit = store_config.chunk_max_rows as usize;
    let max_rows_if_unsorted_limit = store_config.chunk_max_rows_if_unsorted as usize;

    let mut min_bytes = u64::MAX;
    let mut max_bytes = 0u64;
    let mut total_bytes = 0u64;
    let mut min_rows = usize::MAX;
    let mut max_rows_seen = 0usize;
    let mut total_rows = 0u64;
    let mut num_chunks = 0u64;
    let mut num_unsorted = 0u64;

    // Capped-chunk stats: chunks that hit a row-count limit during compaction.
    // The "rest" are chunks that converged below the limits, and are the most
    // interesting input for tuning chunk-size targets.
    let mut num_unsorted_at_limit = 0u64;
    let mut num_sorted_at_max_rows = 0u64;
    let mut rest_num_chunks = 0u64;
    let mut rest_total_bytes = 0u64;
    let mut rest_total_rows = 0u64;

    for db in entity_dbs.values() {
        for chunk in db.storage_engine().store().iter_physical_chunks() {
            let size = chunk.heap_size_bytes();
            let rows = chunk.num_rows();
            let is_sorted = chunk.is_time_sorted();

            min_bytes = min_bytes.min(size);
            max_bytes = max_bytes.max(size);
            total_bytes += size;
            min_rows = min_rows.min(rows);
            max_rows_seen = max_rows_seen.max(rows);
            total_rows += rows as u64;
            if !is_sorted {
                num_unsorted += 1;
            }
            num_chunks += 1;

            if !is_sorted && rows == max_rows_if_unsorted_limit {
                num_unsorted_at_limit += 1;
            } else if is_sorted && rows == max_rows_limit {
                num_sorted_at_max_rows += 1;
            } else {
                rest_num_chunks += 1;
                rest_total_bytes += size;
                rest_total_rows += rows as u64;
            }
        }
    }

    if num_chunks == 0 {
        return;
    }

    let avg_bytes = total_bytes / num_chunks;
    let avg_rows = total_rows / num_chunks;
    let unsorted_pct = num_unsorted as f64 / num_chunks as f64 * 100.0;

    let rest_avg_bytes_str = if rest_num_chunks == 0 {
        "N/A".to_owned()
    } else {
        re_format::format_bytes((rest_total_bytes / rest_num_chunks) as _)
    };
    let rest_avg_rows_str = if rest_num_chunks == 0 {
        "N/A".to_owned()
    } else {
        re_format::format_uint(rest_total_rows / rest_num_chunks)
    };

    re_log::info!(
        num_chunks,
        min = %re_format::format_bytes(min_bytes as _),
        max = %re_format::format_bytes(max_bytes as _),
        avg = %re_format::format_bytes(avg_bytes as _),
        total = %re_format::format_bytes(total_bytes as _),
        rows_min = min_rows,
        rows_max = max_rows_seen,
        rows_avg = avg_rows,
        unsorted_chunks = format!("{num_unsorted}/{num_chunks} ({unsorted_pct:.1}%)"),
        unsorted_at_limit = format!("{num_unsorted_at_limit} (rows == max_rows_if_unsorted = {max_rows_if_unsorted_limit})"),
        sorted_at_max_rows = format!("{num_sorted_at_max_rows} (rows == max_rows = {max_rows_limit})"),
        rest_num_chunks,
        rest_avg_bytes = %rest_avg_bytes_str,
        rest_avg_rows = %rest_avg_rows_str,
        "{label} chunk size stats",
    );
}
