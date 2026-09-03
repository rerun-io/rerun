//! Reference frame tracking: the decoded picture buffer, reference picture list
//! construction (spec 8.2.4), and reference picture marking (spec 8.2.5).
//!
//! Also owns the DPB slot assignment: every reference frame occupies one slot from
//! decode until it is unmarked. Non-reference frames get a scratch slot nothing
//! references, see [`MarkOutcome::scratch_slot`].

use h264_reader::nal::slice::{
    DecRefPicMarking, MemoryManagementControlOperation, ModificationOfPicNums,
    RefPicListModifications, SliceHeader,
};
use h264_reader::nal::{pps::PicParameterSet, sps::SeqParameterSet};

use super::ParseError;
use super::ops::{RefLists, ReferenceInfo};
use super::poc::Poc;

/// One reference frame in the DPB.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    slot: u8,
    frame_num: u16,
    poc: Poc,

    /// `Some(LongTermFrameIdx)` for long-term references.
    long_term_frame_idx: Option<u16>,
}

impl Entry {
    fn is_short_term(&self) -> bool {
        self.long_term_frame_idx.is_none()
    }

    /// `FrameNumWrap` (spec 8.2.4.1): `frame_num` relative to the current frame,
    /// values from before the last `frame_num` wraparound become negative.
    fn frame_num_wrap(&self, current_frame_num: u16, max_frame_num: i32) -> i32 {
        if i32::from(self.frame_num) > i32::from(current_frame_num) {
            i32::from(self.frame_num) - max_frame_num
        } else {
            i32::from(self.frame_num)
        }
    }

    fn reference_info(&self) -> ReferenceInfo {
        ReferenceInfo {
            slot: self.slot,
            frame_num: self.long_term_frame_idx.unwrap_or(self.frame_num),
            top_field_order_cnt: self.poc.top,
            bottom_field_order_cnt: self.poc.bottom,
            is_long_term: self.long_term_frame_idx.is_some(),
        }
    }
}

/// What the current frame looks like to the DPB.
pub struct CurrentFrame {
    pub frame_num: u16,
    pub poc: Poc,

    /// `MaxFrameNum` of the active SPS.
    pub max_frame_num: i32,
}

/// The outcome of marking a reference frame, see [`Dpb::mark`].
#[derive(Default)]
pub struct MarkOutcome {
    /// The slot the current frame gets decoded into, `None` for non-reference frames.
    pub setup_slot: Option<u8>,

    /// A free slot for non-reference frames to activate, `None` when `setup_slot` is set.
    /// See [`super::ops::DecodeInfo::scratch_slot`].
    pub scratch_slot: Option<u8>,

    /// Set when the current frame was marked as a long-term reference.
    pub long_term_frame_idx: Option<u16>,

    /// Slots of reference frames unmarked by this frame, free for reuse afterwards.
    pub freed: Vec<u8>,
}

/// The decoded picture buffer: all active reference frames and their slots.
#[derive(Default)]
pub struct Dpb {
    entries: Vec<Entry>,

    /// Total slots: enough for `max_num_ref_frames` references plus the frame being decoded.
    num_slots: u8,

    /// `Max(max_num_ref_frames, 1)` of the active SPS, the sliding window capacity.
    max_num_ref_frames: u32,

    /// `MaxLongTermFrameIdx`, `None` meaning "no long-term frame indices"
    /// (all long-term references are disallowed).
    max_long_term_frame_idx: Option<u16>,

    /// `frame_num` of the most recent reference frame, for `frame_num` gap detection.
    prev_ref_frame_num: u16,
}

impl Dpb {
    /// Applies the DPB requirements of the SPS that just became active.
    ///
    /// `max_dpb_slots` is the hardware slot capacity.
    pub fn configure(
        &mut self,
        sps: &SeqParameterSet,
        max_dpb_slots: u8,
    ) -> Result<(), ParseError> {
        let max_num_ref_frames = sps.max_num_ref_frames.max(1);
        let needed = max_num_ref_frames + 1;
        if needed > u32::from(max_dpb_slots) {
            return Err(ParseError::TooManyRefFrames {
                needed,
                available: max_dpb_slots,
            });
        }

        let num_slots = needed as u8;
        if num_slots < self.num_slots && !self.entries.is_empty() {
            // Shrinking while reference frames are active would invalidate slots. Real streams
            // change these requirements at IDR frames, where the DPB is empty.
            return Err(ParseError::Unsupported(
                "SPS shrinking the DPB while reference frames are active",
            ));
        }
        self.num_slots = self.num_slots.max(num_slots);
        self.max_num_ref_frames = max_num_ref_frames;
        Ok(())
    }

