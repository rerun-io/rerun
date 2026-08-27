//! The reference slot bookkeeping of spec 7.20 (reference frame update process).
//!
//! AV1 keeps eight reference slots that a frame header addresses by name. Each
//! decoded picture takes one Vulkan DPB slot and is then written into the
//! reference slots its `refresh_frame_flags` names, possibly several at once.
//! A DPB slot stays occupied while at least one reference slot points at it.

use crate::ParseError;

use super::ops::ReferenceInfo;

/// Reference slots the bitstream addresses, `NUM_REF_FRAMES` in the spec.
pub const NUM_REF_FRAMES: usize = 8;

/// Reference names one inter frame binds, `REFS_PER_FRAME` in the spec.
pub const REFS_PER_FRAME: usize = 7;

/// One decoded picture occupying a DPB slot.
struct Picture {
    picture_id: u64,
    reference: ReferenceInfo,
}

/// The decoded picture buffer: DPB slots and the reference names pointing at them.
pub struct Dpb {
    /// The picture in each DPB slot, `None` while the slot is free.
    slots: Vec<Option<Picture>>,

    /// The DPB slot each reference name refers to.
    refs: [Option<u8>; NUM_REF_FRAMES],
}

impl Dpb {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            refs: [None; NUM_REF_FRAMES],
        }
    }

    /// Sizes the buffer for a stream needing `dpb_slots` slots, on a device
    /// offering `max_dpb_slots`. Drops everything the buffer held.
    pub fn configure(&mut self, dpb_slots: u32, max_dpb_slots: u8) -> Result<(), ParseError> {
        if dpb_slots > u32::from(max_dpb_slots) {
            return Err(ParseError::TooManyRefFrames {
                needed: dpb_slots,
                available: max_dpb_slots,
            });
        }
        self.slots = (0..dpb_slots).map(|_| None).collect();
        self.refs = [None; NUM_REF_FRAMES];
        Ok(())
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
        self.refs = [None; NUM_REF_FRAMES];
    }

    /// A free DPB slot for the picture about to be decoded.
    pub fn allocate(&self) -> Result<u8, ParseError> {
        self.slots
            .iter()
            .position(Option::is_none)
            .map(|slot| slot as u8)
            .ok_or(ParseError::DpbOverflow)
    }

    /// The DPB slot each of an inter frame's reference names resolves to.
    ///
    /// Every name must resolve: a stream referring to a slot no picture was
    /// decoded into can't be decoded (a seek landed mid-sequence).
    pub fn reference_name_slots(
        &self,
        ref_frame_idx: &[u8; REFS_PER_FRAME],
    ) -> Result<[Option<u8>; REFS_PER_FRAME], ParseError> {
        let mut names = [None; REFS_PER_FRAME];
        for (name, &index) in names.iter_mut().zip(ref_frame_idx) {
            let slot = self
                .refs
                .get(usize::from(index))
                .copied()
                .flatten()
                .ok_or(ParseError::NoReferencesAvailable)?;
            *name = Some(slot);
        }
        Ok(names)
    }

    /// The reference infos of the slots the names resolve to, one entry per
    /// distinct slot, sorted by slot.
    pub fn references(&self, names: &[Option<u8>; REFS_PER_FRAME]) -> Vec<ReferenceInfo> {
        let mut slots: Vec<u8> = names.iter().flatten().copied().collect();
        slots.sort_unstable();
        slots.dedup();
        slots
            .into_iter()
            .filter_map(|slot| {
                self.slots
                    .get(usize::from(slot))?
                    .as_ref()
                    .map(|picture| picture.reference.clone())
            })
            .collect()
    }

    /// Puts a decoded picture into its DPB slot and the reference names its
    /// `refresh_frame_flags` name, then frees the slots no name points at.
    ///
    /// Returns the ids of the pictures that left the buffer.
    pub fn update(
        &mut self,
        picture_id: u64,
        reference: ReferenceInfo,
        refresh_frame_flags: u8,
    ) -> Result<Vec<u64>, ParseError> {
        let slot = reference.slot;
        let entry = self
            .slots
            .get_mut(usize::from(slot))
            .ok_or(ParseError::DpbOverflow)?;
        *entry = Some(Picture {
            picture_id,
            reference,
        });

        for (name, target) in self.refs.iter_mut().enumerate() {
            if (refresh_frame_flags >> name) & 1 != 0 {
                *target = Some(slot);
            }
        }

        let mut evicted = Vec::new();
        for (index, entry) in self.slots.iter_mut().enumerate() {
            let referenced = self.refs.contains(&Some(index as u8));
            if referenced {
                continue;
            }
            if let Some(picture) = entry.take()
                && picture.picture_id != picture_id
            {
                evicted.push(picture.picture_id);
            }
        }
        Ok(evicted)
    }

    /// The picture a `show_existing_frame` refers to.
    pub fn picture_of_name(&self, name: u8) -> Result<u64, ParseError> {
        let slot = self
            .refs
            .get(usize::from(name))
            .copied()
            .flatten()
            .ok_or(ParseError::NoReferencesAvailable)?;
        self.slots
            .get(usize::from(slot))
            .and_then(|entry| entry.as_ref())
            .map(|picture| picture.picture_id)
            .ok_or(ParseError::NoReferencesAvailable)
    }

    /// The reference reload a key frame shown through `show_existing_frame`
    /// performs (spec 7.21): the picture takes over every reference name, and
    /// everything else leaves the buffer.
    ///
    /// Returns the shown picture and the ids of the pictures that left.
    pub fn reload_key_frame(&mut self, name: u8) -> Result<(u64, Vec<u64>), ParseError> {
        let slot = self
            .refs
            .get(usize::from(name))
            .copied()
            .flatten()
            .ok_or(ParseError::NoReferencesAvailable)?;
        let picture_id = self.picture_of_name(name)?;

        self.refs = [Some(slot); NUM_REF_FRAMES];

        let mut evicted = Vec::new();
        for (index, entry) in self.slots.iter_mut().enumerate() {
            if index == usize::from(slot) {
                continue;
            }
            if let Some(picture) = entry.take() {
                evicted.push(picture.picture_id);
            }
        }
        Ok((picture_id, evicted))
    }

    /// Every picture the buffer holds.
    #[cfg(test)]
    pub fn picture_ids(&self) -> Vec<u64> {
        self.slots
            .iter()
            .flatten()
            .map(|picture| picture.picture_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Dpb, NUM_REF_FRAMES, REFS_PER_FRAME};
    use crate::vulkan::av1::ops::ReferenceInfo;

    fn reference(slot: u8, order_hint: u8) -> ReferenceInfo {
        ReferenceInfo {
            slot,
            frame_type: 0,
            order_hint,
            saved_order_hints: [0; NUM_REF_FRAMES],
            ref_frame_sign_bias: 0,
            disable_frame_end_update_cdf: false,
            segmentation_enabled: false,
        }
    }

    /// A key frame refreshes every reference name, so one slot holds the whole
    /// buffer and the following pictures find free slots beside it.
    #[test]
    fn key_frame_occupies_one_slot() {
        let mut dpb = Dpb::new();
        dpb.configure(9, 9).unwrap();

        let slot = dpb.allocate().unwrap();
        assert_eq!(slot, 0);
        assert!(dpb.update(1, reference(slot, 0), 0xff).unwrap().is_empty());

        assert_eq!(dpb.allocate().unwrap(), 1);
        assert_eq!(dpb.picture_ids(), vec![1]);
    }

    /// A picture stays in the buffer while any reference name points at it, and
    /// leaves once the last name is overwritten.
    #[test]
    fn pictures_leave_with_their_last_name() {
        let mut dpb = Dpb::new();
        dpb.configure(9, 9).unwrap();

        let key = dpb.allocate().unwrap();
        dpb.update(1, reference(key, 0), 0xff).unwrap();

        // Refreshes names 0 and 1 only, the key frame keeps the other six.
        let inter = dpb.allocate().unwrap();
        assert!(dpb.update(2, reference(inter, 1), 0b11).unwrap().is_empty());

        // Takes over the remaining names, the key frame's slot frees up.
        let last = dpb.allocate().unwrap();
        assert_eq!(dpb.update(3, reference(last, 2), 0xff).unwrap(), vec![1, 2]);
        assert_eq!(dpb.picture_ids(), vec![3]);
    }

    /// A picture refreshing nothing frees its slot right away: nothing can ever
    /// reference it, and it is never held back for a later show.
    #[test]
    fn picture_refreshing_nothing_frees_its_slot() {
        let mut dpb = Dpb::new();
        dpb.configure(9, 9).unwrap();

        let key = dpb.allocate().unwrap();
        dpb.update(1, reference(key, 0), 0xff).unwrap();

        let slot = dpb.allocate().unwrap();
        assert!(dpb.update(2, reference(slot, 1), 0).unwrap().is_empty());
        assert_eq!(dpb.allocate().unwrap(), slot);
    }

    /// Every reference name of an inter frame must resolve to a decoded picture.
    #[test]
    fn unresolved_reference_names_are_an_error() {
        let dpb = Dpb::new();
        assert!(dpb.reference_name_slots(&[0; REFS_PER_FRAME]).is_err());
    }
}
