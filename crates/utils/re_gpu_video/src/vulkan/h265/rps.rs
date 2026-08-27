//! Reference picture set construction and DPB slot tracking, spec 8.3.2.
//!
//! H.265 states the whole reference set of a picture in its own slice header:
//! there is no sliding window and no memory management control operations. Each
//! picture lists the counts of every reference it or a later picture still needs,
//! and everything the list leaves out becomes unused for reference. That makes
//! the marking a set difference, and the DPB slot bookkeeping falls out of it.

use crate::ParseError;

use super::ops::ReferenceInfo;

/// The short-term reference picture set of the current picture, as
/// `cros-codecs` resolved it: absolute count deltas, whatever syntax the
/// bitstream used to convey them.
pub struct ShortTermSet<'a> {
    /// `DeltaPocS0`, the counts below the current one, descending.
    pub delta_poc_s0: &'a [i32],

    /// `UsedByCurrPicS0`, whether the entry is a reference of this picture
    /// rather than only of a later one.
    pub used_by_curr_pic_s0: &'a [bool],

    /// `DeltaPocS1`, the counts above the current one, ascending.
    pub delta_poc_s1: &'a [i32],

    /// `UsedByCurrPicS1`.
    pub used_by_curr_pic_s1: &'a [bool],
}

/// One entry of the long-term reference picture set of the current picture.
pub struct LongTermEntry {
    /// `PocLsbLt`: the least-significant bits of the reference's count.
    pub poc_lsb_lt: u32,

    /// `DeltaPocMsbCycleLt`, only meaningful with `msb_present`.
    pub delta_poc_msb_cycle_lt: u32,

    /// The most-significant bits are given, so the full count identifies the
    /// reference. Otherwise the least-significant bits alone have to.
    pub msb_present: bool,

    /// `UsedByCurrPicLt`: a reference of this picture rather than only of a later one.
    pub used_by_curr_pic: bool,
}

/// What [`Dpb::build_reference_sets`] needs about the current picture.
pub struct CurrentPicture<'a> {
    pub poc: i32,

    /// The picture starts a new prediction sequence (`NoRaslOutputFlag` equal
    /// to 1): the buffer is emptied before it is decoded.
    pub starts_sequence: bool,

    /// `MaxPicOrderCntLsb` of the active SPS.
    pub max_poc_lsb: i32,

    pub short_term: ShortTermSet<'a>,
    pub long_term: &'a [LongTermEntry],
}

/// The reference sets of one picture, as DPB slots.
pub struct ReferenceSets {
    /// The union of the three lists below, sorted by slot: what the decode
    /// operation binds as its active reference slots.
    pub references: Vec<ReferenceInfo>,

    /// `RefPicSetStCurrBefore`, in list order.
    pub st_curr_before: Vec<u8>,

    /// `RefPicSetStCurrAfter`.
    pub st_curr_after: Vec<u8>,

    /// `RefPicSetLtCurr`.
    pub lt_curr: Vec<u8>,

    /// The slot the current picture decodes into.
    pub setup_slot: u8,
}

/// One picture held in the decoded picture buffer.
struct Entry {
    slot: u8,
    poc: i32,
    is_long_term: bool,
}

/// The decoded picture buffer: which slot holds which picture.
pub struct Dpb {
    entries: Vec<Entry>,

    /// Slot capacity, the stream's requirement bounded by the hardware's.
    capacity: u8,
}

impl Dpb {
    pub fn new(capacity: u8) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Drops every picture, for a seek or an IDR.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// No picture is held: the start of the stream, or right after a seek.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Grows or shrinks to what a new SPS demands, within the hardware's capacity.
    pub fn configure(&mut self, needed_slots: u32, max_dpb_slots: u8) -> Result<(), ParseError> {
        let needed = u8::try_from(needed_slots).map_err(|_err| ParseError::TooManyRefFrames {
            needed: needed_slots,
            available: max_dpb_slots,
        })?;
        if needed > max_dpb_slots {
            return Err(ParseError::TooManyRefFrames {
                needed: needed_slots,
                available: max_dpb_slots,
            });
        }
        self.capacity = needed;
        // A shrinking DPB can only happen at an IDR, which emptied it first.
        self.entries.retain(|entry| entry.slot < needed);
        Ok(())
    }

