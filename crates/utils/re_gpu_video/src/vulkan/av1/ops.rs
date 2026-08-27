//! The `DecodeOp` IR crossing from the safe parser to the GPU backend.
//!
//! Everything here is plain data: no GPU handles, no unsafe code, no `Rc` from
//! the `cros-codecs` parser. One temporal unit pushed into the [`super::Parser`]
//! yields a sequence of ops that the backend executes in order.

use std::ops::Range;

use cros_codecs::codec::av1::parser::FrameHeaderObu;

use crate::vulkan::codec::FrameOutput;

/// One instruction for the GPU backend.
pub enum DecodeOp {
    /// The sequence header changed.
    ///
    /// The backend recreates its session parameters from
    /// [`super::Parser::std_sequence_header`], and the session itself when the
    /// coded size changed.
    ParametersChanged,

    /// Decode one picture.
    DecodeFrame(DecodeInfo),

    /// `show_existing_frame`: output a picture decoded earlier and held back.
    ShowExisting(ShowExisting),
}

/// Everything the backend needs to decode one picture.
pub struct DecodeInfo {
    /// Byte ranges of the picture's OBUs in the pushed temporal unit.
    ///
    /// Uploaded verbatim and in this order, headers and size fields included.
    /// Only valid for the data of the `push_access_unit` call that produced this op.
    pub obu_ranges: Vec<Range<usize>>,

    /// Index into [`Self::obu_ranges`] of the OBU carrying the uncompressed
    /// frame header.
    pub frame_header_obu: usize,

    /// The picture's tiles, in decoding order.
    pub tiles: Vec<TileRef>,

    /// Names this picture while it is held back for a later [`ShowExisting`].
    pub picture_id: u64,

    /// The DPB slot this picture decodes into. Every AV1 picture takes one:
    /// even a picture that refreshes no reference slot is decoded into the DPB.
    pub setup_slot: u8,

    /// The reference pictures this picture may use, one DPB slot each.
    pub references: Vec<ReferenceInfo>,

    /// The DPB slot per AV1 reference name (`LAST_FRAME` … `ALTREF_FRAME`),
    /// all `None` for intra pictures.
    pub reference_name_slots: [Option<u8>; 7],

    pub is_key_frame: bool,

    /// `OrderHint` of this picture, the stream's own presentation order key.
    /// AV1 emits pictures in presentation order, so this is informational.
    pub order_hint: i32,

    pub output: FrameOutput,

    /// Pictures that left the reference slots with this one and can never be
    /// shown again. Their held-back output is dropped.
    pub evicted_pictures: Vec<u64>,

    /// The parsed frame header, converted to the Vulkan picture info when the
    /// decode is recorded.
    pub header: Box<FrameHeaderObu>,
}

/// One tile of a picture within the uploaded OBUs.
#[derive(Debug, Clone)]
pub struct TileRef {
    /// Index into [`DecodeInfo::obu_ranges`] of the OBU holding the tile data.
    pub obu: usize,

    /// Byte offset of the tile data within that OBU.
    pub offset: u32,

    /// Byte size of the tile data.
    pub size: u32,
}

/// One active reference picture in the DPB.
#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    pub slot: u8,

    /// `StdVideoAV1FrameType` of the reference picture.
    pub frame_type: u8,

    /// `OrderHint` of the reference picture.
    pub order_hint: u8,

    /// `SavedOrderHints`: the `OrderHints` the reference picture was decoded with.
    pub saved_order_hints: [u8; 8],

    /// `RefFrameSignBias` of the reference picture, one bit per reference name.
    pub ref_frame_sign_bias: u8,

    pub disable_frame_end_update_cdf: bool,

    pub segmentation_enabled: bool,
}

/// Output a picture that was decoded earlier and held back.
#[derive(Debug)]
pub struct ShowExisting {
    /// The [`DecodeInfo::picture_id`] of the picture to output.
    pub picture_id: u64,

    /// See [`DecodeInfo::evicted_pictures`]. Showing a key frame again reloads
    /// the whole reference state, which evicts everything else.
    pub evicted_pictures: Vec<u64>,
}

impl std::fmt::Display for DecodeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParametersChanged => write!(f, "ParametersChanged"),
            Self::DecodeFrame(info) => info.fmt(f),
            Self::ShowExisting(show) => {
                write!(f, "ShowExisting {{ picture {} }}", show.picture_id)
            }
        }
    }
}

impl std::fmt::Display for DecodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DecodeFrame {{ picture {id}, order hint {hint}",
            id = self.picture_id,
            hint = self.order_hint,
        )?;
        if self.is_key_frame {
            write!(f, ", key")?;
        }
        write!(f, ", slot {slot}", slot = self.setup_slot)?;
        match self.output {
            FrameOutput::Show => {}
            FrameOutput::HoldBack => write!(f, ", held back")?,
            FrameOutput::None => write!(f, ", not shown")?,
        }
        if self.tiles.len() != 1 {
            write!(f, ", tiles: {}", self.tiles.len())?;
        }
        if !self.references.is_empty() {
            write!(f, ", refs: [")?;
            for (index, reference) in self.references.iter().enumerate() {
                if index != 0 {
                    write!(f, ", ")?;
                }
                write!(
                    f,
                    "slot {slot} (order hint {hint})",
                    slot = reference.slot,
                    hint = reference.order_hint,
                )?;
            }
            write!(f, "]")?;
        }
        let names: Vec<String> = self
            .reference_name_slots
            .iter()
            .map(|slot| slot.map_or_else(|| "-".to_owned(), |slot| slot.to_string()))
            .collect();
        if self.reference_name_slots.iter().any(Option::is_some) {
            write!(f, ", names: [{}]", names.join(", "))?;
        }
        if !self.evicted_pictures.is_empty() {
            write!(f, ", evicted: {:?}", self.evicted_pictures)?;
        }
        write!(f, " }}")
    }
}
