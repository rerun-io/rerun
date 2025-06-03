/// In Annex-B before every NAL unit is a nal start code.
///
/// Can also be 3 bytes of 0x00, see [`NAL_START_CODE_SHORT`].
///
/// This is used in Annex-B byte stream formats such as h264 files.
/// Packet transform systems (RTP) may omit these.
pub const NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

/// See [`NAL_START_CODE`].
pub const NAL_START_CODE_SHORT: &[u8] = &[0x00, 0x00, 0x01];

/// Possible values for `nal_unit_type` field in `nal_unit`.
///
/// Encodes to 5 bits.
/// Via:
/// * <https://docs.rs/less-avc/0.1.5/src/less_avc/nal_unit.rs.html#232/>
/// * <https://github.com/FFmpeg/FFmpeg/blob/87068b9600daa522e3f45b5501ecd487a3c0be57/libavcodec/h264.h#L33>
#[derive(PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum NalUnitType {
    /// Unspecified
    Unspecified = 0,

    /// Coded slice of a non-IDR picture
    CodedSliceOfANonIDRPicture = 1,

    /// Coded slice data partition A
    CodedSliceDataPartitionA = 2,

    /// Coded slice data partition B
    CodedSliceDataPartitionB = 3,

    /// Coded slice data partition C
    CodedSliceDataPartitionC = 4,

    /// Coded slice of an IDR picture
    CodedSliceOfAnIDRPicture = 5,

    /// Supplemental enhancement information (SEI)
    SupplementalEnhancementInformation = 6,

    /// Sequence parameter set
    SequenceParameterSet = 7,

    /// Picture parameter set
    PictureParameterSet = 8,

    /// Signals the end of a NAL unit.
    AccessUnitDelimiter = 9,

    EndSequence = 10,
    EndStream = 11,
    FillerData = 12,
    SequenceParameterSetExt = 13,

    /// Header type not listed here.
    Other,
}

/// Header of the "Network Abstraction Layer" unit that is used by H.264/AVC & H.265/HEVC.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NalHeader(pub u8);

impl NalHeader {
    #[allow(dead_code)] // May be unused if `ffmpeg` decoder isn't used.
    pub const fn new(unit_type: NalUnitType, ref_idc: u8) -> Self {
        Self((unit_type as u8) | (ref_idc << 5))
    }

    pub fn unit_type(self) -> NalUnitType {
        match self.0 & 0b11111 {
            0 => NalUnitType::Unspecified,
            1 => NalUnitType::CodedSliceOfANonIDRPicture,
            2 => NalUnitType::CodedSliceDataPartitionA,
            3 => NalUnitType::CodedSliceDataPartitionB,
            4 => NalUnitType::CodedSliceDataPartitionC,
            5 => NalUnitType::CodedSliceOfAnIDRPicture,
            6 => NalUnitType::SupplementalEnhancementInformation,
            7 => NalUnitType::SequenceParameterSet,
            8 => NalUnitType::PictureParameterSet,
            9 => NalUnitType::AccessUnitDelimiter,
            10 => NalUnitType::EndSequence,
            11 => NalUnitType::EndStream,
            12 => NalUnitType::FillerData,
            13 => NalUnitType::SequenceParameterSetExt,
            _ => NalUnitType::Other,
        }
    }

    /// Ref idc is a value from 0-3 that tells us how "important" the frame/sample is.
    #[allow(dead_code)]
    pub fn ref_idc(self) -> u8 {
        (self.0 >> 5) & 0b11
    }
}
