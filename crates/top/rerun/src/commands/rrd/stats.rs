use std::collections::BTreeMap;

use ahash::{HashMap, HashMapExt as _};
use itertools::Itertools as _;
use re_chunk::Chunk;
use re_log_encoding::ToApplication as _;
use re_log_types::{EntityPath, TimelineName};
use re_protos::log_msg::v1alpha1::log_msg::Msg;
use re_quota_channel::send_crossbeam;

use crate::commands::read_raw_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct StatsCommand {
    /// If set, the data will never be decoded.
    ///
    /// Statistics will be computed at the transport-level instead, which is more limited in
    /// terms of what can be computed, but also orders of magnitude faster.
    #[clap(long = "no-decode", default_value_t = false)]
    no_decode: bool,

    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = true)]
    continue_on_error: bool,
}

impl StatsCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            no_decode,
            path_to_input_rrds,
            continue_on_error,
        } = self;

        let mut num_chunks = 0u64;
        let mut num_chunks_per_entity: HashMap<String, u64> = HashMap::new();
        let mut num_chunks_per_index: HashMap<String, u64> = HashMap::new();
        let mut num_chunks_per_component: HashMap<String, u64> = HashMap::new();
        // Per entity, per timeline: `true` iff every chunk seen so far has this timeline sorted.
        let mut timeline_is_sorted: BTreeMap<EntityPath, BTreeMap<TimelineName, bool>> =
            BTreeMap::new();
        let mut num_rows = Vec::with_capacity(num_chunks as _);
        let mut num_static = 0u64;
        let mut num_indexes = Vec::with_capacity(num_chunks as _);
        let mut num_components = Vec::with_capacity(num_chunks as _);
        let mut ipc_size_bytes_compressed = Vec::with_capacity(num_chunks as _);
        let mut ipc_size_bytes_uncompressed = Vec::with_capacity(num_chunks as _);
        let mut ipc_schema_size_bytes_uncompressed = Vec::with_capacity(num_chunks as _);
        let mut ipc_data_size_bytes_uncompressed = Vec::with_capacity(num_chunks as _);

        let (rx_raw, rx_footers) = read_raw_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        // Each message is accompanied by the original compressed payload size (in bytes).
        // For uncompressed messages, this equals the payload size.
        let (tx_uncompressed, rx_uncompressed) =
            crossbeam::channel::bounded::<(u64, anyhow::Result<Msg>)>(100);
        let decompress_thread_handle = std::thread::Builder::new()
            .name("decompress".to_owned())
            .spawn(move || {
                for (_source, res) in rx_raw {
                    let Ok(Msg::ArrowMsg(mut msg)) = res else {
                        send_crossbeam(&tx_uncompressed, (0, res))?;
                        continue;
                    };

                    let mut uncompressed = Vec::new();

                    const COMPRESSION_NONE: i32 =
                        re_protos::common::v1alpha1::Compression::None as _;
                    const COMPRESSION_LZ4: i32 = re_protos::common::v1alpha1::Compression::Lz4 as _;

                    let compressed_size = msg.payload.len() as u64;

                    match msg.compression {
                        COMPRESSION_NONE => {}

                        COMPRESSION_LZ4 => {
                            uncompressed.resize(msg.uncompressed_size as _, 0);
                            re_log_encoding::external::lz4_flex::block::decompress_into(
                                &msg.payload,
                                &mut uncompressed,
                            )?;
                            msg.payload = uncompressed.into();
                            msg.compression = COMPRESSION_NONE;
                        }

                        huh => anyhow::bail!("unknown Compression: {huh}"),
                    }

                    send_crossbeam(
                        &tx_uncompressed,
                        (
                            compressed_size,
                            Ok(re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(msg)),
                        ),
                    )?;
                }

                Ok(())
            })?;

        re_log::info!("processing input…");
        let mut num_msgs = 0;
        let mut last_checkpoint = std::time::Instant::now();
        for (compressed_size, res) in rx_uncompressed {
            let mut is_success = true;

            match res {
                Ok(msg) => {
                    num_msgs += 1;
                    match compute_stats(!*no_decode, compressed_size, &msg) {
                        Ok(Some(stats)) => {
                            num_chunks += 1;

                            if let Some(stats) = stats.app {
                                *num_chunks_per_entity.entry(stats.entity_path).or_default() += 1;
                                for index in stats.indexes {
                                    *num_chunks_per_index.entry(index).or_default() += 1;
                                }
                                for component in stats.components {
                                    *num_chunks_per_component.entry(component).or_default() += 1;
                                }
                                num_rows.push(stats.num_rows);
                                num_static += (stats.num_indexes == 0) as u64;
                                num_indexes.push(stats.num_indexes);
                                num_components.push(stats.num_components);
                                for (entity_path, timeline_name, sorted) in
                                    stats.timeline_sortedness
                                {
                                    let entry = timeline_is_sorted
                                        .entry(entity_path)
                                        .or_default()
                                        .entry(timeline_name)
                                        .or_insert(true);
                                    *entry &= sorted;
                                }
                            }

                            ipc_size_bytes_compressed
                                .push(stats.transport.ipc_size_bytes_compressed);
                            ipc_size_bytes_uncompressed
                                .push(stats.transport.ipc_size_bytes_uncompressed);
                            ipc_schema_size_bytes_uncompressed
                                .push(stats.transport.ipc_schema_size_bytes);
                            ipc_data_size_bytes_uncompressed
                                .push(stats.transport.ipc_data_size_bytes);
                        }

                        Ok(None) => {}

                        Err(err) => {
                            re_log::error_once!("{}", re_error::format(err));
                            is_success = false;
                        }
                    }
                }

                Err(err) => {
                    re_log::error_once!("{}", re_error::format(err));
                    is_success = false;
                }
            }

            if !*continue_on_error && !is_success {
                anyhow::bail!(
                    "one or more IO and/or decoding failures in the input stream (check logs)"
                )
            }

            let msg_count = num_msgs + 1;
            let check_in_interval = 10_000;
            if msg_count % check_in_interval == 0 {
                let msgs_per_sec =
                    check_in_interval as f64 / last_checkpoint.elapsed().as_secs_f64();
                last_checkpoint = std::time::Instant::now();
                re_log::info!(
                    "processed {msg_count} messages so far, current speed is {msgs_per_sec:.2} msg/s"
                );
                re_tracing::reexports::puffin::GlobalProfiler::lock().new_frame();
            }
        }

        decompress_thread_handle
            .join()
            .expect("couldn't join thread")?;

        re_log::info!("computing stats…");

        println!("Overview");
        println!("----------");

        println!("num_chunks = {}", re_format::format_uint(num_chunks));

        if !*no_decode {
            println!(
                "num_entity_paths = {}",
                re_format::format_uint(num_chunks_per_entity.len())
            );

            let num_chunks_without_components = num_components.iter().filter(|v| **v == 0).count();
            println!(
                "num_chunks_without_components = {} ({:.3}%)",
                re_format::format_uint(num_chunks_without_components),
                num_chunks_without_components as f64 / num_chunks as f64 * 100.0,
            );

            let num_rows_total = num_rows.iter().copied().sum::<u64>();
            let num_rows_min = num_rows.iter().copied().min().unwrap_or_default();
            let num_rows_max = num_rows.iter().copied().max().unwrap_or_default();
            let num_rows_avg = num_rows_total as f64 / num_rows.len() as f64;

            println!("num_rows = {}", re_format::format_uint(num_rows_total));
            println!("num_rows_min = {}", re_format::format_uint(num_rows_min));
            println!("num_rows_max = {}", re_format::format_uint(num_rows_max));
            println!("num_rows_avg = {num_rows_avg:.3}");

            let num_indexes_min = num_indexes.iter().copied().min().unwrap_or_default();
            let num_indexes_max = num_indexes.iter().copied().max().unwrap_or_default();
            let num_indexes_avg =
                num_indexes.iter().copied().sum::<u64>() as f64 / num_indexes.len() as f64;

            println!("num_static = {}", re_format::format_uint(num_static));
            println!(
                "num_indexes_min = {}",
                re_format::format_uint(num_indexes_min)
            );
            println!(
                "num_indexes_max = {}",
                re_format::format_uint(num_indexes_max)
            );
            println!("num_indexes_avg = {num_indexes_avg:.3}");

            let num_components_min = num_components.iter().copied().min().unwrap_or_default();
            let num_components_max = num_components.iter().copied().max().unwrap_or_default();
            let num_components_avg =
                num_components.iter().copied().sum::<u64>() as f64 / num_components.len() as f64;

            println!(
                "num_components_min = {}",
                re_format::format_uint(num_components_min)
            );
            println!(
                "num_components_max = {}",
                re_format::format_uint(num_components_max)
            );
            println!("num_components_avg = {num_components_avg:.3}");

            let print_details = |num_chunks_per_xxx: HashMap<String, u64>| {
                let mut num_chunks_per_xxx = num_chunks_per_xxx.into_iter().collect_vec();
                num_chunks_per_xxx.sort_by(|(kl, _), (kr, _)| kl.cmp(kr));

                for (xxx, num_chunks) in num_chunks_per_xxx {
                    println!("{xxx}: {}", re_format::format_uint(num_chunks));
                }
            };

            println!();
            println!("Num chunks per entity");
            println!("---------------------");
            print_details(num_chunks_per_entity);

            println!();
            println!("Num chunks per index");
            println!("--------------------");
            print_details(num_chunks_per_index);

            println!();
            println!("Num chunks per component");
            println!("------------------------");
            print_details(num_chunks_per_component);
        }

        let print_ipc_size_bytes_stats = |mut ipc_size_bytes: Vec<u64>| {
            ipc_size_bytes.sort();

            let ipc_size_bytes_total = ipc_size_bytes.iter().copied().sum::<u64>() as f64;
            let ipc_size_bytes_avg = ipc_size_bytes_total / ipc_size_bytes.len() as f64;

            let ipc_size_bytes_min =
                ipc_size_bytes.iter().copied().min().unwrap_or_default() as f64;
            let ipc_size_bytes_max =
                ipc_size_bytes.iter().copied().max().unwrap_or_default() as f64;

            println!(
                "ipc_size_bytes_total = {}",
                re_format::format_bytes(ipc_size_bytes_total)
            );
            println!(
                "ipc_size_bytes_min = {}",
                re_format::format_bytes(ipc_size_bytes_min)
            );
            println!(
                "ipc_size_bytes_max = {}",
                re_format::format_bytes(ipc_size_bytes_max)
            );
            println!(
                "ipc_size_bytes_avg = {}",
                re_format::format_bytes(ipc_size_bytes_avg)
            );

            let print_percentile = |pxx_name: &str, p: f64| {
                let pxx = ipc_size_bytes
                    .get((ipc_size_bytes.len() as f64 * p) as usize)
                    .map_or(0.0, |&v| v as f64);

                println!(
                    "ipc_size_bytes_{pxx_name} = {}",
                    re_format::format_bytes(pxx)
                );
            };

            print_percentile("p50", 0.5);
            print_percentile("p90", 0.9);
            print_percentile("p95", 0.95);
            print_percentile("p99", 0.99);
            print_percentile("p999", 0.999);
        };

        println!();
        println!("Size (schema + data, compressed)");
        println!("--------------------------------");
        print_ipc_size_bytes_stats(ipc_size_bytes_compressed);

        println!();
        println!("Size (schema + data, uncompressed)");
        println!("----------------------------------");
        print_ipc_size_bytes_stats(ipc_size_bytes_uncompressed);

        println!();
        println!("Size (schema only, uncompressed)");
        println!("--------------------------------");
        print_ipc_size_bytes_stats(ipc_schema_size_bytes_uncompressed);

        println!();
        println!("Size (data only, uncompressed)");
        println!("------------------------------");
        print_ipc_size_bytes_stats(ipc_data_size_bytes_uncompressed);

        if !*no_decode {
            println!();
            println!("Unsorted timelines");
            println!("------------------");
            let entities_with_unsorted: Vec<&EntityPath> = timeline_is_sorted
                .iter()
                .filter(|(_, timelines)| timelines.values().any(|sorted| !*sorted))
                .map(|(entity, _)| entity)
                .collect();

            if entities_with_unsorted.is_empty() {
                println!("(none — every timeline on every chunk is sorted)");
            } else {
                println!(
                    "{} entity(ies) had at least one chunk with an unsorted timeline. \
                     For each such entity, all of its timelines are listed below:",
                    re_format::format_uint(entities_with_unsorted.len())
                );
                for entity in entities_with_unsorted {
                    println!("  {entity}");
                    for (timeline, sorted) in &timeline_is_sorted[entity] {
                        let status = if *sorted { "sorted" } else { "UNSORTED" };
                        println!("    {timeline}: {status}");
                    }
                }
            }
        }

        // The footer is parsed straight from the raw bytes, so these stats are available even with
        // `--no-decode`.
        println!();
        println!("Footers");
        println!("-------");
        match rx_footers.recv() {
            Ok((_size_bytes, footers)) => print_footer_stats(footers, *continue_on_error)?,
            Err(_) => println!("(none — the input stream produced no footer metadata)"),
        }

        Ok(())
    }
}