    /// Builds the current picture's reference sets, marks everything the stream
    /// no longer needs as unused, and assigns the picture its own slot.
    ///
    /// Returns the slots that became free, for tracing.
    pub fn build_reference_sets(
        &mut self,
        current: &CurrentPicture<'_>,
    ) -> Result<(ReferenceSets, Vec<u8>), ParseError> {
        if current.starts_sequence {
            self.clear();
        }

        // Step 1: the counts of the pictures this one and its successors need
        // (spec 8.3.2, equations 8-6 through 8-9).
        let short_term = &current.short_term;
        let mut poc_st_curr_before = Vec::new();
        let mut poc_st_curr_after = Vec::new();
        let mut poc_st_foll = Vec::new();
        for (&delta, &used) in short_term
            .delta_poc_s0
            .iter()
            .zip(short_term.used_by_curr_pic_s0)
        {
            let poc = current.poc.wrapping_add(delta);
            if used {
                poc_st_curr_before.push(poc);
            } else {
                poc_st_foll.push(poc);
            }
        }
        for (&delta, &used) in short_term
            .delta_poc_s1
            .iter()
            .zip(short_term.used_by_curr_pic_s1)
        {
            let poc = current.poc.wrapping_add(delta);
            if used {
                poc_st_curr_after.push(poc);
            } else {
                poc_st_foll.push(poc);
            }
        }

        // The long-term entries, either by full count or by its lower bits only.
        let mut lt_curr = Vec::new();
        let mut lt_foll = Vec::new();
        for entry in current.long_term {
            let key = if entry.msb_present {
                // Reconstruct the full count from the cycle the slice header gives.
                let cycles = i32::try_from(entry.delta_poc_msb_cycle_lt).map_err(|_err| {
                    ParseError::Invalid("long-term reference count out of range")
                })?;
                let poc = current
                    .poc
                    .wrapping_sub(cycles.wrapping_mul(current.max_poc_lsb))
                    .wrapping_sub(current.poc & (current.max_poc_lsb - 1))
                    .wrapping_add(i32::try_from(entry.poc_lsb_lt).map_err(|_err| {
                        ParseError::Invalid("long-term reference count out of range")
                    })?);
                LongTermKey::FullCount(poc)
            } else {
                LongTermKey::LowerBits(i32::try_from(entry.poc_lsb_lt).map_err(|_err| {
                    ParseError::Invalid("long-term reference count out of range")
                })?)
            };
            if entry.used_by_curr_pic {
                lt_curr.push(key);
            } else {
                lt_foll.push(key);
            }
        }

        // Step 2: everything the lists leave out is unused for reference from now on.
        let keeps_short_term = |poc: i32| {
            poc_st_curr_before.contains(&poc)
                || poc_st_curr_after.contains(&poc)
                || poc_st_foll.contains(&poc)
        };
        let keeps_long_term = |poc: i32| {
            lt_curr
                .iter()
                .chain(&lt_foll)
                .any(|key| key.matches(poc, current.max_poc_lsb))
        };
        let mut freed = Vec::new();
        self.entries.retain(|entry| {
            let keep = keeps_short_term(entry.poc) || keeps_long_term(entry.poc);
            if !keep {
                freed.push(entry.slot);
            }
            keep
        });

        // The long-term lists also decide which of the kept pictures are long-term
        // references, which is what the reference metadata reports to the driver.
        for entry in &mut self.entries {
            entry.is_long_term = lt_curr
                .iter()
                .chain(&lt_foll)
                .any(|key| key.matches(entry.poc, current.max_poc_lsb));
        }

        // Step 3: resolve the lists this picture predicts from into slots.
        let st_curr_before = self.resolve_short_term(&poc_st_curr_before)?;
        let st_curr_after = self.resolve_short_term(&poc_st_curr_after)?;
        let lt_curr = self.resolve_long_term(&lt_curr, current.max_poc_lsb)?;

        // The union, with the metadata the decode operation binds per slot.
        let mut references: Vec<ReferenceInfo> = Vec::new();
        for &slot in st_curr_before.iter().chain(&st_curr_after).chain(&lt_curr) {
            if references.iter().any(|reference| reference.slot == slot) {
                continue;
            }
            let entry = self.entries.iter().find(|entry| entry.slot == slot).ok_or(
                ParseError::MissingReference {
                    what: "reference picture",
                },
            )?;
            references.push(ReferenceInfo {
                slot,
                poc: entry.poc,
                is_long_term: entry.is_long_term,
            });
        }
        references.sort_by_key(|reference| reference.slot);

        // Step 4: the current picture takes a slot of its own. Every H.265 picture
        // is a short-term reference right after decoding, a later picture's set is
        // what releases it again.
        let setup_slot = self.take_free_slot()?;
        self.entries.push(Entry {
            slot: setup_slot,
            poc: current.poc,
            is_long_term: false,
        });

        Ok((
            ReferenceSets {
                references,
                st_curr_before,
                st_curr_after,
                lt_curr,
                setup_slot,
            },
            freed,
        ))
    }

