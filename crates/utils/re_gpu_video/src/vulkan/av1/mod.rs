//! Safe AV1 bitstream parser producing the plain-data [`DecodeOp`] IR for the
//! Vulkan backend.
//!
//! All "spec X.Y.Z" references are to the AV1 Bitstream & Decoding Process
//! Specification, version 1.0.0 with Errata 1.
//!
//! The syntax-level parsing (sequence headers, frame headers, tile groups)
//! comes from the `cros-codecs` crate, which `re_video` already uses for stream
//! inspection. What lives here is the semantic layer on top: OBU framing, the
//! reference slot bookkeeping of spec 7.20 ([`refs`]), the output decisions AV1
//! makes with `show_frame` and `show_existing_frame`, and the conversion of the
//! headers into the Vulkan `StdVideo` structs ([`std_params`], the only file
//! here touching `ash`).
//!
//! Unlike H.264 and H.265 there is no picture order count to derive and no
//! reordering: AV1 states which picture to output with every frame it codes, so
//! pictures leave the decoder in presentation order already.

mod ops;
mod parse;
mod refs;
mod std_params;

#[cfg(test)]
mod tests;

use ash::vk::native as std_video;
use cros_codecs::codec::av1::parser::{
    self as av1, FrameHeaderObu, FrameType, ParsedObu, SequenceHeaderObu,
};

use crate::vulkan::codec::FrameOutput;
use crate::{ColorProperties, MatrixCoefficients, ParseError};

pub use ops::{DecodeInfo, DecodeOp, ShowExisting, TileRef};
pub use std_params::{PictureStdParams, SequenceStdParams, reference_info, std_reference_info};

/// What the backend derives from the sequence header.
#[derive(Clone, Copy, Debug)]
pub struct SeqFacts {
    /// The largest coded width & height the sequence declares, what the session
    /// and its images are created for.
    pub coded_extent: [u32; 2],

    /// DPB slots the stream needs: the eight reference slots plus the current
    /// picture.
    pub dpb_slots: u32,

    /// Upper bound on the reference pictures one decode operation binds.
    pub max_ref_frames: u32,

    pub std_profile: std_video::StdVideoAV1Profile,

    /// The sequence may carry film grain parameters and the device can apply
    /// them, which the decode session must be created for.
    pub film_grain_present: bool,

    pub color: ColorProperties,
}

/// The `cros-codecs` parser together with everything derived from it.
///
/// `cros-codecs` shares the parsed sequence header through an `Rc`, so this is
/// not `Send` on its own. Every one of those `Rc`s lives inside this struct,
/// which travels to the decoder thread as one value and is only ever used from
/// there.
struct ParserState {
    parser: av1::Parser,

    /// The raw bytes of the sequence header OBU last seen, to skip the
    /// identical repeats encoders put in front of every key frame.
    seen_sequence_header: Option<Vec<u8>>,

    sequence_std: Option<SequenceStdParams>,
    facts: Option<SeqFacts>,
}

// SAFETY: The `Rc`s of the `cros-codecs` parser never leave this struct, and the
// std parameter structs own the allocations their pointers refer into.
#[expect(unsafe_code)]
unsafe impl Send for ParserState {}

/// Turns temporal units into [`DecodeOp`]s, one call per temporal unit.
///
/// Decoding must start at a key frame carrying its sequence header, which is
/// also what [`Self::reset`] returns to. Any error leaves the parser waiting for
/// the next one.
pub struct Parser {
    state: ParserState,
    dpb: refs::Dpb,
    pending: Option<PendingPicture>,

    /// Hardware DPB slot capacity.
    max_dpb_slots: u8,

    /// The device can apply film grain. Without it a stream that applies grain
    /// has to fall back to software decoding.
    film_grain_support: bool,

    /// The slot count the DPB is currently configured for.
    configured_dpb_slots: Option<u32>,

    /// Names decoded pictures so a later `show_existing_frame` can point at the
    /// output the backend held back.
    next_picture_id: u64,

    /// The next picture must be a key frame: start of stream, after
    /// [`Self::reset`], or after an error.
    awaiting_key_frame: bool,
}

/// The OBUs of one picture, collected until all its tiles arrived.
struct PendingPicture {
    header: FrameHeaderObu,

    /// The picture's OBUs in the pushed temporal unit, in order.
    obu_ranges: Vec<std::ops::Range<usize>>,

    /// Index into `obu_ranges` of the OBU carrying the uncompressed frame header.
    frame_header_obu: usize,

    tiles: Vec<TileRef>,

    /// `TileCols * TileRows` of the frame header: the picture is complete once
    /// this many tiles arrived.
    tiles_expected: usize,
}

