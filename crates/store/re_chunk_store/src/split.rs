use ahash::{HashMap, HashSet};
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, RowId, Span, TimeInt, TimelineName};
use re_log_types::{AbsoluteTimeRange, EntityPath};
use re_sdk_types::archetypes::{Pinhole, Transform3D, VideoStream};
use re_sdk_types::components::VideoCodec;
use re_types_core::{Archetype as _, ComponentIdentifier};

use crate::{ChunkStore, ChunkTrackingMode, IsStartOfGop, LatestAtQuery, RangeQuery};

impl ChunkStore {
    /// Extracts, for each of the given `ranges` on `timeline`, the chunks needed to view that range on its own.
    ///
    /// On top of the data inside the range, each split gets what our datamodel needs to show
    /// it correctly:
    /// * the latest-at value of every component at the start of the range,
    /// * for `VideoStream`s, all samples since the last keyframe before the start of the range,
    /// * for `Transform3D`s and `Pinhole`s with multiplexed coordinate frames, all of their data before the range.
    ///
    /// Keyframes are only detected if `is_start_of_gop` is provided, which can use `re_video::is_start_of_gop`.
    ///
    /// Returns one list per range, in the same order. Each list contains `(original chunk ID, chunk)`
    /// pairs, ordered by original chunk ID so that the loading order of the original recording is preserved.
    pub fn split_by_time_ranges(
        &self,
        tracking: ChunkTrackingMode,
        timeline: &TimelineName,
        ranges: &[AbsoluteTimeRange],
        is_start_of_gop: Option<&IsStartOfGop>,
    ) -> Vec<Vec<(ChunkId, Chunk)>> {
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

        let no_bootstrap = HashSet::default();

        let all_entities_and_their_components = self
            .all_entities()
            .into_iter()
            .filter_map(|entity| {
                self.schema()
                    .all_components_for_entity(&entity)
                    .map(|components| (entity, components))
            })
            .collect_vec();

        let mut keyframes_per_entity: IntMap<EntityPath, Vec<TimeInt>> = IntMap::default();
        // Only the keyframes before the start of a range are of any use.
        let latest_start = ranges.iter().map(|range| range.min()).max();
        if let (Some(is_start_of_gop), Some(latest_start)) = (is_start_of_gop, latest_start) {
            for (entity, _) in &all_entities_and_their_components {
                let keyframes = self.extract_keyframes(
                    tracking,
                    entity,
                    timeline,
                    latest_start,
                    is_start_of_gop.as_ref(),
                );
                if !keyframes.is_empty() {
                    keyframes_per_entity.insert(entity.clone(), keyframes);
                }
            }
        }

        ranges
            .iter()
            .map(|range| {
                let start_inclusive = range.min();
                let end_exclusive = range.max().inc();

                // Static data applies to all times, so it is kept as-is in every split.
                let mut all_chunks_in_split = self
                    .iter_physical_chunks()
                    .filter(|chunk| chunk.is_static())
                    .map(|chunk| (chunk.id(), Chunk::clone(chunk)))
                    .collect_vec();

                for (entity, components) in &all_entities_and_their_components {
                    // Transforms and/or pinholes with multiplexed coordinate frames, see the special case below.
                    let entity_has_multiplexed_transforms_on_timeline =
                        self.entity_has_component_on_timeline(
                            Some(timeline),
                            entity,
                            transform_parent_frame_identifier,
                        ) || self.entity_has_component_on_timeline(
                            Some(timeline),
                            entity,
                            transform_child_frame_identifier,
                        );
                    let entity_has_multiplexed_pinholes_on_timeline =
                        self.entity_has_component_on_timeline(
                            Some(timeline),
                            entity,
                            pinhole_parent_frame_identifier,
                        ) || self.entity_has_component_on_timeline(
                            Some(timeline),
                            entity,
                            pinhole_child_frame_identifier,
                        );
                    let entity_has_multiplexed_frames = entity_has_multiplexed_transforms_on_timeline
                        || entity_has_multiplexed_pinholes_on_timeline;

                    // Base case: everything
                    {
                        // We must not try to bootstrap special components, as they will undergo some
                        // custom bootstrap logic below anyway.
                        // Normally this would be fine, but again we cannot afford the presence of any
                        // duplicate data when it comes to video decoding.
                        // Transforms and pinholes without multiplexed coordinate frames are plain
                        // latest-at data though.
                        let components_to_bootstrap = components
                            .iter()
                            .copied()
                            .filter(|component| {
                                !special_components.contains(component)
                                    || (!entity_has_multiplexed_frames
                                        && frame_components.contains(component))
                            })
                            .collect();
                        let components = components.iter().copied().collect();
                        let keep_other_components = true;
                        let chunks = self.extract_chunks_for_single_split(
                            tracking,
                            entity,
                            &components,
                            &components_to_bootstrap,
                            timeline,
                            start_inclusive,
                            end_exclusive,
                            keep_other_components,
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
                            let keep_other_components = false;
                            let chunks = self.extract_chunks_for_single_split(
                                tracking,
                                entity,
                                &components,
                                &no_bootstrap,
                                timeline,
                                start_revised,
                                start_inclusive,
                                keep_other_components,
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
                            "gathering all transforms/pinholes up to start time due to multiplexed coordinate frames…"
                        );

                        // Data logged next to the transforms is already covered by the base case.
                        let keep_other_components = false;
                        let chunks = self.extract_chunks_for_single_split(
                            tracking,
                            entity,
                            &frame_components,
                            &no_bootstrap,
                            timeline,
                            TimeInt::MIN,
                            start_inclusive,
                            keep_other_components,
                        );
                        all_chunks_in_split.extend(chunks);
                    }
                }

                // We must make sure that the new recordings have their chunks in the same order as the original
                // one (as dictated by their chunk IDs), otherwise the loading experience would be absolutely awful,
                // given that we've just queried the data per-entity per-component.
                all_chunks_in_split.sort_by_key(|(original_chunk_id, _)| *original_chunk_id);

                all_chunks_in_split
            })
            .collect()
    }

    // TODO(RR-3810): For a virtual store implementation, we'd want this to load no more than 1 chunk at a time.
    fn extract_keyframes(
        &self,
        tracking: ChunkTrackingMode,
        entity_path: &EntityPath,
        timeline: &TimelineName,
        until: TimeInt,
        is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    ) -> Vec<TimeInt> {
        let codec = {
            let codec_identifier = VideoStream::descriptor_codec().component;
            let results = self.latest_at_relevant_chunks(
                tracking,
                &LatestAtQuery::new(*timeline, TimeInt::MAX),
                entity_path,
                codec_identifier,
            );

            results
                .chunks
                .iter()
                .flat_map(|chunk| chunk.iter_component::<VideoCodec>(codec_identifier))
                .find_map(|data| data.as_slice().first().copied())
        };

        let Some(codec) = codec else {
            return vec![];
        };

        let sample_identifier = VideoStream::descriptor_sample().component;
        let results = self.range_relevant_chunks(
            tracking,
            &RangeQuery::new(*timeline, AbsoluteTimeRange::new(TimeInt::MIN, until)),
            entity_path,
            sample_identifier,
        );

        let mut keyframes = Vec::new();
        let mut num_failures = 0;
        let mut last_error = None;
        for chunk in &results.chunks {
            let it = itertools::izip!(
                chunk.iter_indices(timeline),
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
                        num_failures += 1;
                        last_error = Some(err);
                    }
                }
            }
        }