    /// Unmarks all reference frames, returning the freed slots. Used at IDR frames and resets.
    pub fn clear(&mut self) -> Vec<u8> {
        self.max_long_term_frame_idx = None;
        self.prev_ref_frame_num = 0;
        let mut freed: Vec<u8> = self.entries.drain(..).map(|entry| entry.slot).collect();
        freed.sort_unstable();
        freed
    }

    /// Detects lost reference frames (spec 7.4.3): between two frames, `frame_num` either
    /// stays (non-reference frame, or a frame following one) or increments by one.
    ///
    /// Streams produced under packet loss (`gaps_in_frame_num_value_allowed_flag`) would
    /// require synthesizing "non-existing" reference frames, which hardware decode has no
    /// pictures for. Both cases are errors, making the decoder fall back.
    pub fn check_frame_num(&self, sps: &SeqParameterSet, frame_num: u16) -> Result<(), ParseError> {
        let max_frame_num = 1u32 << sps.log2_max_frame_num();
        let expected_next = ((u32::from(self.prev_ref_frame_num) + 1) % max_frame_num) as u16;
        if frame_num == self.prev_ref_frame_num || frame_num == expected_next {
            Ok(())
        } else {
            Err(ParseError::FrameNumGap {
                got: frame_num,
                expected: expected_next,
                gaps_allowed: sps.gaps_in_frame_num_value_allowed_flag,
            })
        }
    }

    /// Builds the reference picture lists for one P or B slice
    /// (spec 8.2.4: initialization plus the slice header's modifications).
    ///
    /// Returns the lists as slot indices, and appends the referenced frames to
    /// `references`, deduplicated by slot.
    pub fn ref_lists(
        &self,
        sps: &SeqParameterSet,
        pps: &PicParameterSet,
        header: &SliceHeader,
        current: &CurrentFrame,
        references: &mut Vec<ReferenceInfo>,
    ) -> Result<RefLists, ParseError> {
        let max_frame_num = 1i32 << sps.log2_max_frame_num();

        let empty = Vec::new();
        let (is_b, modifications_l0, modifications_l1) = match &header.ref_pic_list_modification {
            Some(RefPicListModifications::P {
                ref_pic_list_modification_l0,
            }) => (false, ref_pic_list_modification_l0, &empty),
            Some(RefPicListModifications::B {
                ref_pic_list_modification_l0,
                ref_pic_list_modification_l1,
            }) => (
                true,
                ref_pic_list_modification_l0,
                ref_pic_list_modification_l1,
            ),
            Some(RefPicListModifications::I) | None => return Ok(RefLists::default()),
        };

        if self.entries.is_empty() {
            return Err(ParseError::NoReferencesAvailable);
        }

        let (num_active_l0, num_active_l1) = num_active_references(pps, header, is_b);

        let (l0_init, l1_init) = if is_b {
            let l0 = self.init_list_b(current, false);
            let mut l1 = self.init_list_b(current, true);
            // Spec 8.2.4.2.3 step 3: when the initial B lists come out identical and hold
            // more than one frame, the first two entries of l1 are swapped.
            if l1.len() > 1 && l0 == l1 {
                l1.swap(0, 1);
            }
            (l0, l1)
        } else {
            (self.init_list_p(current, max_frame_num), Vec::new())
        };

        let mut l0 = self.modify_list(
            l0_init,
            modifications_l0,
            num_active_l0,
            current,
            max_frame_num,
        )?;
        let mut l1 = self.modify_list(
            l1_init,
            modifications_l1,
            num_active_l1,
            current,
            max_frame_num,
        )?;

        l0.truncate(num_active_l0);
        l1.truncate(num_active_l1);

        for entry in std::iter::chain(&l0, &l1) {
            if !references
                .iter()
                .any(|existing| existing.slot == entry.slot)
            {
                references.push(entry.reference_info());
            }
        }

        Ok(RefLists {
            l0: l0.iter().map(|entry| entry.slot).collect(),
            l1: l1.iter().map(|entry| entry.slot).collect(),
        })
    }