impl Parser {
    /// `max_dpb_slots` is the device's DPB slot capacity, an upper bound on what
    /// a stream may need. `film_grain_support` is whether the device's decoder
    /// can apply film grain.
    pub fn new(max_dpb_slots: u8, film_grain_support: bool) -> Self {
        let mut parser = av1::Parser::default();
        parse::lock_to_low_overhead(&mut parser);

        Self {
            state: ParserState {
                parser,
                seen_sequence_header: None,
                sequence_std: None,
                facts: None,
            },
            dpb: refs::Dpb::new(),
            pending: None,
            max_dpb_slots,
            film_grain_support,
            configured_dpb_slots: None,
            next_picture_id: 0,
            awaiting_key_frame: true,
        }
    }

    /// Parses one temporal unit into the ops decoding it requires.
    ///
    /// The OBU byte ranges in the returned ops point into `data`.
    /// On error the temporal unit produces nothing and the parser waits for the
    /// next key frame.
    pub fn push_access_unit(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        let result = self.push_inner(data);
        if result.is_err() {
            self.pending = None;
            self.awaiting_key_frame = true;
        }
        result
    }

    /// Drops all picture state for a seek. The sequence header is kept: the next
    /// temporal unit must hold a key frame, which re-sends it anyway.
    pub fn reset(&mut self) {
        self.pending = None;
        self.dpb.clear();
        self.awaiting_key_frame = true;
    }

    /// AV1 codes the output of every picture, so nothing is ever held back for
    /// reordering.
    #[expect(clippy::unused_self)]
    pub fn reorder_delay(&self) -> usize {
        0
    }

    /// What the backend derives from the sequence header.
    pub fn seq_facts(&self) -> Result<SeqFacts, ParseError> {
        self.state.facts.ok_or(ParseError::MissingReference {
            what: "sequence header",
        })
    }

    /// The sequence header a session parameters object is built from.
    ///
    /// The returned struct borrows the allocations the parser owns, which stay
    /// in place until the next sequence header change.
    pub fn std_sequence_header(&self) -> Result<&std_video::StdVideoAV1SequenceHeader, ParseError> {
        self.state
            .sequence_std
            .as_ref()
            .map(SequenceStdParams::std)
            .ok_or(ParseError::MissingReference {
                what: "sequence header",
            })
    }

    fn push_inner(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        re_tracing::profile_function!();

        let mut ops = Vec::new();
        let mut sequence_changed = false;
        let mut pos = 0;

        while pos < data.len() {
            let Some((obu, position)) = parse::read_obu(&mut self.state.parser, data, &mut pos)?
            else {
                continue;
            };

            match obu.header.obu_type {
                av1::ObuType::SequenceHeader => {
                    let bytes = &data[position.range.clone()];
                    if self.state.seen_sequence_header.as_deref() != Some(bytes) {
                        self.parse_sequence_header(obu)?;
                        self.state.seen_sequence_header = Some(bytes.to_vec());
                        sequence_changed = true;
                    }
                }

                av1::ObuType::TemporalDelimiter => {
                    self.state
                        .parser
                        .parse_obu(obu)
                        .map_err(|err| ParseError::nal("temporal delimiter", err))?;
                }

                av1::ObuType::FrameHeader => {
                    // A frame header repeated while a picture is still
                    // collecting its tiles carries nothing new.
                    if self.pending.is_some() {
                        continue;
                    }
                    let ParsedObu::FrameHeader(header) = self
                        .state
                        .parser
                        .parse_obu(obu)
                        .map_err(|err| ParseError::nal("frame header", err))?
                    else {
                        return Err(ParseError::Invalid(
                            "frame header OBU parsed as another type",
                        ));
                    };
                    self.start_picture(header, position.range, &mut ops)?;
                }

                av1::ObuType::TileGroup => {
                    let Some(pending) = &self.pending else {
                        return Err(ParseError::IncompletePicture);
                    };
                    let tile_data_offset = position.payload_offset;
                    let obu_index = pending.obu_ranges.len();

                    let ParsedObu::TileGroup(tile_group) = self
                        .state
                        .parser
                        .parse_obu(obu)
                        .map_err(|err| ParseError::nal("tile group", err))?
                    else {
                        return Err(ParseError::Invalid("tile group OBU parsed as another type"));
                    };

                    let pending = self.pending.as_mut().expect("checked above");
                    pending.obu_ranges.push(position.range);
                    append_tiles(pending, obu_index, tile_data_offset, &tile_group.tiles);
                    self.finish_picture_if_complete(&mut ops)?;
                }

                av1::ObuType::Frame => {
                    if self.pending.is_some() {
                        return Err(ParseError::IncompletePicture);
                    }
                    let payload_offset = position.payload_offset;
                    let range = position.range;

                    let ParsedObu::Frame(frame) = self
                        .state
                        .parser
                        .parse_obu(obu)
                        .map_err(|err| ParseError::nal("frame", err))?
                    else {
                        return Err(ParseError::Invalid("frame OBU parsed as another type"));
                    };

                    // The tile group of a frame OBU follows the uncompressed
                    // header within the same payload.
                    let tile_data_offset = payload_offset + frame.header.header_bytes;
                    self.start_picture(frame.header, range, &mut ops)?;
                    let pending = self
                        .pending
                        .as_mut()
                        .ok_or(ParseError::Invalid("frame OBU shows an existing frame"))?;
                    append_tiles(pending, 0, tile_data_offset, &frame.tile_group.tiles);
                    self.finish_picture_if_complete(&mut ops)?;
                }

                // Nothing decode-relevant in these.
                av1::ObuType::Metadata
                | av1::ObuType::Padding
                | av1::ObuType::RedundantFrameHeader
                | av1::ObuType::TileList => {}

                av1::ObuType::Reserved
                | av1::ObuType::Reserved2
                | av1::ObuType::Reserved3
                | av1::ObuType::Reserved4
                | av1::ObuType::Reserved5
                | av1::ObuType::Reserved6
                | av1::ObuType::Reserved7 => {
                    return Err(ParseError::Unsupported("reserved OBU type"));
                }
            }
        }

        if self.pending.is_some() {
            return Err(ParseError::IncompletePicture);
        }

        if sequence_changed {
            // Ahead of the decode ops of this temporal unit: they need the new
            // session parameters.
            ops.insert(0, DecodeOp::ParametersChanged);
        }

        Ok(ops)
    }

