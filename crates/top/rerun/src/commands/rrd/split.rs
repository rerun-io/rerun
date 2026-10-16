use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use anyhow::Context as _;
use arrow::compute::kernels::cast_utils::Parser as _;
use itertools::Itertools as _;

use re_build_info::CrateVersion;
use re_chunk::external::crossbeam;
use re_chunk::{Chunk, TimeInt, TimelineName};
use re_chunk_store::{ChunkStore, ChunkTrackingMode};
use re_log_types::AbsoluteTimeRange;
use re_quota_channel::send_crossbeam;
use re_sdk::external::arrow;
use re_sdk::{StoreId, StoreKind, Timeline};

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

// TODO(RR-3810): There exists an optimal version of this that uses virtual chunk stores instead of physical
// chunk stores, and only goes physical for chunks that require it (anything that could benefit
// from slicing, meaning it sits across 2 or more splits, as well as keyframes & transforms special cases).

// TODO(RR-3819): Support auto-splitting by size instead of time.

#[derive(Debug, Clone, clap::Parser)]
pub struct SplitCommand {
    /// Path to read from.
    path_to_input_rrd: String,

    /// Path to the output directory. All generated RRD files will end up there.
    #[arg(short = 'o', long = "output-dir", value_name = "output directory")]
    path_to_output_dir: String,

    /// The timeline used to compute the splits.
    ///
    /// The other timelines will be kept in the output, which might or might not make sense
    /// depending on the density of the dataset.
    /// Use `--drop-unused-timelines` to discard them.
    #[clap(long = "timeline")]
    timeline: String,

    /// The timestamps at which to perform the splits. Incompatible with `--num-parts`/`-n`.
    ///
    /// There are always `number_of_times + 1` resulting splits.
    ///
    /// For example, given `-t 10 -t 20 -t 30`, this command will output 4 splits: [-inf:10), [10:20), [20:30), [30:+inf).
    //
    // NOTE: This is a string because we expect the timestamps to come in whatever in the most
    // natural format for them, depending on the selected timeline.
    #[arg(short = 't', long = "time", conflicts_with = "num_parts")]
    times: Vec<String>,

    /// The number of parts to split the recording into. Incompatible with `--time`/`-t`.
    ///
    /// There will be exactly that number of resulting splits. Each split will cover an equal time
    /// span in the timeline.
    #[arg(short = 'n', long = "num-parts", conflicts_with = "times")]
    num_parts: Option<u32>,

    /// The recording ID prefix to be used for the output recordings.
    ///
    /// If left unspecified, the ID of the original recording, suffixed with a `-`, will be used
    /// as a prefix.
    ///
    /// Each split will use `<recording_id_prefix><i>` as their respective recording ID, where `i`
    /// is the index of the split.
    //
    // TODO(cmc): Too many video decoding problems come up if we allow shared recording IDs, and so we
    // don't, at least for now.
    #[arg(long = "recording-id", value_name = "recording ID prefix")]
    recording_id_prefix: Option<String>,

    /// If true, timelines other than the one specified with `--timeline` will be discarded.
    #[clap(long = "drop-unused-timelines")]
    discard_unused_timelines: bool,
}

