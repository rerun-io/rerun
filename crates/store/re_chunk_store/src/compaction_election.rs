use std::sync::Arc;

use ahash::HashMap;
use itertools::Itertools as _;

use re_byte_size::SizeBytes;
use re_chunk::Chunk;

use crate::{ChunkId, ChunkStore, ChunkStoreConfig};

impl ChunkStore {
    /// Finds the most appropriate candidate for compaction.
    ///
    /// The algorithm is simple: for each incoming [`Chunk`], we take a look at its future neighbors.
    /// Each neighbor is a potential candidate for compaction.
    ///
    /// Because the chunk is going to be inserted into many different indices -- for each of its timelines
    /// and components -- it will have many direct neighbors.
    /// Everytime we encounter a neighbor, it earns points.
    ///
    /// The neighbor with the most points at the end of the process is elected.
    pub(crate) fn find_and_elect_compaction_candidate(
        &self,
        chunk: &Arc<Chunk>,
    ) -> Option<Arc<Chunk>> {
        re_tracing::profile_function!();

        // Early exit if the newly added Chunk is already the result of a split, directly or indirectly.
        // Compacting chunks coming from a split lineage is generally a mistake, as that is likely
        // to lead to overlaps that weren't there in the first place.
        if self.descends_from_a_split(&chunk.id()) {
            return None;
        }

        {
            // Make sure to early exit if the newly added Chunk is already beyond the compaction thresholds
            // on its own.

            let ChunkStoreConfig {
                enable_changelog: _,
                chunk_max_bytes,
                chunk_max_rows,
                chunk_max_rows_if_unsorted,
            } = self.config;

            let total_bytes = <Chunk as SizeBytes>::total_size_bytes(chunk);
            let is_below_bytes_threshold = total_bytes <= chunk_max_bytes;

            let total_rows = (chunk.num_rows()) as u64;
            let is_below_rows_threshold = if chunk.all_timelines_sorted() {
                total_rows <= chunk_max_rows
            } else {
                total_rows <= chunk_max_rows_if_unsorted
            };

            if !(is_below_bytes_threshold && is_below_rows_threshold) {
                return None;
            }
        }

        let mut candidates_below_threshold: HashMap<ChunkId, u64> = HashMap::default();
        let mut check_if_chunk_below_threshold =
            |store: &Self, candidate_chunk_id: ChunkId| -> u64 {
                let ChunkStoreConfig {
                    enable_changelog: _,
                    chunk_max_bytes,
                    chunk_max_rows,
                    chunk_max_rows_if_unsorted,
                } = store.config;

                *candidates_below_threshold
                    .entry(candidate_chunk_id)
                    .or_insert_with(|| {
                        store
                            .physical_chunks_per_chunk_id
                            .get(&candidate_chunk_id)
                            .map_or(0, |candidate| {
                                if chunk.id() == candidate_chunk_id {
                                    return 0;
                                }

                                if !chunk.concatenable(candidate) {
                                    return 0;
                                }

                                // Refuse the candidate if it descends from a split chunk, directly or indirectly.
                                // Compacting chunks coming from a split lineage is generally a mistake, as that is likely
                                // to lead to overlaps that weren't there in the first place.
                                if self.descends_from_a_split(&candidate_chunk_id) {
                                    return 0;
                                }

                                let total_bytes = <Chunk as SizeBytes>::total_size_bytes(chunk)
                                    + <Chunk as SizeBytes>::total_size_bytes(candidate);
                                let is_below_bytes_threshold = total_bytes <= chunk_max_bytes;

                                let total_rows = (chunk.num_rows() + candidate.num_rows()) as u64;
                                let is_below_rows_threshold = if candidate.all_timelines_sorted() {
                                    total_rows <= chunk_max_rows
                                } else {
                                    total_rows <= chunk_max_rows_if_unsorted
                                };

                                if is_below_bytes_threshold && is_below_rows_threshold {
                                    return candidate.num_rows() as u64;
                                }

                                0
                            })
                    })
            };

        let mut candidates: HashMap<ChunkId, u64> = HashMap::default();

        let temporal_chunk_ids_per_timeline = self
            .temporal_chunk_ids_per_entity_per_component
            .get(chunk.entity_path())?;

        for (timeline, time_range_per_component) in chunk.time_range_per_component() {
            let Some(temporal_chunk_ids_per_component) =
                temporal_chunk_ids_per_timeline.get(&timeline)
            else {
                continue;
            };

            for (component, time_range) in time_range_per_component {
                let Some(temporal_chunk_ids_per_time) =
                    temporal_chunk_ids_per_component.get(&component)
                else {
                    continue;
                };

                {
                    // Direct neighbors (before): 1 point each.
                    if let Some((_data_time, chunk_id_set)) = temporal_chunk_ids_per_time
                        .per_start_time
                        .range(..time_range.min())
                        .next_back()
                    {
                        for &chunk_id in chunk_id_set {
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id);
                        }
                    }

                    // Direct neighbors (after): 1 point each.
                    if let Some((_data_time, chunk_id_set)) = temporal_chunk_ids_per_time
                        .per_start_time
                        .range(time_range.max().inc()..)
                        .next()
                    {
                        for &chunk_id in chunk_id_set {
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id);
                        }
                    }

                    // Shared start times: 2 points each.
                    {
                        let chunk_id_set = temporal_chunk_ids_per_time
                            .per_start_time
                            .get(&time_range.min());
                        for chunk_id in chunk_id_set.iter().flat_map(|set| set.iter().copied()) {
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id) * 2;
                        }
                    }
                }
            }
        }

        let mut candidates = candidates.into_iter().collect_vec();
        {
            re_tracing::profile_scope!("sort_candidates");
            candidates.sort_by_key(|(_chunk_id, points)| *points);
            candidates.reverse();
        }

        candidates
            .into_iter()
            .filter(|(_chunk_id, points)| *points > 0)
            .find_map(|(chunk_id, _points)| {
                self.physical_chunks_per_chunk_id
                    .get(&chunk_id)
                    .map(Arc::clone)
            })
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::{EntityPath, RowId, Timeline};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_sdk_types::components::Blob;
    use re_types_core::ComponentDescriptor;
    use similar_asserts::assert_eq;

    use super::*;

    // TODO(cmc): We could have more test coverage here, especially regarding thresholds etc.
    // For now the development and maintenance cost doesn't seem to be worth it.
    // We can re-assess later if things turns out to be shaky in practice.

    #[test]
    fn compaction_simple() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            Default::default(),
        );

        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();
        let row_id6 = RowId::new();
        let row_id7 = RowId::new();
        let row_id8 = RowId::new();
        let row_id9 = RowId::new();
        let row_id10 = RowId::new();

        let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
        let timepoint2 = [(Timeline::new_sequence("frame"), 3)];
        let timepoint3 = [(Timeline::new_sequence("frame"), 5)];
        let timepoint4 = [(Timeline::new_sequence("frame"), 7)];
        let timepoint5 = [(Timeline::new_sequence("frame"), 9)];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .build()?;
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id6,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id7,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id8,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .build()?;
        let chunk4 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id9,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id10,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);
        let chunk4 = Arc::new(chunk4);

        eprintln!("---\n{store}\ninserting {}", chunk1.id());

        store.insert_chunk(&chunk1)?;

        eprintln!("---\n{store}\ninserting {}", chunk2.id());

        store.insert_chunk(&chunk2)?;

        eprintln!("---\n{store}\ninserting {}", chunk3.id());

        store.insert_chunk(&chunk3)?;

        eprintln!("---\n{store}\ninserting {}", chunk4.id());

        store.insert_chunk(&chunk4)?;

        eprintln!("---\n{store}");

        let got = store
            .physical_chunks_per_chunk_id
            .first_key_value()
            .map(|(_id, chunk)| chunk)
            .unwrap();

        let expected = Chunk::builder_with_id(got.id(), entity_path.clone())
            .with_component_batches(
                row_id1,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .with_component_batches(
                row_id4,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .with_component_batches(
                row_id6,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id7,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id8,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .with_component_batches(
                row_id9,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id10,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .build()?;

        assert_eq!(1, store.physical_chunks_per_chunk_id.len());
        assert_eq!(
            expected,
            **got,
            "{}",
            similar_asserts::SimpleDiff::from_str(
                &format!("{expected}"),
                &format!("{got}"),
                "expected",
                "got",
            ),
        );

        Ok(())
    }

    #[test]
    fn compaction_blobs() -> anyhow::Result<()> {
        #![expect(clippy::cloned_ref_to_slice_refs)]

        re_log::setup_logging();

        // Create a store with a specific byte limit for testing
        // Default chunk_max_bytes is 12 * 8 * 4096 = 393,216 bytes
        let chunk_max_bytes = 300_000u64; // 300KB limit for easier testing
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig {
                chunk_max_bytes,
                ..Default::default()
            },
        );

        let entity_path = EntityPath::from("blob/data");

        // Calculate blob sizes relative to the limit
        let blob_size_1_3rd = (chunk_max_bytes / 3) as usize; // ~100KB
        let blob_size_1_2nd = (chunk_max_bytes / 2) as usize; // ~150KB

        // Create test data
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
        let timepoint2 = [(Timeline::new_sequence("frame"), 2)];
        let timepoint3 = [(Timeline::new_sequence("frame"), 3)];
        let timepoint4 = [(Timeline::new_sequence("frame"), 4)];
        let timepoint5 = [(Timeline::new_sequence("frame"), 5)];

        // Create blobs of different sizes
        let blob1 = Blob::from(vec![1u8; blob_size_1_3rd]); // 1/3 limit
        let blob2 = Blob::from(vec![2u8; blob_size_1_2nd]); // 1/2 limit
        let blob3 = Blob::from(vec![3u8; blob_size_1_2nd]); // 1/2 limit
        let blob4 = Blob::from(vec![4u8; blob_size_1_2nd]); // 1/2 limit
        let blob5 = Blob::from(vec![5u8; blob_size_1_3rd]); // 1/3 limit

        // Create a simple descriptor for blob components
        let blob_descriptor = ComponentDescriptor::partial("blob");

        // Create chunks according to the pattern:
        // 1. Chunk with blob 1/3rd the limit
        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1,
                timepoint1,
                [(
                    blob_descriptor.clone(),
                    &[blob1.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        // 2. Chunk with three blobs 1/2 the limit (will be split across multiple chunks)
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2,
                timepoint2,
                [(
                    blob_descriptor.clone(),
                    &[blob2.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(
                    blob_descriptor.clone(),
                    &[blob3.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .with_component_batches(
                row_id4,
                timepoint4,
                [(
                    blob_descriptor.clone(),
                    &[blob4.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        // 3. Chunk with blob 1/3rd the limit
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id5,
                timepoint5,
                [(
                    blob_descriptor.clone(),
                    &[blob5.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);

        eprintln!(
            "Inserting chunk1 (blob 1/3 limit: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk1),
        );
        store.insert_chunk(&chunk1)?;
        eprintln!(
            "Store has {} chunks",
            store.physical_chunks_per_chunk_id.len()
        );

        eprintln!(
            "Inserting chunk2 (3 blobs 1/2 limit each: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk2),
        );
        store.insert_chunk(&chunk2)?;
        eprintln!(
            "Store has {} chunks",
            store.physical_chunks_per_chunk_id.len()
        );

        eprintln!(
            "Inserting chunk3 (blob 1/3 limit: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk3),
        );
        store.insert_chunk(&chunk3)?;
        eprintln!(
            "Store has {} chunks",
            store.physical_chunks_per_chunk_id.len()
        );

        // Verify the expected compaction results:
        // Expected:
        // - The first chunk was left untouched.
        // - The second chunk was split into 3 smaller chunks.
        // - The third chunk was left untouched.
        // So we expect 5 chunks total.

        eprintln!("Final store state:");
        eprintln!("{store}");

        // Check that we have the expected number of chunks after compaction
        assert_eq!(
            5,
            store.physical_chunks_per_chunk_id.len(),
            "Expected 4 chunks after compaction: [blob1], [blob2], [blob3], [blob4], [blob5]"
        );

        // Verify the chunks contain the expected data by checking their sizes
        let mut chunk_sizes: Vec<_> = store
            .physical_chunks_per_chunk_id
            .values()
            .map(|chunk| <Chunk as SizeBytes>::total_size_bytes(chunk))
            .collect();
        chunk_sizes.sort_unstable();

        eprintln!("Chunk sizes: {chunk_sizes:?}");

        let smallest_expected = <Chunk as SizeBytes>::total_size_bytes(&chunk1);
        let largest_expected = <Chunk as SizeBytes>::total_size_bytes(&chunk2) / 3;

        // Allow some tolerance for metadata overhead
        let tolerance = 10_000u64; // 10KB tolerance

        for &chunk_size in &chunk_sizes[0..2] {
            assert!(
                chunk_size >= smallest_expected.saturating_sub(tolerance)
                    && chunk_size <= smallest_expected + tolerance,
                "Smallest chunk size {chunk_size} should be around {smallest_expected} ± {tolerance}",
            );
        }

        for &chunk_size in &chunk_sizes[2..] {
            assert!(
                chunk_size >= largest_expected.saturating_sub(tolerance)
                    && chunk_size <= largest_expected + tolerance,
                "Largest chunk size {chunk_size} should be around {largest_expected} ± {tolerance}",
            );
        }

        Ok(())
    }
}