    fn parse_sequence_header(&mut self, obu: av1::Obu<'_>) -> Result<(), ParseError> {
        let ParsedObu::SequenceHeader(sequence) = self
            .state
            .parser
            .parse_obu(obu)
            .map_err(|err| ParseError::nal("sequence header", err))?
        else {
            return Err(ParseError::Invalid(
                "sequence header OBU parsed as another type",
            ));
        };

        parse::check_sequence_header(&sequence)?;
        self.state.facts = Some(seq_facts(&sequence, self.film_grain_support));
        self.state.sequence_std = Some(SequenceStdParams::build(&sequence));
        Ok(())
    }

    /// Starts collecting a picture, or emits the op that shows an existing one.
    fn start_picture(
        &mut self,
        header: FrameHeaderObu,
        range: std::ops::Range<usize>,
        ops: &mut Vec<DecodeOp>,
    ) -> Result<(), ParseError> {
        if header.show_existing_frame {
            let name = header.frame_to_show_map_idx;
            let (picture_id, evicted_pictures) = if header.frame_type == FrameType::KeyFrame {
                self.dpb.reload_key_frame(name)?
            } else {
                (self.dpb.picture_of_name(name)?, Vec::new())
            };
            self.reference_frame_update(&header)?;
            ops.push(DecodeOp::ShowExisting(ShowExisting {
                picture_id,
                evicted_pictures,
            }));
            return Ok(());
        }

        let is_key_frame = header.frame_type == FrameType::KeyFrame;
        if self.awaiting_key_frame {
            if !is_key_frame {
                return Err(ParseError::ExpectedRandomAccessPoint);
            }
            self.awaiting_key_frame = false;
        }

        let tiles_expected = (header.tile_info.tile_cols as usize)
            .checked_mul(header.tile_info.tile_rows as usize)
            .filter(|&tiles| tiles > 0)
            .ok_or(ParseError::Invalid("frame header codes no tiles"))?;

        self.pending = Some(PendingPicture {
            header,
            obu_ranges: vec![range],
            frame_header_obu: 0,
            tiles: Vec::new(),
            tiles_expected,
        });
        Ok(())
    }