    /// The slots holding the pictures with these counts, in list order.
    fn resolve_short_term(&self, pocs: &[i32]) -> Result<Vec<u8>, ParseError> {
        pocs.iter()
            .map(|&poc| {
                self.entries
                    .iter()
                    .find(|entry| entry.poc == poc)
                    .map(|entry| entry.slot)
                    .ok_or(ParseError::MissingReference {
                        what: "short-term reference picture",
                    })
            })
            .collect()
    }

    fn resolve_long_term(
        &self,
        keys: &[LongTermKey],
        max_poc_lsb: i32,
    ) -> Result<Vec<u8>, ParseError> {
        keys.iter()
            .map(|key| {
                self.entries
                    .iter()
                    .find(|entry| key.matches(entry.poc, max_poc_lsb))
                    .map(|entry| entry.slot)
                    .ok_or(ParseError::MissingReference {
                        what: "long-term reference picture",
                    })
            })
            .collect()
    }

    /// The lowest slot index no picture occupies.
    fn take_free_slot(&self) -> Result<u8, ParseError> {
        (0..self.capacity)
            .find(|slot| !self.entries.iter().any(|entry| entry.slot == *slot))
            .ok_or(ParseError::DpbOverflow)
    }
}

/// How a long-term reference picture is identified: by its full count, or, when
/// the slice header left the upper bits out, by the lower bits alone.
enum LongTermKey {
    FullCount(i32),
    LowerBits(i32),
}

impl LongTermKey {
    fn matches(&self, poc: i32, max_poc_lsb: i32) -> bool {
        match self {
            Self::FullCount(expected) => *expected == poc,
            Self::LowerBits(expected) => *expected == (poc & (max_poc_lsb - 1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short_term_set<'a>(
        before: &'a [i32],
        before_used: &'a [bool],
        after: &'a [i32],
        after_used: &'a [bool],
    ) -> ShortTermSet<'a> {
        ShortTermSet {
            delta_poc_s0: before,
            used_by_curr_pic_s0: before_used,
            delta_poc_s1: after,
            used_by_curr_pic_s1: after_used,
        }
    }

    fn decode(
        dpb: &mut Dpb,
        poc: i32,
        starts_sequence: bool,
        before: &[i32],
        after: &[i32],
    ) -> ReferenceSets {
        let before_used = vec![true; before.len()];
        let after_used = vec![true; after.len()];
        let current = CurrentPicture {
            poc,
            starts_sequence,
            max_poc_lsb: 256,
            short_term: short_term_set(before, &before_used, after, &after_used),
            long_term: &[],
        };
        dpb.build_reference_sets(&current).unwrap().0
    }

    /// A plain forward-predicted sequence reuses the slot of the picture whose
    /// reference the newest one replaces.
    #[test]
    fn forward_prediction_recycles_slots() {
        let mut dpb = Dpb::new(3);
        let idr = decode(&mut dpb, 0, true, &[], &[]);
        assert_eq!(idr.setup_slot, 0);
        assert!(idr.references.is_empty());

        // Each picture references only the one before it.
        let first = decode(&mut dpb, 1, false, &[-1], &[]);
        assert_eq!(first.setup_slot, 1);
        assert_eq!(first.st_curr_before, vec![0]);

        // The IDR is no longer in the set, so its slot comes free and is reused.
        let second = decode(&mut dpb, 2, false, &[-1], &[]);
        assert_eq!(second.setup_slot, 0);
        assert_eq!(second.st_curr_before, vec![1]);
    }

    /// A picture predicting from both directions binds both slots, in the order
    /// of the two lists.
    #[test]
    fn bidirectional_prediction_binds_both_lists() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);
        // The far end of the group, predicted from the IDR.
        let last = decode(&mut dpb, 4, false, &[-4], &[]);
        assert_eq!(last.setup_slot, 1);

        // A picture between the two, predicting from both.
        let middle = decode(&mut dpb, 2, false, &[-2], &[2]);
        assert_eq!(middle.st_curr_before, vec![0]);
        assert_eq!(middle.st_curr_after, vec![1]);
        assert_eq!(middle.references.len(), 2);
        assert_eq!(middle.setup_slot, 2);
    }