        // Once per entity: this also runs in the viewer, where every warning reaches the user.
        if let Some(err) = last_error {
            re_log::warn!(entity = %entity_path, num_failures, %err, "keyframe detection failed");
        }

        keyframes.sort(); // we'll be binary searching later on
        keyframes.dedup(); // just to be safe

        keyframes
    }

    #[expect(clippy::too_many_arguments)]
    fn extract_chunks_for_single_split<'a>(
        &'a self,
        tracking: ChunkTrackingMode,
        entity: &'a EntityPath,
        components: &'a HashSet<ComponentIdentifier>,
        components_to_bootstrap: &'a HashSet<ComponentIdentifier>,
        timeline: &'a TimelineName,
        start_inclusive: TimeInt,
        end_exclusive: TimeInt,
        keep_other_components: bool,
    ) -> impl Iterator<Item = (ChunkId, Chunk)> + 'a {
        re_log::debug_assert!(
            start_inclusive < end_exclusive,
            "start_inclusive={start_inclusive:?}, end_exclusive={end_exclusive:?}",
        );

        let query_bootstrap = LatestAtQuery::new(*timeline, start_inclusive);
        let query = RangeQuery::new(
            *timeline,
            AbsoluteTimeRange::new(start_inclusive, end_exclusive.saturating_sub(1)),
        );

        // TODO(cmc): There are certainly opportunities to do better on the bootstrap path, especially
        // when it comes to sharing chunks across components, using e.g. an approach somewhat similar
        // to what we do below for ranges.
        let chunks_bootstrap = components_to_bootstrap.iter().flat_map(move |component| {
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
                    .iter_indices(timeline)
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
                .iter_indices(timeline)
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
        });

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
                if chunks.contains_key(&chunk.id()) {
                    continue;
                }

                let chunk = chunk.sorted_by_timeline_if_unsorted(timeline); // binsearch incoming
                let Some(time_col) = chunk.timelines().get(timeline) else {
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
                let (original_chunk_id, chunk) = if slice_len == chunk.num_rows() {
                    // If we're re-using the original chunk as-is, then make sure to not update its ID.
                    (chunk.id(), chunk)
                } else {
                    // TODO(cmc): It could be worthwhile to implement and use a `Chunk::components_sliced` here (plural).
                    // (We've already decided to generate a new chunk ID anyhow, so column slicing is fine at this point.)
                    (
                        chunk.id(),
                        chunk
                            // Reminder: always perform deep copies if the intent is to write back to disk.
                            .row_sliced_deep(Span::from_start_len(start_idx, slice_len)),
                    )
                };

                let chunk = if keep_other_components {
                    chunk
                } else {
                    chunk.components_sliced(&components.iter().copied().collect_vec())
                };

                let already_exists = chunks.insert(original_chunk_id, chunk).is_some();
                re_log::debug_assert!(!already_exists);
            }
        }

        std::iter::chain(chunks_bootstrap, chunks)
    }
}