    /// Initial list for P slices (spec 8.2.4.2.1):
    /// short-term by descending `PicNum`, then long-term by ascending `LongTermPicNum`.
    fn init_list_p(&self, current: &CurrentFrame, max_frame_num: i32) -> Vec<&Entry> {
        let mut list: Vec<&Entry> = self.entries.iter().collect();
        list.sort_by_key(|entry| match entry.long_term_frame_idx {
            None => (0, -entry.frame_num_wrap(current.frame_num, max_frame_num)),
            Some(long_term_idx) => (1, i32::from(long_term_idx)),
        });
        list
    }

    /// Initial lists for B slices (spec 8.2.4.2.3), keyed on `PicOrderCnt`.
    ///
    /// l0: short-term before the current frame by descending POC, then those after it by
    /// ascending POC, then long-term. l1 mirrors the two short-term groups.
    fn init_list_b(&self, current: &CurrentFrame, is_l1: bool) -> Vec<&Entry> {
        let current_poc = current.poc.poc();
        let mut list: Vec<&Entry> = self.entries.iter().collect();
        list.sort_by_key(|entry| {
            if let Some(long_term_idx) = entry.long_term_frame_idx {
                (2, i32::from(long_term_idx))
            } else {
                let poc = entry.poc.poc();
                let before = poc < current_poc;
                // The "before" group descends, the "after" group ascends.
                let key = if before { -poc } else { poc };
                let group = usize::from(before == is_l1);
                (group, key)
            }
        });
        list
    }