impl SplitCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrd,
            path_to_output_dir,
            timeline: _,
            times,
            num_parts,
            recording_id_prefix,
            discard_unused_timelines: _,
        } = self;

        let input_rrd_stem = PathBuf::from(&path_to_input_rrd)
            .file_stem()
            .with_context(|| {
                format!("couldn't grab file stem from input RRD file {path_to_input_rrd:?}")
            })?
            .to_string_lossy()
            .to_string();

        anyhow::ensure!(
            !times.is_empty() || num_parts.is_some(),
            "must specify at least one of --time or --num-parts"
        );

        if let Some(num_parts) = num_parts {
            anyhow::ensure!(
                *num_parts > 1,
                "must specify 2 splits or more, found {num_parts} instead"
            );
        }

        let now = std::time::Instant::now();
        re_log::info!(srcs = ?path_to_input_rrd, "split started");

        // TODO(RR-941): multi-recording RRD files need to go away.
        let mut stores = BTreeMap::new();

        // NOTE: We enforce the use of a filename in this case, so there is no `or_stdin` involved, ever.
        let (rx_decoder, rx_size_bytes) =
            read_rrd_streams_from_file_or_stdin(std::slice::from_ref(path_to_input_rrd));

        // We need to keep track of the non-data messages (stateful store switches, blueprint
        // activations), so that we can properly rebuild the final RRD files.
        //
        // TODO(RR-1075): recordings should not contain anything but stateless data.
        let mut meta_messages: HashMap<StoreId, Vec<re_log_msg::LogMsg>> = HashMap::default();

        {
            // Load all the data & metadata for all the stores present in the file.

            re_log::info!("processing input…");

            let mut current_store_id = None;
            let mut last_checkpoint = std::time::Instant::now();
            for (msg_nr, (_source, res)) in rx_decoder.iter().enumerate() {
                match res {
                    Ok(msg) => match &msg {
                        re_log_msg::LogMsg::SetStoreInfo(set_store_info) => {
                            let store_id = set_store_info.info.store_id.clone();
                            current_store_id = Some(store_id.clone());

                            meta_messages.entry(store_id.clone()).or_default().push(msg);

                            stores.entry(store_id.clone()).or_insert_with(|| {
                                ChunkStore::new(
                                    store_id,
                                    re_chunk_store::ChunkStoreConfig::ALL_DISABLED,
                                )
                            });
                        }

                        re_log_msg::LogMsg::ArrowMsg(store_id, msg) => {
                            let Some(store) = stores.get_mut(store_id) else {
                                anyhow::bail!("unknown store ID: {store_id:?}");
                            };
                            let chunk = Chunk::from_arrow_msg(msg)?;
                            store.insert_chunk(&Arc::new(chunk))?;
                        }

                        re_log_msg::LogMsg::BlueprintActivationCommand(_) => {
                            let Some(current_store_id) = current_store_id.clone() else {
                                re_log::warn!(
                                    "found BlueprintActivationCommand without an active store, discarding"
                                );
                                continue;
                            };

                            meta_messages.entry(current_store_id).or_default().push(msg);
                        }
                    },

                    Err(err) => {
                        re_log::error!(err = re_error::format(err));
                    }
                }

                let msg_count = msg_nr + 1;
                let check_in_interval = 10_000;
                if msg_count % check_in_interval == 0 {
                    let msg_per_second =
                        check_in_interval as f64 / last_checkpoint.elapsed().as_secs_f64();
                    last_checkpoint = std::time::Instant::now();
                    re_log::info!(
                        "processed {msg_count} messages so far, current speed is {msg_per_second:.2} msg/s"
                    );
                    re_tracing::reexports::puffin::GlobalProfiler::lock().new_frame();
                }
            }
        }

        let (cutoff_timeline, cutoff_times) = self.compute_cutoff_times(&stores)?;
        re_log::info!(
            cutoff_timeline = %cutoff_timeline.name(),
            cutoff_times = cutoff_times.iter().map(|t| time_to_human_string(cutoff_timeline, *t)).join(", "),
            "extracted cutoff times",
        );

        // This block computes the list of output paths for the newly split recordings.
        // There are always exactly as many output paths as there are cutoff times.
        let path_to_output_rrds = {
            let path_to_output_dir = PathBuf::from(path_to_output_dir.clone());

            cutoff_times
                .iter()
                .copied()
                .tuple_windows::<(_, _)>()
                .map(|(t1, t2)| {
                    let filename = format!(
                        "{input_rrd_stem}_{}__{}.rrd",
                        time_to_human_string(cutoff_timeline, t1),
                        time_to_human_string(cutoff_timeline, t2),
                    );

                    path_to_output_dir
                        .join(filename)
                        .to_string_lossy()
                        .to_string()
                })
                .collect_vec()
        };

        re_log::debug_assert!(
            cutoff_times.len() == path_to_output_rrds.len() + 1,
            "there must always be as many cutoff times as there are output paths (plus 1): got {} times for {} paths instead",
            cutoff_times.len(),
            path_to_output_rrds.len() + 1,
        );

        re_log::info!(?path_to_output_rrds, "encoding…");

        let (txs_encoding, rxs_encoding): (Vec<_>, Vec<_>) =
            std::iter::repeat_with(|| crossbeam::channel::bounded(16))
                .take(path_to_output_rrds.len())
                .unzip();

        type Receiver = re_log::Receiver<(StoreId, Vec<re_log_msg::LogMsg>)>;
        let spawn_encoding_thread = move |split_idx, path: String, msgs: Receiver| {
            std::thread::Builder::new()
                .name(format!("rerun-rrd-split-out-{split_idx}"))
                .spawn({
                    let recording_id_prefix = recording_id_prefix.clone();
                    move || -> anyhow::Result<(String, u64)> {
                        use std::io::Write as _;

                        let mut rrd_out = std::io::BufWriter::new(
                            std::fs::File::create(&path).with_context(|| format!("{path:?}"))?,
                        );

                        let mut encoder = {
                            // TODO(cmc): encoding options & version should match the original.
                            let version = CrateVersion::LOCAL;
                            let options =
                                re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
                            re_log_encoding::Encoder::new_eager(version, options, &mut rrd_out)
                                .context("couldn't init encoder")?
                        };

                        let mut size_bytes = 0;
                        for (store_id, msgs) in msgs {
                            let new_store_id = {
                                if let Some(recording_id_prefix) = recording_id_prefix.as_deref() {
                                    store_id.with_recording_id(format!(
                                        "{recording_id_prefix}{split_idx}"
                                    ))
                                } else {
                                    store_id.clone().with_recording_id(format!(
                                        "{}-{split_idx}",
                                        store_id.recording_id()
                                    ))
                                }
                            };

                            for mut msg in msgs {
                                if new_store_id.kind() != StoreKind::Blueprint {
                                    match &mut msg {
                                        re_log_msg::LogMsg::SetStoreInfo(info) => {
                                            info.info.store_id = new_store_id.clone();
                                        }

                                        re_log_msg::LogMsg::ArrowMsg(id, _) => {
                                            *id = new_store_id.clone();
                                        }

                                        re_log_msg::LogMsg::BlueprintActivationCommand(_) => {}
                                    }
                                }

                                size_bytes += encoder.append(&msg).context("encoding failure")?;
                            }
                        }

                        drop(encoder);
                        rrd_out.flush().context("couldn't flush output")?;

                        Ok((path.clone(), size_bytes))
                    }
                })
        };

        let encoding_handles = (0..path_to_output_rrds.len())
            .map(|i| {
                spawn_encoding_thread(i, path_to_output_rrds[i].clone(), rxs_encoding[i].clone())
            })
            .collect_vec();

        for (store_id, store) in stores {
            if let Some(msgs) = meta_messages.remove(&store_id) {
                for tx in &txs_encoding {
                    send_crossbeam(tx, (store_id.clone(), msgs.clone()))?;
                }
            }

            if store_id.kind() == StoreKind::Blueprint {
                // Splitting blueprint recordings doesn't make sense: just forward them as-is *into every split*.

                let chunks = store
                    .iter_physical_chunks()
                    .map(|chunk| {
                        Ok(re_log_msg::LogMsg::ArrowMsg(
                            store_id.clone(),
                            re_log_msg::ArrowMsg {
                                batch: std::sync::Arc::new(chunk.to_chunk_batch()?),
                                on_release: None,
                            },
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                // Make sure to do the forwarding for *every* split! They all need the blueprint!
                for tx in &txs_encoding {
                    send_crossbeam(tx, (store_id.clone(), chunks.clone()))?;
                }

                continue;
            }

            self.split_store(&store, cutoff_timeline, &cutoff_times, &txs_encoding)?;
        }

        let rrd_in_size = rx_size_bytes.recv().ok().map(|(size, _footers)| size);

        std::mem::drop(txs_encoding);

        let file_size_to_string = |size: Option<u64>| {
            size.map_or_else(
                || "<unknown>".to_owned(),
                |size| re_format::format_bytes(size as _),
            )
        };

        let mut rrd_out_paths = Vec::new();
        let mut rrd_out_sizes = Vec::new();
        for handle in encoding_handles {
            let (rrd_out_path, rrd_out_size) = handle
                .context("couldn't spawn IO thread")?
                .join()
                .map_err(|err| anyhow::anyhow!("Unknown error: {err:?}"))??; // NOLINT: there is no `Display` for this `err`
            rrd_out_paths.push(rrd_out_path);
            rrd_out_sizes.push(file_size_to_string(Some(rrd_out_size)));
        }

        re_log::info!(
            src = path_to_input_rrd,
            src_size_bytes = %file_size_to_string(rrd_in_size),
            dsts = ?rrd_out_paths,
            dsts_size_bytes = ?rrd_out_sizes,
            time = ?now.elapsed(),
            "split finished"
        );

        Ok(())
    }

    /// Computes a list of cutoff times that will decide when and where the recordings get split.
    ///
    /// There are always as many splits (i.e. output files) as there are timestamps in this list, plus 1.
    /// The splits are inclusive on their leftmost bound and exclusive on their rightmost bound.
    ///
    /// For example, if `times == [t1, t2, t3]`, the final output of this script will be:
    /// * recording0: [$min:t1)
    /// * recording1: [t1:t2)
    /// * recording2: [t2:t3)
    /// * recording3: [t3:$max)
    ///
    /// Returns `(cutoff_timeline, cutoff_times)`.
    fn compute_cutoff_times(
        &self,
        stores: &BTreeMap<StoreId, ChunkStore>,
    ) -> anyhow::Result<(Timeline, Vec<TimeInt>)> {
        let Self {
            path_to_input_rrd: _,
            path_to_output_dir: _,
            timeline,
            times,
            num_parts,
            recording_id_prefix: _,
            discard_unused_timelines: _,
        } = self;

        let cutoff_timeline = {
            // We need to know about all the timelines that exist, and most importantly we need to know
            // what their physical type actually is (duration, timestamp, tick, etc), so that we can
            // parse the CLI parameters appropriately.
            //
            // Note that this is across *all recordings* in the file/stream.
            let mut known_timelines: BTreeMap<TimelineName, Timeline> = Default::default();
            for (name, timeline) in stores.values().flat_map(|store| store.schema().timelines()) {
                if let Some(existing) = known_timelines.insert(name, timeline) {
                    anyhow::ensure!(
                        existing == timeline,
                        "found incompatible timeline in multi-recording file: {existing:?} vs. {timeline:?}",
                    );
                }
            }

            let Some(cutoff_timeline) = TimelineName::try_new(timeline)
                .ok()
                .and_then(|name| known_timelines.remove(&name))
            else {
                anyhow::bail!(
                    "timeline '{timeline}' does not exist in the input recording, available timelines are {}",
                    known_timelines.keys().map(|name| name.as_str()).join(", ")
                );
            };

            cutoff_timeline
        };

        re_log::info!(
            name = %cutoff_timeline.name(),
            typ = %cutoff_timeline.typ(),
            "extracted cutoff timeline information",
        );

        let mut min_time: Option<i64> = None;
        let mut max_time: Option<i64> = None;

        // We merge the results across all stores because we want the different recordings in
        // the file to still temporarily align in the output.
        for store in stores.values() {
            if let Some(time_range) = store.time_range(cutoff_timeline.name()) {
                let cur_min_time = time_range.min().as_i64();
                let min_time = min_time.get_or_insert(cur_min_time);
                *min_time = i64::min(*min_time, cur_min_time);

                let cur_max_time = time_range.max().as_i64();
                let max_time = max_time.get_or_insert(cur_max_time);
                *max_time = i64::max(*max_time, cur_max_time);
            }
        }

        let Some(min_time) = min_time else {
            anyhow::bail!("timeline '{timeline}' does not contain any data");
        };
        let Some(max_time) = max_time else {
            anyhow::bail!("timeline '{timeline}' does not contain any data");
        };

        // Because user-facing ranges are exclusive on the upper bound.
        let max_time = max_time.saturating_add(1);

        let cutoff_times = if let Some(num_parts) = num_parts {
            let time_span = max_time.saturating_sub(min_time) / *num_parts as i64;
            let mut cur_time = min_time;

            std::iter::chain(
                (0..*num_parts as u64).map(|_| {
                    let t = cur_time;
                    cur_time += time_span;
                    TimeInt::new_temporal(t)
                }),
                std::iter::once(TimeInt::new_temporal(max_time)),
            )
            .collect()
        } else if !times.is_empty() {
            let times = times
                .iter()
                .map(|time_str| time_from_human_string(cutoff_timeline, time_str))
                .collect::<anyhow::Result<Vec<_>>>()?;

            itertools::chain!(
                [TimeInt::new_temporal(min_time)],
                times,
                [TimeInt::new_temporal(max_time)]
            )
            .collect()
        } else {
            anyhow::bail!("unreachable");
        };

        Ok((cutoff_timeline, cutoff_times))
    }

    /// Optimally split a [`ChunkStore`] at the specified `cutoff_times`.
    ///
    /// Video keyframes and transform coordinate frames will be dealt with appropriately,
    /// see [`ChunkStore::split_by_time_ranges`].
    fn split_store(
        &self,
        store: &ChunkStore,
        cutoff_timeline: Timeline,
        cutoff_times: &[TimeInt],
        txs_encoding: &[re_log::Sender<(StoreId, Vec<re_log_msg::LogMsg>)>],
    ) -> anyhow::Result<()> {
        let ranges = cutoff_times
            .iter()
            .copied()
            .tuple_windows::<(_, _)>()
            .map(|(start_inclusive, end_exclusive)| {
                AbsoluteTimeRange::new(start_inclusive, end_exclusive.dec())
            })
            .collect_vec();

        #[cfg(feature = "video")]
        let is_start_of_gop = crate::rrd::gop_detector();

        let splits = store.split_by_time_ranges(
            ChunkTrackingMode::PanicOnMissing,
            cutoff_timeline.name(),
            &ranges,
            cfg_select! {
                feature = "video" => Some(&is_start_of_gop),
                _ => None,
            },
        );

        for (i, all_chunks_in_split) in splits.into_iter().enumerate() {
            let mut all_unique_indexes_in_split = HashSet::default();

            send_crossbeam(
                &txs_encoding[i],
                (
                    store.id(),
                    all_chunks_in_split
                        .into_iter()
                        .map(move |(original_chunk_id, chunk)| {
                            (
                                original_chunk_id,
                                if self.discard_unused_timelines {
                                    chunk.timeline_sliced(*cutoff_timeline.name())
                                } else {
                                    chunk
                                },
                            )
                        })
                        .inspect(|(_original_chunk_id, chunk)| {
                            if cfg!(debug_assertions) {
                                for (time, row_id) in chunk.iter_indices(cutoff_timeline.name()) {
                                    if !all_unique_indexes_in_split.insert((time, row_id)) {
                                        eprintln!(
                                            "ERROR: duplicate index detected on {}: {} / {row_id}",
                                            chunk.entity_path(),
                                            time_to_human_string(cutoff_timeline, time),
                                        );
                                    }
                                }
                            }
                        })
                        .map(move |(original_chunk_id, chunk)| {
                            (
                                original_chunk_id,
                                re_log_msg::LogMsg::ArrowMsg(
                                    store.id(),
                                    re_log_msg::ArrowMsg {
                                        batch: std::sync::Arc::new(
                                            chunk
                                                .to_chunk_batch()
                                                .expect("we got it in, surely we can get it out"),
                                        ),
                                        on_release: None,
                                    },
                                ),
                            )
                        })
                        .map(|(_, chunk)| chunk)
                        .collect(),
                ),
            )?;
        }

        Ok(())
    }
}

// ---

fn time_from_human_string(timeline: Timeline, time_str: &str) -> anyhow::Result<TimeInt> {
    // Users of this CLI most likely got their timestamps from one of the many places where we
    // display Arrow data, and therefore the best way to parse it is to use the appropriate
    // Arrow parser.
    let mut time_parsed = match timeline.typ() {
        re_log_types::TimeType::Sequence => arrow::datatypes::Int64Type::parse(time_str),
        re_log_types::TimeType::DurationNs => {
            arrow::datatypes::DurationNanosecondType::parse(time_str)
        }
        re_log_types::TimeType::TimestampNs => {
            arrow::datatypes::TimestampNanosecondType::parse(time_str)
        }
    };

    // If using the appropriate parser failed, then try as an int64, unconditionally.
    // Maybe we get lucky.
    if time_parsed.is_none() {
        time_parsed = arrow::datatypes::Int64Type::parse(time_str);
    }

    let time_parsed = time_parsed.ok_or_else(|| {
        anyhow::anyhow!(
            "'{time_str}' is not a valid value for a {:?} timeline",
            timeline.typ()
        )
    })?;

    Ok(TimeInt::new_temporal(time_parsed))
}

fn time_to_human_string(timeline: Timeline, time: TimeInt) -> String {
    // Arrow doesn't expose any easy way to re-use its internal formatters for simple scalars, so
    // just do whatever we can. It's for the filenames anyway, so the more control we have the better.
    let s = match timeline.typ() {
        re_log_types::TimeType::Sequence => time.as_i64().to_string(),

        re_log_types::TimeType::DurationNs => {
            format!("{:?}", std::time::Duration::from_nanos(time.as_i64() as _))
        }

        re_log_types::TimeType::TimestampNs => {
            if let Ok(ts) = jiff::Timestamp::from_nanosecond(time.as_i64() as _) {
                ts.to_string()
            } else {
                time.as_i64().to_string()
            }
        }
    };

    s.replace(['.', ':', ' '], "_") // just in case
}