    fn finish_picture_if_complete(&mut self, ops: &mut Vec<DecodeOp>) -> Result<(), ParseError> {
        let complete = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.tiles.len() >= pending.tiles_expected);
        if !complete {
            return Ok(());
        }
        let picture = self.pending.take().expect("checked above");
        let info = self.finalize_picture(picture)?;
        ops.push(DecodeOp::DecodeFrame(info));
        Ok(())
    }

    /// Runs the reference slot bookkeeping on an assembled picture and decides
    /// what becomes of its output.
    fn finalize_picture(&mut self, picture: PendingPicture) -> Result<DecodeInfo, ParseError> {
        let header = picture.header;
        let facts = self.seq_facts()?;

        if header.use_superres {
            return Err(ParseError::Unsupported("super-resolution frames"));
        }
        if header.film_grain_params.apply_grain && !self.film_grain_support {
            return Err(ParseError::FilmGrainUnsupported);
        }
        if [header.frame_width, header.frame_height] != facts.coded_extent {
            return Err(ParseError::Unsupported(
                "frames smaller than the sequence maximum (reference scaling)",
            ));
        }

        if self.configured_dpb_slots != Some(facts.dpb_slots) {
            self.dpb.configure(facts.dpb_slots, self.max_dpb_slots)?;
            self.configured_dpb_slots = Some(facts.dpb_slots);
        }

        // Intra pictures bind no references, and the reference names of a key
        // frame are stale by the time it is decoded.
        let reference_name_slots = if header.frame_is_intra {
            [None; refs::REFS_PER_FRAME]
        } else {
            self.dpb.reference_name_slots(&header.ref_frame_idx)?
        };
        let references = self.dpb.references(&reference_name_slots);

        let setup_slot = self.dpb.allocate()?;
        let picture_id = self.next_picture_id;
        self.next_picture_id += 1;

        let refresh_frame_flags = header.refresh_frame_flags as u8;
        let evicted_pictures = self.dpb.update(
            picture_id,
            std_params::reference_info(&header, setup_slot),
            refresh_frame_flags,
        )?;

        // A picture nothing can reference is unreachable for a later
        // `show_existing_frame`, so holding its output back would be pointless.
        let output = if header.show_frame {
            FrameOutput::Show
        } else if header.showable_frame && refresh_frame_flags != 0 {
            FrameOutput::HoldBack
        } else {
            FrameOutput::None
        };

        self.reference_frame_update(&header)?;

        Ok(DecodeInfo {
            obu_ranges: picture.obu_ranges,
            frame_header_obu: picture.frame_header_obu,
            tiles: picture.tiles,
            picture_id,
            setup_slot,
            references,
            reference_name_slots,
            is_key_frame: header.frame_type == FrameType::KeyFrame,
            order_hint: header.order_hint.cast_signed(),
            output,
            evicted_pictures,
            header: Box::new(header),
        })
    }
}

impl Parser {
    /// The reference frame update process of spec 7.20 on the parser's own
    /// state, which the frame headers that follow are parsed against.
    ///
    /// Runs right after a picture is assembled, which is where the spec puts it:
    /// once the picture is decoded.
    fn reference_frame_update(&mut self, header: &FrameHeaderObu) -> Result<(), ParseError> {
        self.state
            .parser
            .ref_frame_update(header)
            .map_err(|err| ParseError::nal("reference frame update", err))
    }
}

/// Records where the tiles of one tile group sit in the uploaded OBU.
fn append_tiles(
    pending: &mut PendingPicture,
    obu_index: usize,
    tile_data_offset: usize,
    tiles: &[av1::Tile],
) {
    pending.tiles.extend(tiles.iter().map(|tile| TileRef {
        obu: obu_index,
        offset: tile_data_offset as u32 + tile.tile_offset,
        size: tile.tile_size,
    }));
}

/// What the backend needs from a sequence header, extracted before its borrow ends.
fn seq_facts(sequence: &SequenceHeaderObu, film_grain_support: bool) -> SeqFacts {
    SeqFacts {
        coded_extent: [
            u32::from(sequence.max_frame_width_minus_1) + 1,
            u32::from(sequence.max_frame_height_minus_1) + 1,
        ],
        // The eight reference slots the bitstream addresses, plus the picture
        // being decoded.
        dpb_slots: refs::NUM_REF_FRAMES as u32 + 1,
        max_ref_frames: refs::REFS_PER_FRAME as u32,
        std_profile: sequence.seq_profile as std_video::StdVideoAV1Profile,
        film_grain_present: sequence.film_grain_params_present && film_grain_support,
        color: color_properties(sequence),
    }
}

/// Color properties from the sequence header's color config, absent fields left
/// at their defaults.
fn color_properties(sequence: &SequenceHeaderObu) -> ColorProperties {
    let color = &sequence.color_config;
    ColorProperties {
        full_range: color.color_range,
        matrix_coefficients: if color.color_description_present_flag {
            match color.matrix_coefficients {
                av1::MatrixCoefficients::Bt709 => MatrixCoefficients::Bt709,
                av1::MatrixCoefficients::Bt470bg | av1::MatrixCoefficients::Bt601 => {
                    MatrixCoefficients::Bt601
                }
                _ => MatrixCoefficients::Unspecified,
            }
        } else {
            MatrixCoefficients::Unspecified
        },
    }
}