    /// Applies the slice header's list modifications (spec 8.2.4.3).
    fn modify_list<'a>(
        &'a self,
        initial: Vec<&'a Entry>,
        modifications: &[ModificationOfPicNums],
        num_active: usize,
        current: &CurrentFrame,
        max_frame_num: i32,
    ) -> Result<Vec<&'a Entry>, ParseError> {
        let mut list = initial;
        if modifications.is_empty() {
            return Ok(list);
        }

        // The spec runs the insertions on the truncated initial list in a workspace
        // one entry longer than the active count, truncated back by the caller.
        list.truncate(num_active);

        let current_pic_num = i32::from(current.frame_num);
        let max_pic_num = max_frame_num;
        let mut pic_num_prediction = current_pic_num;
        let mut insert_index = 0;

        for modification in modifications {
            let entry = match modification {
                ModificationOfPicNums::Subtract(abs_diff_minus1)
                | ModificationOfPicNums::Add(abs_diff_minus1) => {
                    let diff = abs_diff_minus1.cast_signed() + 1;
                    let mut pic_num_no_wrap =
                        if matches!(modification, ModificationOfPicNums::Subtract(_)) {
                            pic_num_prediction - diff
                        } else {
                            pic_num_prediction + diff
                        };
                    if pic_num_no_wrap < 0 {
                        pic_num_no_wrap += max_pic_num;
                    } else if pic_num_no_wrap >= max_pic_num {
                        pic_num_no_wrap -= max_pic_num;
                    }
                    pic_num_prediction = pic_num_no_wrap;

                    let pic_num = if pic_num_no_wrap > current_pic_num {
                        pic_num_no_wrap - max_pic_num
                    } else {
                        pic_num_no_wrap
                    };
                    self.entries
                        .iter()
                        .find(|entry| {
                            entry.is_short_term()
                                && entry.frame_num_wrap(current.frame_num, max_frame_num) == pic_num
                        })
                        .ok_or(ParseError::MissingReference {
                            what: "short-term reference for list modification",
                        })?
                }

                ModificationOfPicNums::LongTermRef(long_term_pic_num) => self
                    .entries
                    .iter()
                    .find(|entry| entry.long_term_frame_idx == Some(*long_term_pic_num as u16))
                    .ok_or(ParseError::MissingReference {
                        what: "long-term reference for list modification",
                    })?,
            };

            // Spec 8-37/8-38: insert at the running index. The shift drops whatever
            // falls off the workspace, then a later duplicate of the same frame is
            // removed and the tail compacted.
            list.insert(insert_index.min(list.len()), entry);
            insert_index += 1;
            list.truncate(num_active + 1);
            if let Some(duplicate) = list[insert_index..]
                .iter()
                .position(|existing| existing.slot == entry.slot)
            {
                list.remove(insert_index + duplicate);
            }
        }

        Ok(list)
    }

    /// Runs the reference picture marking of the current frame (spec 8.2.5) and assigns
    /// its DPB slot. Pass `marking: None` for non-reference frames, which touch nothing.
    ///
    /// The setup slot never collides with pre-marking references, so the backend can
    /// decode with the old references bound and deactivate the freed slots afterwards.
    pub fn mark(
        &mut self,
        current: &CurrentFrame,
        marking: Option<&DecRefPicMarking>,
    ) -> Result<MarkOutcome, ParseError> {
        let Some(marking) = marking else {
            return Ok(MarkOutcome {
                scratch_slot: Some(self.allocate_slot()?),
                ..MarkOutcome::default()
            });
        };

        let mut freed = Vec::new();
        let mut long_term_frame_idx = None;

        // The frame being decoded needs its slot before eviction makes more room:
        // the slot count leaves one slot beyond the sliding window capacity for exactly this.
        let setup_slot = self.allocate_slot()?;

        match marking {
            DecRefPicMarking::Idr {
                no_output_of_prior_pics_flag: _,
                long_term_reference_flag,
            } => {
                freed = self.clear();
                if *long_term_reference_flag {
                    self.max_long_term_frame_idx = Some(0);
                    long_term_frame_idx = Some(0);
                } else {
                    self.max_long_term_frame_idx = None;
                }
            }

            DecRefPicMarking::SlidingWindow => {
                // Spec 8.2.5.3: at capacity, the short-term reference with the smallest
                // `FrameNumWrap` is evicted.
                if self.entries.len() as u32 >= self.max_num_ref_frames {
                    let oldest = self
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.is_short_term())
                        .min_by_key(|(_, entry)| {
                            entry.frame_num_wrap(current.frame_num, current.max_frame_num)
                        })
                        .map(|(index, _)| index)
                        .ok_or(ParseError::DpbOverflow)?;
                    freed.push(self.entries.swap_remove(oldest).slot);
                }
            }

            DecRefPicMarking::Adaptive(operations) => {
                long_term_frame_idx =
                    self.apply_adaptive_marking(operations, current, &mut freed)?;
                // Adaptive marking must keep the DPB within `max_num_ref_frames` on its
                // own, there is no sliding window to fall back to. The current frame is
                // about to be added.
                if self.entries.len() as u32 + 1 > self.max_num_ref_frames {
                    return Err(ParseError::DpbOverflow);
                }
            }
        }

        // After memory management control operation 5 the current frame
        // is treated as if it had `frame_num` 0.
        let frame_num = if has_mmco5(Some(marking)) {
            0
        } else {
            current.frame_num
        };
        self.entries.push(Entry {
            slot: setup_slot,
            frame_num,
            poc: current.poc,
            long_term_frame_idx,
        });
        self.prev_ref_frame_num = frame_num;

        Ok(MarkOutcome {
            setup_slot: Some(setup_slot),
            scratch_slot: None,
            long_term_frame_idx,
            freed,
        })
    }

    /// The memory management control operations of adaptive marking (spec 8.2.5.4).
    /// Returns the long-term frame index assigned to the current frame, if any.
    fn apply_adaptive_marking(
        &mut self,
        operations: &[MemoryManagementControlOperation],
        current: &CurrentFrame,
        freed: &mut Vec<u8>,
    ) -> Result<Option<u16>, ParseError> {
        let mut current_long_term_frame_idx = None;

        for operation in operations {
            match operation {
                MemoryManagementControlOperation::ShortTermUnusedForRef {
                    difference_of_pic_nums_minus1,
                } => {
                    let index = self.find_short_term(current, *difference_of_pic_nums_minus1)?;
                    freed.push(self.entries.swap_remove(index).slot);
                }

                MemoryManagementControlOperation::LongTermUnusedForRef { long_term_pic_num } => {
                    let index = self
                        .entries
                        .iter()
                        .position(|entry| {
                            entry.long_term_frame_idx == Some(*long_term_pic_num as u16)
                        })
                        .ok_or(ParseError::MissingReference {
                            what: "long-term reference to unmark",
                        })?;
                    freed.push(self.entries.swap_remove(index).slot);
                }

                MemoryManagementControlOperation::ShortTermUsedForLongTerm {
                    difference_of_pic_nums_minus1,
                    long_term_frame_idx,
                } => {
                    // A long-term reference already holding the target index is replaced.
                    if let Some(existing) = self.entries.iter().position(|entry| {
                        entry.long_term_frame_idx == Some(*long_term_frame_idx as u16)
                    }) {
                        freed.push(self.entries.swap_remove(existing).slot);
                    }
                    let index = self.find_short_term(current, *difference_of_pic_nums_minus1)?;
                    self.entries[index].long_term_frame_idx = Some(*long_term_frame_idx as u16);
                }

                MemoryManagementControlOperation::MaxUsedLongTermFrameRef {
                    max_long_term_frame_idx_plus1,
                } => {
                    self.max_long_term_frame_idx = max_long_term_frame_idx_plus1
                        .checked_sub(1)
                        .map(|idx| idx as u16);
                    let max = self.max_long_term_frame_idx;
                    self.entries.retain(|entry| {
                        if let Some(idx) = entry.long_term_frame_idx
                            && max.is_none_or(|max| idx > max)
                        {
                            freed.push(entry.slot);
                            false
                        } else {
                            true
                        }
                    });
                }

                MemoryManagementControlOperation::AllRefPicturesUnused => {
                    freed.append(&mut self.clear());
                }

                MemoryManagementControlOperation::CurrentUsedForLongTerm {
                    long_term_frame_idx,
                } => {
                    if let Some(existing) = self.entries.iter().position(|entry| {
                        entry.long_term_frame_idx == Some(*long_term_frame_idx as u16)
                    }) {
                        freed.push(self.entries.swap_remove(existing).slot);
                    }
                    current_long_term_frame_idx = Some(*long_term_frame_idx as u16);
                }
            }
        }

        Ok(current_long_term_frame_idx)
    }

    /// Finds the short-term reference addressed by `difference_of_pic_nums_minus1`
    /// relative to the current frame (spec 8.2.5.4.1).
    fn find_short_term(
        &self,
        current: &CurrentFrame,
        difference_of_pic_nums_minus1: u32,
    ) -> Result<usize, ParseError> {
        // `CurrPicNum - (difference_of_pic_nums_minus1 + 1)`, compared against `PicNum`
        // which equals `FrameNumWrap` for frames.
        let pic_num =
            i32::from(current.frame_num) - (difference_of_pic_nums_minus1.cast_signed() + 1);
        self.entries
            .iter()
            .position(|entry| {
                entry.is_short_term()
                    && entry.frame_num_wrap(current.frame_num, current.max_frame_num) == pic_num
            })
            .ok_or(ParseError::MissingReference {
                what: "short-term reference addressed by memory management",
            })
    }

    /// The smallest slot index not occupied by a reference frame.
    fn allocate_slot(&self) -> Result<u8, ParseError> {
        (0..self.num_slots)
            .find(|slot| !self.entries.iter().any(|entry| entry.slot == *slot))
            .ok_or(ParseError::DpbOverflow)
    }
}

