use re_build_info::CrateVersion;
use re_chunk::ChunkError;

pub type CodecResult<T> = Result<T, CodecError>;

/// Possible errors when encoding and decoding RRD data.
///
/// Encoding *never* involves any IO: the only way you can get this error is due to invalid data in
/// the stream.
//
// TODO(cmc): maybe we should split this into read and write errors.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Invalid encoding options: {0}")]
    InvalidOptions(#[from] OptionsError),

    #[error(
        "Data from Rerun version {file}, which is incompatible with the local Rerun version {local}"
    )]
    IncompatibleRerunVersion {
        file: Box<CrateVersion>,
        local: Box<CrateVersion>,
    },

    #[error("{0}")]
    NotAnRrd(NotAnRrdError),

    #[error("Data was from an old, incompatible Rerun version")]
    OldRrdVersion,

    /// Something went wrong when attempting to decode any kind of RRD frame.
    ///
    /// There are 3 kinds of RRD frames:
    /// * [`crate::StreamHeader`]
    /// * [`crate::MessageHeader`]
    /// * [`crate::StreamFooter`]
    #[error("Failed to decode frame: {0}")]
    FrameDecoding(String),

    #[error("CRC check failed: expected {expected:08x} but got {got:08x}")]
    CrcMismatch { expected: u32, got: u32 },

    #[error("Arrow IPC deserialization error: {0}")]
    ArrowDeserialization(::arrow::error::ArrowError),

    #[error("Arrow IPC serialization error: {0}")]
    ArrowSerialization(::arrow::error::ArrowError),

    #[error("Protobuf encoding error: {0}")]
    ProtobufEncode(#[from] re_protos::external::prost::EncodeError),

    #[error("Protobuf error: {0}")]
    ProtobufDecode(#[from] re_protos::external::prost::DecodeError),

    #[error("Could not convert type from protobuf: {0}")]
    TypeConversion(Box<re_protos::TypeConversionError>),

    #[error("Invalid chunk: {0}")]
    Chunk(Box<ChunkError>),

    /// This is returned when `ArrowMsg` or `BlueprintActivationCommand` are received with a legacy
    /// store id (missing the application id) before the corresponding `SetStoreInfo` message. In
    /// that case, the best effort is to recover by dropping such message with a warning.
    #[error("Message with an unknown application id was received")]
    StoreIdMissingApplicationId {
        store_kind: re_log_types::StoreKind,
        recording_id: re_log_types::RecordingId,
    },

    #[error("Unsupported encoding, expected Arrow IPC")]
    UnsupportedEncoding,

    #[error("Missing record batch")]
    MissingRecordBatch,

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::DecompressError),

    #[error("Sorbet error: {0}")]
    Sorbet(#[from] re_sorbet::SorbetError),

    #[error("Integer overflow: {0}")]
    Overflow(#[from] std::num::TryFromIntError),
}

const _: () = assert!(
    std::mem::size_of::<CodecError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl From<re_protos::TypeConversionError> for CodecError {
    fn from(value: re_protos::TypeConversionError) -> Self {
        Self::TypeConversion(Box::new(value))
    }
}

impl From<ChunkError> for CodecError {
    fn from(value: ChunkError) -> Self {
        Self::Chunk(Box::new(value))
    }
}

impl From<re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError> for CodecError {
    fn from(value: re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError) -> Self {
        Self::StoreIdMissingApplicationId {
            store_kind: value.store_kind,
            recording_id: value.recording_id,
        }
    }
}

/// When the file does not have the expected .rrd [FourCC](https://en.wikipedia.org/wiki/FourCC) header.
#[derive(Debug)]
pub struct NotAnRrdError {
    pub expected_fourcc: [u8; 4],
    pub actual_fourcc: [u8; 4],
}

impl std::fmt::Display for NotAnRrdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn format_fourcc(fourcc: [u8; 4]) -> String {
            String::from_utf8(fourcc.to_vec()).unwrap_or_else(|_err| {
                // Show as hex instead
                format!(
                    "0x{:02X}{:02X}{:02X}{:02X}",
                    fourcc[0], fourcc[1], fourcc[2], fourcc[3]
                )
            })
        }

        write!(
            f,
            "Not an RRD file: expected FourCC header {:?}, got {:?}",
            format_fourcc(self.expected_fourcc),
            format_fourcc(self.actual_fourcc),
        )
    }
}

/// On failure to decode [`crate::rrd::EncodingOptions`]
#[derive(thiserror::Error, Debug)]
pub enum OptionsError {
    #[error("Reserved bytes not zero")]
    UnknownReservedBytes,

    #[error("Unknown compression: {0}")]
    UnknownCompression(u8),

    // TODO(jan): Remove this at some point, realistically 1-2 releases after 0.23
    #[error(
        "You are trying to load an old .rrd file that's not supported by this version of Rerun."
    )]
    RemovedMsgPackSerializer,

    #[error("Unknown serializer: {0}")]
    UnknownSerializer(u8),
}
