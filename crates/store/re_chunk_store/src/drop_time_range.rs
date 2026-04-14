use re_chunk::TimelineName;
use re_log::debug_assert;
use re_log_types::AbsoluteTimeRange;

use crate::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};

impl ChunkStore {
    /// Drop all events that are in the given range on the given timeline.
    ///
    /// Note that matching events will be dropped from all timelines they appear on.
    ///
    /// Chunks are [*shallowly* removed]: they can be recovered if they were originally fetched
    /// from a known RRD manifest.
    /// Static chunks are unaffected.
    ///
    /// [*shallowly* removed]: [`Self::remove_chunks_shallow`]
    pub fn drop_time_range_shallow(
        &mut self,
        timeline: &TimelineName,
        drop_range: AbsoluteTimeRange,
    ) -> Vec<ChunkStoreEvent> {
        let deep_removal = false;
        self.drop_time_range(timeline, drop_range, deep_removal)
    }

    /// Drop all events that are in the given range on the given timeline.
    ///
    /// Note that matching events will be dropped from all timelines they appear on.
    ///
    /// Chunks are [*deeply* removed]: they won't be recoverable.
    /// Static chunks are unaffected.
    ///
    /// Used to implement undo (erase the last event from the blueprint db).
    ///
    /// [*deeply* removed]: [`Self::remove_chunks_deep`]
    pub fn drop_time_range_deep(
        &mut self,
        timeline: &TimelineName,
        drop_range: AbsoluteTimeRange,
    ) -> Vec<ChunkStoreEvent> {
        let deep_removal = true;
        self.drop_time_range(timeline, drop_range, deep_removal)
    }

    fn drop_time_range(
        &mut self,
        timeline: &TimelineName,
        drop_range: AbsoluteTimeRange,
        deep_removal: bool,
    ) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        if drop_range.max() < drop_range.min() {
            return Default::default();
        }

        // Prepare the changes:

        let mut chunks_to_drop = vec![];
        let mut new_chunks = vec![];

        for chunk in self.physical_chunks_per_chunk_id.values() {
            let Some(time_column) = chunk.timelines().get(timeline) else {
                // static chunk, or chunk that doesn't overlap this timeline
                continue; // keep it
            };

            let chunk_range = time_column.time_range();

            if drop_range.contains_range(chunk_range) {
                // The whole chunk should be dropped!
                chunks_to_drop.push(chunk.clone());
            } else if drop_range.intersects(chunk_range) {
                let sorted = chunk.sorted_by_timeline_if_unsorted(timeline);

                let num_rows = sorted.num_rows();

                // Get the sorted times:
                #[expect(clippy::unwrap_used)] // We already know the chunk has the timeline
                let time_column = sorted.timelines().get(timeline).unwrap();
                let times = time_column.times_raw();

                let drop_range_min = drop_range.min().as_i64();
                let drop_range_max = drop_range.max().as_i64();

                let min_idx = times.partition_point(|&time| time < drop_range_min);
                let max_idx = times.partition_point(|&time| time <= drop_range_max);

                {
                    // Sanity check:
                    debug_assert!(min_idx <= max_idx);
                    debug_assert!(drop_range_min <= times[min_idx]);
                    if 0 < min_idx {
                        debug_assert!(times[min_idx - 1] < drop_range_min);
                    }
                    if max_idx < num_rows {
                        debug_assert!(drop_range_max < times[max_idx]);
                        if 0 < max_idx {
                            debug_assert!(times[max_idx - 1] <= drop_range_max);
                        }
                    }
                }

                if min_idx < max_idx {
                    // Drop the original chunk (not the sorted copy) so the store can find it by ID.
                    chunks_to_drop.push(chunk.clone());
                    if 0 < min_idx {
                        new_chunks.push(sorted.row_sliced_shallow(0, min_idx));
                    }
                    if max_idx < num_rows {
                        new_chunks.push(sorted.row_sliced_shallow(max_idx, num_rows - max_idx));
                    }
                }
            }
        }

        // ------------------
        // Apply the changes:

        let mut deletion_diffs: Vec<ChunkStoreDiff> = vec![];

        for chunk in chunks_to_drop {
            let dels = if deep_removal {
                self.remove_chunks_deep(vec![chunk], None)
            } else {
                self.remove_chunks_shallow(vec![chunk], None)
            };
            deletion_diffs.extend(dels.into_iter().map(ChunkStoreDiff::from));
        }

        let mut events = self.finalize_events(deletion_diffs);

        for mut chunk in new_chunks {
            chunk.sort_if_unsorted();
            #[expect(clippy::unwrap_used)] // The chunk came from the store, so it should be fine
            events.append(&mut self.insert_chunk(&chunk.into()).unwrap());
        }

        events
    }
}
