//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use crate::codec;
use crate::codec::file::{self, encoder};
use crate::FileHeader;
use crate::LegacyMessageHeader;
use crate::Serializer;
use crate::{Compression, EncodingOptions};
use re_build_info::CrateVersion;
use re_chunk::{ChunkError, ChunkResult};
use re_log_types::LogMsg;

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Failed to write: {0}")]
    Write(#[from] std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::CompressError),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] re_protos::external::prost::EncodeError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("{0}")]
    Codec(#[from] codec::CodecError),

    #[error("Chunk error: {0}")]
    Chunk(#[from] ChunkError),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,

    #[error("Cannot encode with legacy MsgPack path")]
    CannotEncodeWithMsgPack,
}

// ----------------------------------------------------------------------------

pub fn encode_to_bytes<'a>(
    version: CrateVersion,
    options: EncodingOptions,
    msgs: impl IntoIterator<Item = &'a LogMsg>,
) -> Result<Vec<u8>, EncodeError> {
    let mut bytes: Vec<u8> = vec![];
    {
        let mut encoder = Encoder::new(version, options, std::io::Cursor::new(&mut bytes))?;
        for msg in msgs {
            encoder.append(msg)?;
        }
    }
    Ok(bytes)
}

// ----------------------------------------------------------------------------

/// An [`Encoder`] that properly closes the stream on drop.
///
/// When dropped, it will automatically insert an end-of-stream marker, if that wasn't already done manually.
pub struct DroppableEncoder<W: std::io::Write> {
    encoder: Encoder<W>,

    /// Tracks whether the end-of-stream marker has been written out already.
    is_finished: bool,
}

impl<W: std::io::Write> DroppableEncoder<W> {
    #[inline]
    pub fn new(
        version: CrateVersion,
        options: EncodingOptions,
        write: W,
    ) -> Result<Self, EncodeError> {
        Ok(Self {
            encoder: Encoder::new(version, options, write)?,
            is_finished: false,
        })
    }

    /// Returns the size in bytes of the encoded data.
    #[inline]
    pub fn append(&mut self, message: &LogMsg) -> Result<u64, EncodeError> {
        self.encoder.append(message)
    }

    #[inline]
    pub fn finish(&mut self) -> Result<(), EncodeError> {
        if !self.is_finished {
            self.encoder.finish()?;
        }

        self.is_finished = true;

        Ok(())
    }

    #[inline]
    pub fn flush_blocking(&mut self) -> std::io::Result<()> {
        self.encoder.flush_blocking()
    }
}

impl<W: std::io::Write> std::ops::Drop for DroppableEncoder<W> {
    fn drop(&mut self) {
        if !self.is_finished {
            if let Err(err) = self.finish() {
                re_log::warn!("encoder couldn't be finished: {err}");
            }
        }
    }
}

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
///
/// Prefer [`DroppableEncoder`] if possible, make sure to call [`Encoder::finish`] when appropriate
/// otherwise.
pub struct Encoder<W: std::io::Write> {
    serializer: Serializer,
    compression: Compression,
    write: W,
    scratch: Vec<u8>,
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(
        version: CrateVersion,
        options: EncodingOptions,
        mut write: W,
    ) -> Result<Self, EncodeError> {
        FileHeader {
            magic: *crate::RRD_HEADER,
            version: version.to_bytes(),
            options,
        }
        .encode(&mut write)?;

        Ok(Self {
            serializer: options.serializer,
            compression: options.compression,
            write,
            scratch: Vec::new(),
        })
    }

    /// Returns the size in bytes of the encoded data.
    pub fn append(&mut self, message: &LogMsg) -> Result<u64, EncodeError> {
        re_tracing::profile_function!();

        self.scratch.clear();
        match self.serializer {
            Serializer::Protobuf => {
                encoder::encode(&mut self.scratch, message, self.compression)?;

                self.write
                    .write_all(&self.scratch)
                    .map(|_| self.scratch.len() as _)
                    .map_err(EncodeError::Write)
            }
            Serializer::MsgPack => Err(EncodeError::CannotEncodeWithMsgPack),
        }
    }

    // NOTE: This cannot be done in a `Drop` implementation because of `Self::into_inner` which
    // does a partial move.
    #[inline]
    pub fn finish(&mut self) -> Result<(), EncodeError> {
        match self.serializer {
            Serializer::MsgPack => {
                return Err(EncodeError::CannotEncodeWithMsgPack);
            }
            Serializer::Protobuf => {
                file::MessageHeader {
                    kind: file::MessageKind::End,
                    len: 0,
                }
                .encode(&mut self.write)?;
            }
        }
        Ok(())
    }

    #[inline]
    pub fn flush_blocking(&mut self) -> std::io::Result<()> {
        self.write.flush()
    }

    #[inline]
    pub fn into_inner(self) -> W {
        self.write
    }
}

/// Returns the size in bytes of the encoded data.
pub fn encode(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
    write: &mut impl std::io::Write,
) -> Result<u64, EncodeError> {
    re_tracing::profile_function!();
    let mut encoder = DroppableEncoder::new(version, options, write)?;
    let mut size_bytes = 0;
    for message in messages {
        size_bytes += encoder.append(&message?)?;
    }
    Ok(size_bytes)
}

/// Returns the size in bytes of the encoded data.
pub fn encode_ref<'a>(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<&'a LogMsg>>,
    write: &mut impl std::io::Write,
) -> Result<u64, EncodeError> {
    re_tracing::profile_function!();
    let mut encoder = DroppableEncoder::new(version, options, write)?;
    let mut size_bytes = 0;
    for message in messages {
        size_bytes += encoder.append(message?)?;
    }
    Ok(size_bytes)
}

pub fn encode_as_bytes(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    re_tracing::profile_function!();
    let mut bytes: Vec<u8> = vec![];
    let mut encoder = Encoder::new(version, options, &mut bytes)?;
    for message in messages {
        encoder.append(&message?)?;
    }
    encoder.finish()?;
    Ok(bytes)
}

#[inline]
pub fn local_encoder() -> Result<DroppableEncoder<Vec<u8>>, EncodeError> {
    DroppableEncoder::new(
        CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        Vec::new(),
    )
}

#[inline]
pub fn local_raw_encoder() -> Result<Encoder<Vec<u8>>, EncodeError> {
    Encoder::new(
        CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        Vec::new(),
    )
}

#[inline]
pub fn encode_as_bytes_local(
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = local_raw_encoder()?;
    for message in messages {
        encoder.append(&message?)?;
    }
    encoder.finish()?;
    Ok(encoder.into_inner())
}

#[inline]
pub fn encode_ref_as_bytes_local<'a>(
    messages: impl Iterator<Item = ChunkResult<&'a LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = local_raw_encoder()?;
    for message in messages {
        encoder.append(message?)?;
    }
    encoder.finish()?;
    Ok(encoder.into_inner())
}
