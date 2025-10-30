use nohash_hasher::IntSet;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};
use re_types::TransformFrameIdHash;
use std::collections::BTreeMap;
use vec1::smallvec_v1::SmallVec1;

/// Datastructures for tracking which transform relationships are specified by an any given moment in time.
/// Transform relationships are keyed by their source frame, thus the only thing we ever track is the [`TransformFrameIdHash`] of sources.
///
/// Since, except for static time, we don't allow sources to be mentioned by several different entities over time, we do not have to
/// split ranges when other entities are updated, which greatly simplifies tracking.
///
/// The list of frame id hashes can never be empty.
/// If a clear or empty array is logged, we insert the implicit frame again since this is what we always fall back to.
#[derive(Clone)]
pub struct AffectedSources {
    /// Tracks start times of the ranges over which a set of source ranges is affected.
    ///
    /// This list can never be empty.
    /// If a clear or empty array is logged, we insert the implicit frame again since this is what we always fall back to.
    pub range_starts: BTreeMap<TimeInt, SmallVec1<[TransformFrameIdHash; 1]>>,

    /// All sources that this entity ever affects.
    ///
    /// This list can never be empty.
    /// Always contains the implicit source frame.
    pub all_sources: IntSet<TransformFrameIdHash>,
}

impl AffectedSources {
    /// Creates a new instance of [`AffectedSources`] and inserts the implicit frame derived from the entity path at static time.
    pub fn new(entity_path: &EntityPath) -> Self {
        let fallback_source_frame = TransformFrameIdHash::from_entity_path(entity_path);

        // If we see this entity the first time, inject the default source-frame at static time since this is what we use when there's no source specified.
        Self {
            range_starts: std::iter::once((
                TimeInt::STATIC,
                SmallVec1::from_array_const([fallback_source_frame]),
            ))
            .collect(),
            all_sources: std::iter::once(fallback_source_frame).collect(),
        }
    }

    /// Insert a new range-start for a set of sources.
    ///
    /// Returns previous ranges that were set at exactly this time if any.
    pub fn insert_range(
        &mut self,
        start_time: TimeInt,
        sources: SmallVec1<[TransformFrameIdHash; 1]>,
    ) -> Option<SmallVec1<[TransformFrameIdHash; 1]>> {
        self.all_sources.extend(sources.iter().copied());
        self.range_starts.insert(start_time, sources)
    }

    /// Within a subrange, iterates over all ranges it touches. With each range, it specifies which sources are affected therein.
    pub fn iter_ranges(
        &self,
        sub_range: AbsoluteTimeRange,
    ) -> impl Iterator<
        Item = (
            std::ops::Range<TimeInt>,
            &SmallVec1<[TransformFrameIdHash; 1]>,
        ),
    > {
        let Some((first_time, _)) = self.range_starts.range(..=sub_range.min).next_back() else {
            debug_assert!(
                false,
                "We always insert an element at static time, so a latest-at style query should always yield something"
            );
            return itertools::Either::Left(std::iter::empty());
        };

        let mut relevant_range_start_iterator = self
            .range_starts
            .range(*first_time..=sub_range.max)
            .peekable();

        itertools::Either::Right(std::iter::from_fn(move || {
            let (start_time, sources) = relevant_range_start_iterator.next()?;
            let range = if let Some((end_time, _)) = relevant_range_start_iterator.peek() {
                *start_time..**end_time
            } else {
                *start_time..TimeInt::MAX
            };

            Some((range, sources))
        }))
    }

    // TODO: above deserves its own unit test.
}