/// Prints statistics about the RRD footer(s), i.e. the `RrdManifest`s carried by the trailing
/// `::End` message(s) of the stream.
///
/// Each manifest catalogs every chunk in a single recording without requiring any of that chunk
/// data to be decoded, so all of these stats are derived purely from the footer.
fn print_footer_stats(
    footers: Vec<(
        crate::commands::InputSource,
        anyhow::Result<re_log_encoding::RawRrdManifest>,
    )>,
    continue_on_error: bool,
) -> anyhow::Result<()> {
    if footers.is_empty() {
        println!("(none — no RRD footer was found)");
        return Ok(());
    }

    let num_manifests = footers.iter().filter(|(_, res)| res.is_ok()).count();
    println!(
        "num_manifests = {} (one per recording)",
        re_format::format_uint(num_manifests)
    );

    for (source, res) in footers {
        let manifest = match res {
            Ok(manifest) => manifest,
            Err(err) => {
                re_log::error_once!(
                    "failed to parse footer from {source}: {}",
                    re_error::format(err)
                );
                if !continue_on_error {
                    anyhow::bail!(
                        "one or more corrupt RRD footers in the input stream (check logs)"
                    )
                }
                continue;
            }
        };

        let num_chunks = manifest.data.num_rows() as u64;
        let num_static_chunks = manifest.col_chunk_is_static()?.filter(|s| *s).count() as u64;
        let num_entity_paths = manifest.col_chunk_entity_path()?.unique().count();
        let byte_size_total: u64 = manifest.col_chunk_byte_size()?.sum();
        let byte_size_uncompressed_total: u64 = manifest.col_chunk_byte_size_uncompressed()?.sum();

        let sha256 = manifest
            .sorbet_schema_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        println!();
        println!("Footer manifest for {:?}", manifest.store_id);
        println!(
            "  num_chunks_indexed = {}",
            re_format::format_uint(num_chunks)
        );
        println!(
            "  num_static_chunks = {}",
            re_format::format_uint(num_static_chunks)
        );
        println!(
            "  num_entity_paths = {}",
            re_format::format_uint(num_entity_paths)
        );
        println!(
            "  manifest_num_columns = {}",
            re_format::format_uint(manifest.data.num_columns())
        );
        println!(
            "  sorbet_schema_num_fields = {}",
            re_format::format_uint(manifest.sorbet_schema.fields.len())
        );
        println!("  sorbet_schema_sha256 = {sha256}");
        println!(
            "  chunk_byte_size_total (native) = {}",
            re_format::format_bytes(byte_size_total as f64)
        );
        println!(
            "  chunk_byte_size_uncompressed_total = {}",
            re_format::format_bytes(byte_size_uncompressed_total as f64)
        );
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct ChunkStats {
    app: Option<ChunkStatsApplication>,
    transport: ChunkStatsTransport,
}

#[derive(Clone, Debug)]
struct ChunkStatsTransport {
    ipc_size_bytes_compressed: u64,
    ipc_size_bytes_uncompressed: u64,

    ipc_schema_size_bytes: u64,
    ipc_data_size_bytes: u64,
}

#[derive(Clone, Debug)]
struct ChunkStatsApplication {
    // TODO(#6572): the fact that the Entity Path is only present at the app layer is a serious problem.
    entity_path: String,

    indexes: Vec<String>,
    components: Vec<String>,

    num_rows: u64,
    num_indexes: u64,
    num_components: u64,

    /// Per-timeline sortedness for this chunk, scoped to its entity path.
    timeline_sortedness: Vec<(EntityPath, TimelineName, bool)>,
}

fn compute_stats(app: bool, compressed_size: u64, msg: &Msg) -> anyhow::Result<Option<ChunkStats>> {
    if let Msg::ArrowMsg(arrow_msg) = msg {
        let re_protos::log_msg::v1alpha1::ArrowMsg {
            store_id: _,
            chunk_id: _,
            compression: _,
            uncompressed_size: _,
            encoding: _,
            payload,
            is_static: _,
        } = arrow_msg;

        let ipc_schema_size_bytes = {
            // NOTE: This is based on the implementation of `arrow::ipc::convert::try_schema_from_ipc_buffer`.

            const CONTINUATION_MARKER: [u8; 4] = [0xff; 4];

            anyhow::ensure!(
                payload.len() >= 4,
                "The payload length is less than 4 and missing the continuation marker or length of payload"
            );

            let (len, _payload) = if payload[..4] == CONTINUATION_MARKER {
                anyhow::ensure!(
                    payload.len() >= 8,
                    "The payload length is less than 8 and missing the length of payload"
                );
                payload[4..].split_at(4)
            } else {
                payload.split_at(4)
            };

            let len = <i32>::from_le_bytes(len.try_into()?);
            anyhow::ensure!(
                len >= 0,
                "The encapsulated message's reported length is negative ({len})"
            );

            len as u64
        };

        let app = if app {
            let decoded = arrow_msg.to_application(())?;

            let schema = decoded.batch.schema();

            let entity_path = {
                let entity_path = schema
                    .metadata()
                    .get(re_sorbet::metadata::SORBET_ENTITY_PATH);
                let entity_path =
                    entity_path.or_else(|| schema.metadata().get("rerun.entity_path"));
                entity_path.map(ToOwned::to_owned).unwrap_or_default()
            };

            // TODO(cmc): shortest and longest range covered per timeline would be welcome addition,
            // something like the following, but generic:
            if false && let Some(log_tick) = decoded.batch.column_by_name("log_tick") {
                let log_tick = log_tick
                    .as_any()
                    .downcast_ref::<arrow::array::Int64Array>()
                    .ok_or_else(|| anyhow::anyhow!("`log_tick` is not a Int64Array, somehow"))?;
                let _min = log_tick.values().iter().copied().min().unwrap_or_default();
                let _max = log_tick.values().iter().copied().max().unwrap_or_default();
            }

            let indexes = schema
                .fields
                .iter()
                .filter(|&field| {
                    field
                        .metadata()
                        .get(re_sorbet::metadata::RERUN_KIND)
                        .map(|s| s.as_str())
                        == Some("index")
                        || field.metadata().get("rerun.kind").map(|s| s.as_str()) == Some("index")
                })
                .map(|field| field.name().to_owned())
                .collect_vec();
            let num_indexes = indexes.len() as _;

            let components = schema
                .fields
                .iter()
                .filter(|&field| {
                    field
                        .metadata()
                        .get(re_sorbet::metadata::RERUN_KIND)
                        .map(|s| s.as_str())
                        == Some("data")
                        || field.metadata().get("rerun.kind").map(|s| s.as_str()) == Some("data")
                })
                .map(|field| field.name().to_owned())
                .collect_vec();
            let num_components = components.len() as _;

            // Promote the batch to a `Chunk` so we can inspect per-timeline sortedness.
            // Errors here mean the chunk is malformed in some other way — surface as an
            // empty list rather than failing the whole stats run.
            let timeline_sortedness = match Chunk::from_arrow_msg(&decoded) {
                Ok(chunk) => chunk
                    .timelines()
                    .iter()
                    .map(|(name, tc)| (chunk.entity_path().clone(), *name, tc.is_sorted()))
                    .collect(),
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to promote ArrowMsg into a Chunk for sorted-timeline check: {err}"
                    );
                    Vec::new()
                }
            };

            Some(ChunkStatsApplication {
                entity_path,

                indexes,
                components,

                num_rows: decoded.batch.num_rows() as _,
                num_indexes,
                num_components,

                timeline_sortedness,
            })
        } else {
            None
        };

        let ipc_size_bytes_uncompressed = payload.len() as u64;
        return Ok(Some(ChunkStats {
            app,
            transport: ChunkStatsTransport {
                ipc_size_bytes_compressed: compressed_size,
                ipc_size_bytes_uncompressed,

                ipc_schema_size_bytes,
                ipc_data_size_bytes: ipc_size_bytes_uncompressed
                    .saturating_sub(ipc_schema_size_bytes),
            },
        }));
    }

    Ok(None)
}
