use ahash::{HashMap, HashSet};
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, RowId, TimeInt, TimelineName};
use re_log_types::{AbsoluteTimeRange, EntityPath};
use re_sdk_types::archetypes::{Pinhole, Transform3D, VideoStream};
use re_sdk_types::components::VideoCodec;
use re_types_core::{Archetype as _, ComponentIdentifier};

use crate::rebatch_videos::extract_codec;
use crate::{ChunkStore, ChunkTrackingMode, IsStartOfGop, LatestAtQuery, RangeQuery};

/// Which components of a chunk to keep when extracting a time range from it.
#[derive(Clone, Copy)]
enum KeepComponents {
    /// Every component in the chunk, including the ones that were not queried.
    All,

    /// Only the queried components.
    Queried,
}

impl ChunkStore {
    /// Extracts, for each of the given `ranges` on `timeline`, the chunks needed to view that range on its own.
    ///
    /// On top of the data inside the range, each split gets what our datamodel needs to show
    /// it correctly:
    /// * all static data,
    /// * the latest-at value of every component at the start of the range,
    /// * for `VideoStream`s, all samples since the last keyframe before the start of the range,
    /// * for `Transform3D`s and `Pinhole`s with multiplexed coordinate frames, all of their data before the range.
    ///
    /// Chunks that are not loaded are reported through `tracking` and left out of the splits.
    ///
    /// Keyframes are only detected if `is_start_of_gop` is provided, which can use `re_video::is_start_of_gop`.
    ///
    /// Yields one split per range, in the same order, each as an iterator over its chunks.
    ///
    /// Splits are computed one at a time, as the outer iterator advances.
    pub fn split_by_time_ranges<'a>(
        &'a self,
        tracking: ChunkTrackingMode,
        timeline: TimelineName,
        ranges: &'a [AbsoluteTimeRange],
        is_start_of_gop: Option<&IsStartOfGop>,
    ) -> impl Iterator<Item = impl Iterator<Item = Chunk> + use<>> + use<'a> {
        re_tracing::profile_function!();

        // `VideoStream`s must be split on a keyframe, always.
        //
        // The solution is to find the closest past keyframe, and then duplicate the entire stream
        // from there up to the cutoff point. See `extract_keyframes`.
        //
        // The keyframe markers, if any, go along with the samples.
        let video_components = [
            VideoStream::descriptor_sample().component,
            VideoStream::descriptor_is_keyframe().component,
        ];

        // `Transform3D`s can be multiplexed on a single entity stream using `CoordinateFrame`s,
        // making them completely opaque to the query engine.
        //
        // The solution is to always duplicate the entire stream up to the cutoff point whenever that happens.
        let transform_parent_frame_identifier = Transform3D::descriptor_parent_frame().component;
        let transform_child_frame_identifier = Transform3D::descriptor_child_frame().component;

        // `Pinhole`s can be multiplexed on a single entity stream using `CoordinateFrame`s,
        // making them completely opaque to the query engine.
        //
        // The solution is to always duplicate the entire stream up to the cutoff point whenever that happens.
        let pinhole_parent_frame_identifier = Pinhole::descriptor_parent_frame().component;
        let pinhole_child_frame_identifier = Pinhole::descriptor_child_frame().component;

        let frame_components: HashSet<_> = itertools::chain!(
            Transform3D::all_component_identifiers(),
            Pinhole::all_component_identifiers(),
        )
        .collect();

        let special_components: HashSet<_> =
            itertools::chain!(video_components, frame_components.iter().copied()).collect();

        let has_multiplexed_frames = |entity: &EntityPath| {
            [
                transform_parent_frame_identifier,
                transform_child_frame_identifier,
                pinhole_parent_frame_identifier,
                pinhole_child_frame_identifier,
            ]
            .into_iter()
            .any(|component| {
                self.entity_has_component_on_timeline(Some(&timeline), entity, component)
            })
        };

        let all_entities_and_their_components = self
            .all_entities()
            .into_iter()
            .filter_map(|entity| {
                let components = self.schema().all_components_for_entity(&entity)?;
                let entity_has_multiplexed_frames = has_multiplexed_frames(&entity);
                Some((entity, components, entity_has_multiplexed_frames))
            })
            .collect_vec();

        let mut keyframes_per_entity: IntMap<EntityPath, Vec<TimeInt>> = IntMap::default();
        if let Some(is_start_of_gop) = is_start_of_gop {
            for (entity, _, _) in &all_entities_and_their_components {
                let keyframes =
                    self.extract_keyframes(tracking, entity, timeline, is_start_of_gop.as_ref());
                if !keyframes.is_empty() {
                    keyframes_per_entity.insert(entity.clone(), keyframes);
                }
            }
        }

        // Static data applies to all times, so it is kept as-is in every split.
        // The latest-at bootstrap below skips it.
        // Missing static chunks are still reported, by the range queries in the base case.
        let static_chunks = self
            .iter_physical_chunks()
            .filter(|chunk| chunk.is_static())
            .map(|chunk| (chunk.id(), Chunk::clone(chunk)))
            .collect_vec();

        ranges.iter().map(move |range| {
            re_tracing::profile_scope!("split");

            let start_inclusive = range.min();
            let end_exclusive = range.max().inc();

            let mut all_chunks_in_split = static_chunks.clone();

            for (entity, components, entity_has_multiplexed_frames) in
                &all_entities_and_their_components
            {
                let entity_has_multiplexed_frames = *entity_has_multiplexed_frames;

                // Base case: everything
                {
                    // Video samples and multiplexed transforms/pinholes get their data before the
                    // range from the special cases below, so bootstrapping them here would
                    // duplicate it, which the video decoder cannot handle.
                    // Transforms and pinholes without multiplexed coordinate frames are plain
                    // latest-at data.
                    //
                    // All components go through a single range pass, so that a chunk holding both
                    // special and regular components (common after compaction) is only extracted once.
                    let components_to_bootstrap = components
                        .iter()
                        .copied()
                        .filter(|component| {
                            !special_components.contains(component)
                                || (!entity_has_multiplexed_frames
                                    && frame_components.contains(component))
                        })
                        .collect();
                    all_chunks_in_split.extend(self.extract_bootstrap_chunks(
                        tracking,
                        entity,
                        &components_to_bootstrap,
                        timeline,
                        start_inclusive,
                        end_exclusive,
                    ));

                    let components = components.iter().copied().collect();
                    let chunks = self.extract_chunks_for_single_split(
                        tracking,
                        entity,
                        &components,
                        timeline,
                        start_inclusive,
                        end_exclusive,
                        KeepComponents::All,
                    );
                    all_chunks_in_split.extend(chunks);
                }

                // Special case: video keyframes
                {
                    let start_revised = keyframes_per_entity.get(entity).and_then(|keyframes| {
                        let p = keyframes
                            .partition_point(|t| *t <= start_inclusive)
                            .saturating_sub(1);
                        let start_revised = keyframes[p];

                        if start_revised < start_inclusive {
                            re_log::debug!(
                                %entity,
                                %timeline,
                                ?start_inclusive,
                                ?start_revised,
                                "revising start time to match video keyframe…"
                            );
                            Some(start_revised)
                        } else {
                            None
                        }
                    });

                    if let Some(start_revised) = start_revised {
                        let components = video_components.into_iter().collect();
                        // Data logged next to the video samples is already covered by the base case.
                        let chunks = self.extract_chunks_for_single_split(
                            tracking,
                            entity,
                            &components,
                            timeline,
                            start_revised,
                            start_inclusive,
                            KeepComponents::Queried,
                        );
                        all_chunks_in_split.extend(chunks);
                    }
                }

                // Special cases: transforms and/or pinholes with multiplexed coordinate frames
                if entity_has_multiplexed_frames {
                    re_log::debug!(
                        %entity,
                        %timeline,
                        ?start_inclusive,
                        "gathering all transforms/pinholes up to cutoff point due to multiplexed coordinate frames…"
                    );

                    // Data logged next to the transforms is already covered by the base case.
                    let chunks = self.extract_chunks_for_single_split(
                        tracking,
                        entity,
                        &frame_components,
                        timeline,
                        TimeInt::MIN,
                        start_inclusive,
                        KeepComponents::Queried,
                    );
                    all_chunks_in_split.extend(chunks);
                }
            }

            // We must make sure that the new recordings have their chunks in the same order as the original
            // one (as dictated by their chunk IDs), otherwise the loading experience would be absolutely awful,
            // given that we've just queried the data per-entity per-component.
            all_chunks_in_split.sort_by_key(|(original_chunk_id, _)| *original_chunk_id);

            all_chunks_in_split
                .into_iter()
                .map(|(_original_chunk_id, chunk)| chunk)
        })
    }

    // TODO(RR-3810): For a virtual store implementation, we'd want this to load no more than 1 chunk at a time.
    fn extract_keyframes(
        &self,
        tracking: ChunkTrackingMode,
        entity_path: &EntityPath,
        timeline: TimelineName,
        is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    ) -> Vec<TimeInt> {
        let Some(codec) = extract_codec(self, tracking, entity_path, timeline) else {
            return vec![];
        };

        let sample_identifier = VideoStream::descriptor_sample().component;
        let results = self.range_relevant_chunks(
            tracking,
            &RangeQuery::everything(timeline),
            entity_path,
            sample_identifier,
        );

        let mut keyframes = Vec::new();
        for chunk in &results.chunks {
            let it = itertools::izip!(
                chunk.iter_indices(&timeline),
                chunk.iter_component::<re_sdk_types::components::VideoSample>(sample_identifier)
            );

            for ((time, _row_id), sample) in it {
                let Some(sample) = sample.as_slice().first() else {
                    continue;
                };

                let sample = sample.0.inner().as_slice();
                match is_start_of_gop(sample, codec) {
                    Ok(true) => {
                        re_log::debug!(
                            entity = %entity_path,
                            ?time,
                            "detected video keyframe",
                        );

                        keyframes.push(time);
                    }

                    Ok(false) => {}

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

    /// The latest-at value of each of `components` at `start_inclusive`, unless it already lies
    /// within `start_inclusive..end_exclusive` or is static.
    fn extract_bootstrap_chunks<'a>(
        &'a self,
        tracking: ChunkTrackingMode,
        entity: &'a EntityPath,
        components: &'a HashSet<ComponentIdentifier>,
        timeline: TimelineName,
        start_inclusive: TimeInt,
        end_exclusive: TimeInt,
    ) -> impl Iterator<Item = (ChunkId, Chunk)> + 'a {
        let query_bootstrap = LatestAtQuery::new(timeline, start_inclusive);

        // TODO(cmc): There are certainly opportunities to do better on the bootstrap path, especially
        // when it comes to sharing chunks across components, using e.g. an approach somewhat similar
        // to what we do for ranges in `extract_chunks_for_single_split`.
        components.iter().flat_map(move |component| {
            let chunks = self
                .latest_at_relevant_chunks(tracking, &query_bootstrap, entity, *component)
                .chunks
                .into_iter()
                .filter_map(|chunk| chunk.latest_at(&query_bootstrap, *component));

            // Due to the overlap heuristics, the bootstrap query might return an arbitrary amount of
            // chunks: we need to find the most relevant in those, which in this case is whichever has
            // the highest (time, row_id) index.
            let Some(chunk) = chunks.max_by_key(|chunk| {
                chunk
                    .iter_indices(&timeline)
                    .next()
                    .expect("non-empty latest-at chunk must have a single row")
            }) else {
                return vec![];
            };

            if chunk.is_static() {
                // Static data is kept as-is, see `split_by_time_ranges`.
                return vec![];
            }

            re_log::debug_assert!(chunk.num_rows() == 1);

            let (time, _row_id) = chunk
                .iter_indices(&timeline)
                .next()
                .expect("non-empty latest-at chunk must have a single row");

            if start_inclusive <= time && time < end_exclusive {
                // This row is part of the range that `extract_chunks_for_single_split` extracts,
                // so keeping it would create duplicate data with different chunk and row IDs.
                // This is wasteful and useless in general, but the video decoder in particular really
                // hates it, so make sure to filter it out.
                return vec![];
            }

            vec![(
                chunk.id(),
                chunk
                    // We're bootstrapping at the component batch level, so might as well discard everything else.
                    .component_sliced(*component)
                    // `Chunk::latest_at` internally performs shallow-slicing, so make sure to actually deeply re-slice.
                    .row_sliced_unit_deep(0)
                    // This chunk might be re-used in other places in this split, and because we're slicing it
                    // (and we really, really need to slice it), we must make sure that it doesn't share
                    // a chunk ID nor a row ID with anything else.
                    .clone_as(ChunkId::new(), RowId::new()),
            )]
        })
    }

    #[expect(clippy::too_many_arguments)]
    fn extract_chunks_for_single_split(
        &self,
        tracking: ChunkTrackingMode,
        entity: &EntityPath,
        components: &HashSet<ComponentIdentifier>,
        timeline: TimelineName,
        start_inclusive: TimeInt,
        end_exclusive: TimeInt,
        keep_components: KeepComponents,
    ) -> impl Iterator<Item = (ChunkId, Chunk)> + use<> {
        re_log::debug_assert!(
            start_inclusive < end_exclusive,
            "start_inclusive={start_inclusive:?}, end_exclusive={end_exclusive:?}",
        );

        let query = RangeQuery::new(
            timeline,
            AbsoluteTimeRange::new(start_inclusive, end_exclusive.saturating_sub(1)),
        );
        let components_to_keep = components.iter().copied().collect_vec();

        let mut chunks: HashMap<ChunkId, Chunk> = Default::default();
        for component in components {
            let results = self.range_relevant_chunks(tracking, &query, entity, *component);

            for chunk in results.chunks {
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
                // It is much, much harder to deduplicate appropriately across multiple timelines at once.
                //
                // Sorting gives unsorted chunks a new ID, so deduplicate on the original one.
                let original_chunk_id = chunk.id();
                if chunks.contains_key(&original_chunk_id) {
                    continue;
                }

                let chunk = chunk.sorted_by_timeline_if_unsorted(&timeline); // binsearch incoming
                let Some(time_col) = chunk.timelines().get(&timeline) else {
                    continue;
                };
                let times = time_col.times_raw();
                assert!(times.is_sorted());

                let start_idx = times.partition_point(|t| *t < start_inclusive.as_i64());
                let end_idx_excl = times.partition_point(|t| *t < end_exclusive.as_i64());
                let slice_len = end_idx_excl.saturating_sub(start_idx);

                if slice_len == 0 {
                    continue;
                }

                let end_idx = end_idx_excl.saturating_sub(1);

                re_log::debug_assert!(
                    start_inclusive.as_i64() <= times[start_idx]
                        && times[start_idx] < end_exclusive.as_i64(),
                    "{start_inclusive:?} <= {} < {end_exclusive:?}",
                    times[start_idx],
                );
                re_log::debug_assert!(
                    start_inclusive.as_i64() <= times[end_idx]
                        && times[end_idx] < end_exclusive.as_i64(),
                    "{start_inclusive:?} <= {} < {end_exclusive:?}",
                    times[end_idx],
                );

                // TODO(RR-3810): If we were to implement this with a virtual store instead, this would be
                // our indicator that this specific data doesn't need to be loaded at all (i.e. it doesnt
                // extend across any 2 splits, nor does it take part in any keyframe/CoordinateFrame shenanigans).
                let chunk = if slice_len == chunk.num_rows() {
                    chunk
                } else {
                    // Reminder: always perform deep copies if the intent is to write back to disk.
                    chunk.row_sliced_deep(re_chunk::Span::from_start_len(start_idx, slice_len))
                };

                let mut chunk = match keep_components {
                    KeepComponents::All => chunk,
                    KeepComponents::Queried => chunk.components_sliced(&components_to_keep),
                };

                // Unsorted chunks were sorted by time for slicing, which can leave their rows out of
                // `RowId` order: put them back, a recording with such a chunk doesn't load.
                chunk.sort_by_row_ids_if_needed();

                let already_exists = chunks.insert(original_chunk_id, chunk).is_some();
                re_log::debug_assert!(!already_exists);
            }
        }

        chunks.into_iter()
    }
}