    /// Pictures a later picture still needs stay in the buffer even while the
    /// current one doesn't reference them.
    #[test]
    fn references_of_later_pictures_stay() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);
        decode(&mut dpb, 4, false, &[-4], &[]);

        // References the picture at 4 only, but keeps the one at 0 for later.
        let before_used = [false];
        let after_used = [true];
        let current = CurrentPicture {
            poc: 2,
            starts_sequence: false,
            max_poc_lsb: 256,
            short_term: short_term_set(&[-2], &before_used, &[2], &after_used),
            long_term: &[],
        };
        let (sets, freed) = dpb.build_reference_sets(&current).unwrap();
        assert!(freed.is_empty());
        assert!(sets.st_curr_before.is_empty());
        assert_eq!(sets.st_curr_after, vec![1]);
        assert_eq!(sets.setup_slot, 2);
    }

    /// A random access point empties the buffer, so the next picture starts at
    /// the first slot again.
    #[test]
    fn a_random_access_point_empties_the_buffer() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);
        decode(&mut dpb, 1, false, &[-1], &[]);
        decode(&mut dpb, 2, false, &[-1], &[]);

        let idr = decode(&mut dpb, 0, true, &[], &[]);
        assert_eq!(idr.setup_slot, 0);
        assert!(idr.references.is_empty());
    }

    /// A reference the buffer never held is an error rather than silent corruption.
    #[test]
    fn a_missing_reference_is_an_error() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);

        let before_used = [true];
        let current = CurrentPicture {
            poc: 8,
            starts_sequence: false,
            max_poc_lsb: 256,
            short_term: short_term_set(&[-4], &before_used, &[], &[]),
            long_term: &[],
        };
        assert!(dpb.build_reference_sets(&current).is_err());
    }

    /// A long-term reference signalled with only the lower bits of its count is
    /// matched on those bits, across a wrap of the counter.
    #[test]
    fn long_term_references_match_on_their_lower_bits() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);
        decode(&mut dpb, 1, false, &[-1], &[]);

        // The picture at 0 becomes a long-term reference of a picture a whole
        // counter cycle later, addressed by the lower bits of its count.
        let long_term = [LongTermEntry {
            poc_lsb_lt: 0,
            delta_poc_msb_cycle_lt: 0,
            msb_present: false,
            used_by_curr_pic: true,
        }];
        let current = CurrentPicture {
            poc: 16,
            starts_sequence: false,
            max_poc_lsb: 16,
            short_term: short_term_set(&[], &[], &[], &[]),
            long_term: &long_term,
        };
        let (sets, _freed) = dpb.build_reference_sets(&current).unwrap();
        assert_eq!(sets.lt_curr, vec![0]);
        assert!(sets.references[0].is_long_term);
    }

    /// A long-term reference given with its full count matches that count only.
    #[test]
    fn long_term_references_match_on_their_full_count() {
        let mut dpb = Dpb::new(4);
        decode(&mut dpb, 0, true, &[], &[]);
        decode(&mut dpb, 16, false, &[-16], &[]);

        // Both pictures share the lower bits, only the cycle tells them apart.
        let long_term = [LongTermEntry {
            poc_lsb_lt: 0,
            delta_poc_msb_cycle_lt: 2,
            msb_present: true,
            used_by_curr_pic: true,
        }];
        let current = CurrentPicture {
            poc: 32,
            starts_sequence: false,
            max_poc_lsb: 16,
            short_term: short_term_set(&[], &[], &[], &[]),
            long_term: &long_term,
        };
        let (sets, _freed) = dpb.build_reference_sets(&current).unwrap();
        // Two cycles below 32 is the picture at 0.
        assert_eq!(sets.lt_curr, vec![0]);
    }

    /// A stream needing more slots than the buffer has is rejected.
    #[test]
    fn running_out_of_slots_is_an_error() {
        let mut dpb = Dpb::new(2);
        decode(&mut dpb, 0, true, &[], &[]);
        decode(&mut dpb, 1, false, &[-1], &[]);

        // Keeps both earlier pictures and needs a third slot for itself.
        let before_used = [true, true];
        let current = CurrentPicture {
            poc: 2,
            starts_sequence: false,
            max_poc_lsb: 256,
            short_term: short_term_set(&[-1, -2], &before_used, &[], &[]),
            long_term: &[],
        };
        assert!(matches!(
            dpb.build_reference_sets(&current),
            Err(ParseError::DpbOverflow)
        ));
    }
}
