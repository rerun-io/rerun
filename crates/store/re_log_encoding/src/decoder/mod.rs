//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

#[cfg(feature = "decoder")]
pub mod streaming;

use re_build_info::CrateVersion;

use crate::{EncodingOptions, FileHeader, Serializer};

// ----------------------------------------------------------------------------

fn warn_on_version_mismatch(encoded_version: [u8; 4]) -> Result<(), DecodeError> {
    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    if encoded_version.major == 0 && encoded_version.minor < 23 {
        // We broke compatibility for 0.23 for (hopefully) the last time.
        Err(DecodeError::IncompatibleRerunVersion {
            file: Box::new(encoded_version),
            local: Box::new(CrateVersion::LOCAL),
        })
    } else if encoded_version <= CrateVersion::LOCAL {
        // Loading old files should be fine, and if it is not, the chunk migration in re_sorbet should already log a warning.
        Ok(())
    } else {
        re_log::warn_once!(
            "Found data stream with Rerun version {encoded_version} which is newer than the local Rerun version ({}). This file may contain data that is not compatible with this version of Rerun. Consider updating Rerun.",
            CrateVersion::LOCAL
        );
        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// When the file does not have the expected .rrd [FourCC](https://en.wikipedia.org/wiki/FourCC) header
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

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("{0}")]
    NotAnRrd(NotAnRrdError),

    #[error("Data was from an old, incompatible Rerun version")]
    OldRrdVersion,

    #[error(
        "Data from Rerun version {file}, which is incompatible with the local Rerun version {local}"
    )]
    IncompatibleRerunVersion {
        file: Box<CrateVersion>,
        local: Box<CrateVersion>,
    },

    /// This is returned when `ArrowMsg` or `BlueprintActivationCommand` are received with a legacy
    /// store id (missing the application id) before the corresponding `SetStoreInfo` message. In
    /// that case, the best effort is to recover by dropping such message with a warning.
    #[error("Message with an unknown application id was received.")]
    StoreIdMissingApplicationId {
        store_kind: re_log_types::StoreKind,
        recording_id: re_log_types::RecordingId,
    },

    #[error("Failed to decode the options: {0}")]
    Options(#[from] crate::OptionsError),

    #[error("Failed to read: {0}")]
    Read(#[from] std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::DecompressError),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] re_protos::external::prost::DecodeError),

    #[error("Could not convert type from protobuf: {0}")]
    TypeConversion(Box<re_protos::TypeConversionError>),

    #[error("Sorbet error: {0}")]
    SorbetError(#[from] re_sorbet::SorbetError),

    #[error("Failed to read chunk: {0}")]
    Chunk(Box<re_chunk::ChunkError>),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Codec error: {0}")]
    Codec(#[from] crate::codec::CodecError),
}

const _: () = assert!(
    std::mem::size_of::<DecodeError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl From<re_protos::TypeConversionError> for DecodeError {
    fn from(value: re_protos::TypeConversionError) -> Self {
        Self::TypeConversion(Box::new(value))
    }
}

impl From<re_chunk::ChunkError> for DecodeError {
    fn from(value: re_chunk::ChunkError) -> Self {
        Self::Chunk(Box::new(value))
    }
}

impl From<re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError> for DecodeError {
    fn from(value: re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError) -> Self {
        Self::StoreIdMissingApplicationId {
            store_kind: value.store_kind,
            recording_id: value.recording_id,
        }
    }
}

// ----------------------------------------------------------------------------

/// Read encoding options from the beginning of the stream.
pub fn read_options(
    reader: &mut impl std::io::Read,
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];
    reader.read_exact(&mut data).map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

/// Read encoding options from the beginning of the stream asynchronously.
pub async fn read_options_async(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];

    use tokio::io::AsyncReadExt as _;
    reader
        .read_exact(&mut data)
        .await
        .map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

pub fn options_from_bytes(bytes: &[u8]) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut read = std::io::Cursor::new(bytes);

    let FileHeader {
        fourcc: _, // Checked in FileHeader::decode
        version,
        options,
    } = FileHeader::decode(&mut read)?;

    warn_on_version_mismatch(version)?;

    match options.serializer {
        Serializer::Protobuf => {}
    }

    Ok((CrateVersion::from_bytes(version), options))
}
