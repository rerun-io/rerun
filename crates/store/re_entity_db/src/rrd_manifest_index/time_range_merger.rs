//! The goal of this utility is to create non-overlapping ranges without gaps that are loaded/unloaded
//! from the perspective of a latest-at query from a list of chunk time ranges. While also prioritizing
//! unloaded chunks.

use std::{
    collections::BinaryHeap,
    ops::{Deref, DerefMut},
};

use re_log_types::AbsoluteTimeRange;

#[derive(Clone, Copy)]
pub struct TimeRange {
    pub range: AbsoluteTimeRange,
    pub loaded: bool,
}

impl Deref for TimeRange {
    type Target = AbsoluteTimeRange;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.range
    }
}

impl DerefMut for TimeRange {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.range
    }
}

/// Wrapper struct for custom ordering in binary heap.
struct IncomingRange(TimeRange);

impl Deref for IncomingRange {
    type Target = TimeRange;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for IncomingRange {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.range.min == other.range.min
    }
}

impl Eq for IncomingRange {}

impl PartialOrd for IncomingRange {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IncomingRange {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.range.min.cmp(&other.range.min).reverse()
    }
}

struct Ranges {
    new: Vec<TimeRange>,
    incoming: BinaryHeap<IncomingRange>,
}

impl Ranges {
    fn push(&mut self, mut range: TimeRange) {
        let Some(last_range) = self.new.last_mut() else {
            self.new.push(range);
            return;
        };

        // We handle merging ranges differently depending on their state.
        //
        // The goal here is to both prioritize showing unloaded ranges, and have no gaps
        // between ranges. For gaps we extend the last state since we want it to represent what ranges a latest at query has available.
        match (last_range.loaded, range.loaded) {
            // Equal states for both ranges, combine them.
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |-unloaded-|
            //          range:     |---unloaded---|
            // result:
            // new last_range: |-----unloaded-----|
            //
            // case 2:
            //     last_range: |--loaded--|
            //          range:                    |-loaded-|
            //
            // result:
            // new last_range: |---------loaded----------|
            //
            // ```
            (true, true) | (false, false) => {
                last_range.max = last_range.max.max(range.max);
            }
            // The last state should be prioritized
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |--unloaded--|
            //          range:          |--loaded--|
            //
            // result:
            // new last_range: |--unloaded--|
            //  delayed range:              |loaded|
            //
            // case 2:
            //     last_range: |--------unloaded---------|
            //          range:          |loaded|
            //
            // result:
            // new last_range: |--------unloaded---------|
            //
            // case 3:
            //     last_range: |unloaded|
            //          range:              |loaded|
            //
            // result:
            // new last range: |--unloaded--|
            //      new range:              |loaded|
            // ```
            (false, true) => {
                if last_range.max <= range.min {
                    // To not leave any gaps between states, expand the last state
                    last_range.max = range.min;
                    self.new.push(range);
                } else if last_range.max < range.max {
                    // To not have overlapping states, start the new state at the end of the prioritized last state
                    range.min = last_range.max;
                    if range.min < range.max {
                        self.incoming.push(IncomingRange(range));
                    }
                }
            }
            // The new state should be prioritized
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |--loaded--|
            //          range:          |unloaded|
            //
            // result:
            // new last_state: |-loaded-|
            //      new range:          |unloaded|
            //
            // case 2:
            //     last_range: |----------loaded----------|
            //          range:          |unloaded|
            //
            // result:
            // new last_range: |-loaded-|
            //      new range:          |unloaded|
            //  delayed range:                   |-loaded-|
            //
            // case 3:
            //     last_range: |loaded|
            //          range:              |unloaded|
            //
            // result:
            // new last_range: |---loaded---|
            //      new range:              |unloaded|
            // ```
            (true, false) => {
                if range.min <= last_range.max {
                    // To not have overlapping states, start the last state at the end of the prioritized new state
                    if range.max < last_range.max {
                        self.incoming.push(IncomingRange(TimeRange {
                            range: AbsoluteTimeRange::new(range.max, last_range.max),
                            loaded: last_range.loaded,
                        }));
                    }

                    if last_range.min == range.min {
                        // We can replace the last here since we don't want overlapping states
                        *last_range = range;
                    } else {
                        last_range.max = range.min;

                        self.new.push(range);
                    }
                } else {
                    // To not leave any gaps between states, expand the last
                    // state to end at the start of the current state
                    last_range.max = range.max;
                    self.new.push(range);
                }
            }
        }
    }
}

/// Utility to merge multiple ranges of loaded/unloaded ranges into a list of
/// sorted ranges with no gaps or overlaps while also prioritizing unloaded
/// ranges over loaded ones.
pub fn merge_ranges(ranges: &[TimeRange]) -> Vec<TimeRange> {
    re_tracing::profile_function!();

    let mut ranges = Ranges {
        new: Vec::new(),
        incoming: ranges.iter().map(|t| IncomingRange(*t)).collect(),
    };

    while let Some(r) = ranges.incoming.pop() {
        ranges.push(r.0);
    }

    ranges.new
}
