use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use anyhow::Context as _;
use arrow::compute::kernels::cast_utils::Parser as _;
use itertools::Itertools as _;

use re_build_info::CrateVersion;
use re_chunk::external::crossbeam;
use re_chunk::{Chunk, ChunkId, RowId, TimeInt, TimelineName};
use re_chunk_store::{ChunkStore, ChunkTrackingMode};
use re_sdk::external::arrow;
use re_sdk::external::nohash_hasher::IntMap;
use re_sdk::{Archetype as _, ComponentIdentifier, EntityPath, StoreId, StoreKind, Timeline};

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

// TODO(RR-3809): we should expose a `ChunkStore::split()` API.

// TODO(RR-3810): There exists an optimal version of this that uses virtual chunk stores instead of physical
// chunk stores, and only goes physical for chunks that require it (anything that could benefit
// from slicing, meaning it sits across 2 or more splits, as well as keyframes & transforms special cases).

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

    /// The number of parts to split the recording into. Incompatible with `--times`/`-t`.
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
    // TODO: issue for splitting by size
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
                format!("coudln't grab file stem from input RRD file {path_to_input_rrd:?}")
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
        let mut meta_messages: HashMap<StoreId, Vec<re_log_types::LogMsg>> = HashMap::default();

        {
            // Load all the data & metadata for all the stores present in the file.

            re_log::info!("processing input…");

            let mut current_store_id = None;
            let mut last_checkpoint = std::time::Instant::now();
            for (msg_nr, (_source, res)) in rx_decoder.iter().enumerate() {
                match res {
                    Ok(msg) => match &msg {
                        re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
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

                        re_log_types::LogMsg::ArrowMsg(store_id, msg) => {
                            let Some(store) = stores.get_mut(store_id) else {
                                anyhow::bail!("unknown store ID: {store_id:?}");
                            };
                            let chunk = Chunk::from_arrow_msg(msg)?;
                            store.insert_chunk(&Arc::new(chunk))?;
                        }

                        re_log_types::LogMsg::BlueprintActivationCommand(_) => {
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

        re_log::info!("extracting keyframes…");
        let mut keyframes_per_entity: IntMap<_, Vec<_>> = IntMap::default();
        for store in stores.values() {
            for entity in store.all_entities() {
                let keyframes = extract_keyframes(&entity, store, cutoff_timeline);
                if !keyframes.is_empty() {
                    keyframes_per_entity
                        .entry(entity.clone())
                        .or_default()
                        .extend(keyframes);
                }
            }
        }
        re_log::info!(
            timeline = %cutoff_timeline.name(),
            keyframes = ?keyframes_per_entity
                .iter()
                .map(|(entity, keyframes)| (entity, keyframes.len()))
                .collect_vec(),
            "extracted video keyframes"
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

        type Receiver = re_log::Receiver<(StoreId, Vec<re_log_types::LogMsg>)>;
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
                                        re_log_types::LogMsg::SetStoreInfo(info) => {
                                            info.info.store_id = new_store_id.clone();
                                        }

                                        re_log_types::LogMsg::ArrowMsg(id, _) => {
                                            *id = new_store_id.clone();
                                        }

                                        re_log_types::LogMsg::BlueprintActivationCommand(_) => {}
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
                    tx.send((store_id.clone(), msgs.clone()))?;
                }
            }

            if store_id.kind() == StoreKind::Blueprint {
                // Splitting blueprint recordings doesn't make sense: just forward them as-is *into every split*.

                let chunks = store
                    .iter_physical_chunks()
                    .map(|chunk| {
                        Ok(re_log_types::LogMsg::ArrowMsg(
                            store_id.clone(),
                            re_log_types::ArrowMsg {
                                chunk_id: *chunk.id(),
                                batch: chunk.to_record_batch()?,
                                on_release: None,
                            },
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                // Make sure to do the forwarding for *every* split! They all need the blueprint!
                for tx in &txs_encoding {
                    tx.send((store_id.clone(), chunks.clone()))?;
                }

                continue;
            }

            self.split_store(
                &store,
                cutoff_timeline,
                &cutoff_times,
                &keyframes_per_entity,
                &txs_encoding,
            )?;
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
    /// * recording0: [<min>:t1)
    /// * recording1: [t1:t2)
    /// * recording2: [t2:t3)
    /// * recording3: [t3:<max>)
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
            for (name, timeline) in stores.values().flat_map(|store| store.timelines()) {
                if let Some(existing) = known_timelines.insert(name, timeline) {
                    anyhow::ensure!(
                        existing == timeline,
                        "found incompatible timeline in multi-recording file: {existing:?} vs. {timeline:?}",
                    );
                }
            }

            let Some(cutoff_timeline) = known_timelines.remove(&timeline.as_str().into()) else {
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
        let mut max_len: Option<u64> = None;

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

                let cur_max_len = time_range.abs_length();
                let max_len = max_len.get_or_insert(cur_max_len);
                *max_len = u64::max(*max_len, cur_max_len);
            }
        }

        let Some(min_time) = min_time else {
            anyhow::bail!("timeline '{timeline}' does not contain any data");
        };
        let Some(max_time) = max_time else {
            anyhow::bail!("timeline '{timeline}' does not contain any data");
        };
        let Some(max_len) = max_len else {
            anyhow::bail!("timeline '{timeline}' does not contain any data");
        };

        // Because user-facing ranges are exclusive on the upper bound.
        let max_time = max_time.saturating_add(1);

        let cutoff_times = if let Some(num_parts) = num_parts {
            let num_parts = *num_parts as u64;

            let time_span: i64 = (max_len / num_parts).try_into().expect("cannot be OOB");
            let mut cur_time = min_time;

            (0..num_parts)
                .map(|_| {
                    let t = cur_time;
                    cur_time += time_span;
                    TimeInt::new_temporal(t)
                })
                .chain(std::iter::once(TimeInt::new_temporal(max_time)))
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
    /// Video keyframes and transform coordinate frames will be dealt with appropriately.
    //
    // TODO(RR-3809): implement this on `ChunkStore` directly, with a test suite.
    fn split_store(
        &self,
        store: &ChunkStore,
        cutoff_timeline: Timeline,
        cutoff_times: &[TimeInt],
        keyframes_per_entity: &IntMap<EntityPath, Vec<TimeInt>>,
        txs_encoding: &[re_log::Sender<(StoreId, Vec<re_log_types::LogMsg>)>],
    ) -> anyhow::Result<()> {
        // `VideoStream`s must be split on a keyframe, always.
        //
        // The solution is to find the closest past keyframe, and then duplicate the entire stream
        // from there up to the cutoff point. See `extract_keyframes`.
        let video_sample_identifier =
            re_sdk_types::archetypes::VideoStream::descriptor_sample().component;

        // `Transform3D`s can be multiplexed on a single entity stream using `CoordinateFrame`s,
        // making them completely opaque to the query engine.
        //
        // The solution is to always duplicate the entire stream up to the cutoff point whenever that happens.
        let transform_parent_frame_identifier =
            re_sdk_types::archetypes::Transform3D::descriptor_parent_frame().component;
        let transform_child_frame_identifier =
            re_sdk_types::archetypes::Transform3D::descriptor_child_frame().component;

        // `Pinhole`s can be multiplexed on a single entity stream using `CoordinateFrame`s,
        // making them completely opaque to the query engine.
        //
        // The solution is to always duplicate the entire stream up to the cutoff point whenever that happens.
        let pinhole_parent_frame_identifier =
            re_sdk_types::archetypes::Pinhole::descriptor_parent_frame().component;
        let pinhole_child_frame_identifier =
            re_sdk_types::archetypes::Pinhole::descriptor_child_frame().component;

        let all_entities_and_their_components = store
            .all_entities()
            .into_iter()
            .filter_map(|entity| {
                store
                    .all_components_for_entity(&entity)
                    .map(|components| (entity, components))
            })
            .collect_vec();

        for (i, cutoff_time) in cutoff_times
            .iter()
            .take(cutoff_times.len() - 1)
            .copied()
            .enumerate()
        {
            let mut all_chunks_in_split = Vec::new();

            // NOTE: Keep in mind, the way we do deduplication here only works because we enforce a single timeline.
            // It is much, much harder to deduplicate appropriately across multiple timelines at once.
            let mut all_chunk_ids_in_split: HashSet<ChunkId> = HashSet::default();

            re_log::debug!(
                cutoff_timeline = %cutoff_timeline.name(),
                cutoff_time = %time_to_human_string(cutoff_timeline, cutoff_time),
                "splitting…"
            );

            for (entity, components) in &all_entities_and_their_components {
                let start_inclusive = cutoff_time;
                let end_exclusive = cutoff_times.get(i + 1).copied().unwrap_or(TimeInt::MAX);

                // Base case: everything
                {
                    let components = components.iter().copied().collect();
                    let chunks = extract_chunks_for_single_split(
                        store,
                        entity,
                        &components,
                        cutoff_timeline,
                        start_inclusive,
                        end_exclusive,
                    );
                    all_chunks_in_split.extend(chunks);
                }

                // Special case: video keyframes
                {
                    let cutoff_time_revised = keyframes_per_entity.get(entity).and_then(|keyframes| {
                        let p = keyframes
                            .partition_point(|t| *t <= cutoff_time)
                            .saturating_sub(1);
                        let cutoff_time_revised = keyframes[p];

                        if cutoff_time_revised < cutoff_time {
                            re_log::info!(
                                %entity,
                                cutoff_timeline = %cutoff_timeline.name(),
                                cutoff_time = %time_to_human_string(cutoff_timeline, cutoff_time),
                                cutoff_time_revised = %time_to_human_string(cutoff_timeline, cutoff_time_revised),
                                "revising cutoff time to match video keyframe…"
                            );
                            Some(cutoff_time_revised)
                        } else {
                            None
                        }
                    });

                    if let Some(cutoff_time_revised) = cutoff_time_revised {
                        let components = std::iter::once(video_sample_identifier).collect();
                        let chunks = extract_chunks_for_single_split(
                            store,
                            entity,
                            &components,
                            cutoff_timeline,
                            cutoff_time_revised,
                            start_inclusive,
                        );
                        all_chunks_in_split.extend(chunks);
                    }
                }

                // Special case: transforms with multiplexed coordinate frames
                let entity_has_multiplexed_transforms_on_timeline =
                    store.entity_has_component_on_timeline(
                        cutoff_timeline.name(),
                        entity,
                        transform_parent_frame_identifier,
                    ) || store.entity_has_component_on_timeline(
                        cutoff_timeline.name(),
                        entity,
                        transform_child_frame_identifier,
                    );
                if entity_has_multiplexed_transforms_on_timeline {
                    re_log::info!(
                        %entity,
                        cutoff_timeline = %cutoff_timeline.name(),
                        cutoff_time = %time_to_human_string(cutoff_timeline, cutoff_time),
                        "gathering all transforms up to cutoff point due to multiplexed coordinate frames…"
                    );

                    let components =
                        re_sdk_types::archetypes::Transform3D::all_component_identifiers()
                            .collect();
                    let chunks = extract_chunks_for_single_split(
                        store,
                        entity,
                        &components,
                        cutoff_timeline,
                        TimeInt::MIN,
                        start_inclusive,
                    );
                    all_chunks_in_split.extend(chunks);
                }

                // Special case: pinholes with multiplexed coordinate frames
                let entity_has_multiplexed_pinholes_on_timeline =
                    store.entity_has_component_on_timeline(
                        cutoff_timeline.name(),
                        entity,
                        pinhole_parent_frame_identifier,
                    ) || store.entity_has_component_on_timeline(
                        cutoff_timeline.name(),
                        entity,
                        pinhole_child_frame_identifier,
                    );
                if entity_has_multiplexed_pinholes_on_timeline {
                    re_log::info!(
                        %entity,
                        cutoff_timeline = %cutoff_timeline.name(),
                        cutoff_time = %time_to_human_string(cutoff_timeline, cutoff_time),
                        "gathering all pinholes up to cutoff point due to multiplexed coordinate frames…"
                    );

                    let components =
                        re_sdk_types::archetypes::Pinhole::all_component_identifiers().collect();
                    let chunks = extract_chunks_for_single_split(
                        store,
                        entity,
                        &components,
                        cutoff_timeline,
                        TimeInt::MIN,
                        start_inclusive,
                    );
                    all_chunks_in_split.extend(chunks);
                }
            }

            // We must make sure that the new recordings have their chunks in the same order as the original
            // one (as dictated by their chunk IDs), otherwise the loading experience would be absolutely awful,
            // given that we've just queried the data per-entity per-component.
            all_chunks_in_split.sort_by_key(|(original_chunk_id, _)| *original_chunk_id);

            txs_encoding[i].send((
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
                    // Many of the components will share the same chunks, so make sure to deduplicate before
                    // forwarding to the output.
                    // This works because we make sure to generate new IDs when the slices we compute require
                    // it, which is itself manageable because we enforce a single timeline throughout.
                    .filter(|(_original_chunk_id, chunk)| all_chunk_ids_in_split.insert(chunk.id()))
                    .map(move |(original_chunk_id, chunk)| {
                        (
                            original_chunk_id,
                            re_log_types::LogMsg::ArrowMsg(
                                store.id(),
                                re_log_types::ArrowMsg {
                                    chunk_id: *chunk.id(),
                                    batch: chunk
                                        .to_record_batch()
                                        .expect("we got it in, surely we can get it out"),
                                    on_release: None,
                                },
                            ),
                        )
                    })
                    .map(|(_, chunk)| chunk)
                    .collect(),
            ))?;
        }

        Ok(())
    }
}

// TODO(RR-3810): For a virtual store implementation, we'd want this to load no more than 1 chunk at a time.
fn extract_keyframes(
    entity_path: &EntityPath,
    store: &ChunkStore,
    cutoff_timeline: Timeline,
) -> Vec<TimeInt> {
    let codec = {
        let codec_identifier = re_sdk_types::archetypes::VideoStream::descriptor_codec().component;
        let results = store.latest_at_relevant_chunks(
            re_chunk_store::ChunkTrackingMode::PanicOnMissing,
            &re_chunk_store::LatestAtQuery::new(*cutoff_timeline.name(), TimeInt::MAX),
            entity_path,
            codec_identifier,
        );

        results
            .chunks
            .iter()
            .flat_map(|chunk| {
                chunk.iter_component::<re_sdk_types::components::VideoCodec>(codec_identifier)
            })
            .find_map(|data| data.as_slice().first().copied())
    };

    let Some(codec) = codec else {
        return vec![];
    };

    let sample_identifier = re_sdk_types::archetypes::VideoStream::descriptor_sample().component;
    let results = store.range_relevant_chunks(
        re_chunk_store::ChunkTrackingMode::PanicOnMissing,
        &re_chunk_store::RangeQuery::everything(*cutoff_timeline.name()),
        entity_path,
        sample_identifier,
    );

    let mut keyframes = Vec::new();
    for chunk in &results.chunks {
        let it = itertools::izip!(
            chunk.iter_indices(cutoff_timeline.name()),
            chunk.iter_component::<re_sdk_types::components::VideoSample>(sample_identifier)
        );

        for ((time, _row_id), sample) in it {
            let Some(sample) = sample.as_slice().first() else {
                continue;
            };

            let sample = sample.0.inner().as_slice();
            match re_video::detect_gop_start(sample, codec.into()) {
                Ok(re_video::GopStartDetection::StartOfGop(_)) => {
                    re_log::debug!(
                        entity = %entity_path,
                        time = %time_to_human_string(cutoff_timeline, time),
                        "detected video keyframe",
                    );

                    keyframes.push(time);
                }

                Ok(re_video::GopStartDetection::NotStartOfGop) => {}

                Err(err) => {
                    re_log::warn!(entity = %entity_path, chunk = %chunk.id(), %err, "keyframe detection failed");
                }
            }
        }
    }

    keyframes.sort(); // we'll be binary searching later on
    keyframes.dedup(); // just to be safe

    keyframes
}

fn extract_chunks_for_single_split(
    store: &ChunkStore,
    entity: &EntityPath,
    components: &HashSet<ComponentIdentifier>,
    timeline: Timeline,
    start_inclusive: TimeInt,
    end_exclusive: TimeInt,
) -> impl Iterator<Item = (ChunkId, Chunk)> {
    re_log::debug_assert!(
        start_inclusive < end_exclusive,
        "start_inclusive={}, end_exclusive={}",
        time_to_human_string(timeline, start_inclusive),
        time_to_human_string(timeline, end_exclusive),
    );

    let query_bootstrap = re_chunk_store::LatestAtQuery::new(*timeline.name(), start_inclusive);
    let query = re_chunk_store::RangeQuery::new(
        *timeline.name(),
        re_log_types::AbsoluteTimeRange::new(start_inclusive, end_exclusive.saturating_sub(1)),
    );

    let chunks_bootstrap = components.iter().flat_map(move |component| {
        let chunks = store
            .latest_at_relevant_chunks(
                ChunkTrackingMode::PanicOnMissing,
                &query_bootstrap,
                entity,
                *component,
            )
            .chunks
            .into_iter()
            .map(|chunk| chunk.latest_at(&query_bootstrap, *component))
            .filter(|chunk| !chunk.is_empty());

        // TODO: explain this -- this is due to overlap heuristics
        let Some(chunk) = chunks.max_by_key(|chunk| {
            chunk
                .iter_indices(timeline.name())
                .next()
                .expect("non-empty latest-at chunk must have a single row")
        }) else {
            return vec![];
        };

        re_log::debug_assert!(chunk.num_rows() == 1);

        let (time, _row_id) = chunk
            .iter_indices(timeline.name())
            .next()
            .expect("non-empty latest-at chunk must have a single row");

        if start_inclusive <= time && time < end_exclusive {
            // If this chunk overlaps with the range results that will follow, then we will create
            // duplicate data with different chunk and row IDs.
            // This is wasteful and useless in general, but the video decoder in particular really
            // hates it, so make sure to filter it out.
            return vec![];
        }

        vec![(
            chunk.id(),
            // This chunk might be re-used in other places in this split, and because we're slicing it
            // (and we really, really need to slice it), we must make sure that it doesn't share
            // a chunk ID nor a row ID with anything else.
            chunk
                // `Chunk::latest_at` internally performs shallow-slicing, so make sure to actually deeply re-slice.
                .row_sliced_deep(0, 1)
                .clone_as(ChunkId::new(), RowId::new()),
        )]
    });

    let chunks = components.iter().flat_map(move |component| {
        let results = store.range_relevant_chunks(
            ChunkTrackingMode::PanicOnMissing,
            &query,
            entity,
            *component,
        );

        results.chunks.into_iter().filter_map(move |chunk| {
            let chunk = chunk.sorted_by_timeline_if_unsorted(timeline.name()); // binsearch incoming
            let time_col = chunk.timelines().get(timeline.name())?;
            let times = time_col.times_raw();
            assert!(times.is_sorted());

            // NOTE: Do not perform range queries here!
            //
            // Actual range queries might lead to different chunk spans for the different
            // components, which will make deduplication across the different components either
            // hard or impossible.
            // It is also just suboptimal in this instance: we want to keep all components no
            // matter what! Just sort and slice.
            //
            // Finally, keep in mind that this only works because we enforce a single timeline
            // when using this tool.

            let start_idx = times.partition_point(|t| *t < start_inclusive.as_i64());
            let end_idx = times
                .partition_point(|t| *t < end_exclusive.as_i64())
                .saturating_sub(1);
            // `end_idx` points at the last in-range value, so the length would be off by -1.
            let slice_len = (end_idx.saturating_add(1)).saturating_sub(start_idx);

            if slice_len == 0 {
                return None;
            }

            re_log::debug_assert!(
                start_inclusive.as_i64() <= times[start_idx]
                    && times[start_idx] < end_exclusive.as_i64(),
                "{} < {} < {}",
                time_to_human_string(timeline, start_inclusive),
                time_to_human_string(timeline, TimeInt::new_temporal(times[start_idx])),
                time_to_human_string(timeline, end_exclusive),
            );
            re_log::debug_assert!(
                start_inclusive.as_i64() <= times[end_idx]
                    && times[end_idx] < end_exclusive.as_i64(),
                "{} < {} < {}",
                time_to_human_string(timeline, start_inclusive),
                time_to_human_string(timeline, TimeInt::new_temporal(times[end_idx])),
                time_to_human_string(timeline, end_exclusive),
            );

            // TODO(RR-3810): If we were to implement this with a virtual store instead, this would b
            // our indicator that this specific data doesn't need to be loaded at all (i.e. it doesnt
            // extend across any 2 splits, nor does it take part in any keyframe/CoordinateFrame shenanigans).
            if slice_len == chunk.num_rows() {
                // If we're re-using the original chunk as-is, then make sure to not update its ID.
                return Some((chunk.id(), chunk));
            }

            Some((
                chunk.id(),
                chunk
                    // Reminder: always perform deep copies if the intent is to write back to disk.
                    .row_sliced_deep(start_idx, slice_len)
                    // We must generate a new chunk ID due to the persistent slicing.
                    // The row IDs are safe from duplicates, since we slice the same way for all components.
                    // The special cases have non-overlapping time spans, and thus are safe too.
                    //
                    // This might lead to duplicated data if all the splits are loaded into the same viewer,
                    // but that's certainly better than missing data.
                    // TODO(cmc): shared recording IDs have been forbidden for now because they caused too many
                    // problems with the video decoder, so that last statement doesn't apply anymore, for now.
                    .with_id(ChunkId::new()),
            ))
        })
    });

    chunks_bootstrap.chain(chunks)
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

    s.replace(['.', ' '], "_") // just in case
}