/// Whether the marking contains memory management control operation 5,
/// which rebases the POC and `frame_num` of the current frame.
pub fn has_mmco5(marking: Option<&DecRefPicMarking>) -> bool {
    matches!(marking, Some(DecRefPicMarking::Adaptive(operations))
        if operations
            .iter()
            .any(|op| matches!(op, MemoryManagementControlOperation::AllRefPicturesUnused)))
}

/// Active reference count per list, from the slice header override or the PPS defaults.
fn num_active_references(
    pps: &PicParameterSet,
    header: &SliceHeader,
    is_b: bool,
) -> (usize, usize) {
    use h264_reader::nal::slice::NumRefIdxActive;

    match &header.num_ref_idx_active {
        Some(NumRefIdxActive::P {
            num_ref_idx_l0_active_minus1,
        }) => (*num_ref_idx_l0_active_minus1 as usize + 1, 0),
        Some(NumRefIdxActive::B {
            num_ref_idx_l0_active_minus1,
            num_ref_idx_l1_active_minus1,
        }) => (
            *num_ref_idx_l0_active_minus1 as usize + 1,
            *num_ref_idx_l1_active_minus1 as usize + 1,
        ),
        None => (
            pps.num_ref_idx_l0_default_active_minus1 as usize + 1,
            if is_b {
                pps.num_ref_idx_l1_default_active_minus1 as usize + 1
            } else {
                0
            },
        ),
    }
}
