//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::borrow::Borrow;

use re_build_info::CrateVersion;
use re_chunk::{ChunkError, ChunkResult};
use re_log_types::LogMsg;
use re_protos::log_msg::v1alpha1::LogMsg as LogMsgProto;

use crate::FileHeader;
use crate::Serializer;
use crate::codec;
use crate::codec::file::{self, encoder};
use crate::{Compression, EncodingOptions};

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
    Chunk(Box<ChunkError>),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,

    #[error("Called append on already unwrapped encoder")]
    AlreadyUnwrapped,

    #[error("Missing field: {0}")]
    MissingField(&'static str),
}

const _: () = assert!(
    std::mem::size_of::<EncodeError>() <= 48,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl From<ChunkError> for EncodeError {
    fn from(err: ChunkError) -> Self {
        Self::Chunk(Box::new(err))
    }
}

// ----------------------------------------------------------------------------

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
///
/// When dropped, it will automatically insert an end-of-stream marker, if that wasn't already done manually.
pub struct Encoder<W: std::io::Write> {
    serializer: Serializer,
    compression: Compression,

    /// Optional so that we can `take()` it in `into_inner`, while still being allowed to implement `Drop`.
    write: Option<W>,

    /// So we don't ever successfully write partial messages.
    scratch: Vec<u8>,

    /// Tracks whether the end-of-stream marker has been written out already.
    is_finished: bool,
}

impl Encoder<Vec<u8>> {
    pub fn local() -> Result<Self, EncodeError> {
        Self::new(
            CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            Vec::new(),
        )
    }
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(
        version: CrateVersion,
        options: EncodingOptions,
        mut write: W,
    ) -> Result<Self, EncodeError> {
        FileHeader {
            fourcc: crate::RRD_FOURCC,
            version: version.to_bytes(),
            options,
        }
        .encode(&mut write)?;

        Ok(Self {
            serializer: options.serializer,
            compression: options.compression,
            write: Some(write),
            scratch: Vec::new(),
            is_finished: false,
        })
    }

    /// Returns the size in bytes of the encoded data.
    pub fn append(&mut self, message: &LogMsg) -> Result<u64, EncodeError> {
        if self.is_finished {
            return Err(EncodeError::AlreadyFinished);
        }

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        re_tracing::profile_function!();

        self.scratch.clear();
        match self.serializer {
            Serializer::Protobuf => {
                encoder::encode(&mut self.scratch, message, self.compression)?;

                w.write_all(&self.scratch)
                    .map(|_| self.scratch.len() as _)
                    .map_err(EncodeError::Write)
            }
        }
    }

    /// Returns the size in bytes of the encoded data.
    pub fn append_proto(&mut self, message: LogMsgProto) -> Result<u64, EncodeError> {
        if self.is_finished {
            return Err(EncodeError::AlreadyFinished);
        }

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        re_tracing::profile_function!();

        self.scratch.clear();
        match self.serializer {
            Serializer::Protobuf => {
                encoder::encode_proto(&mut self.scratch, message)?;

                w.write_all(&self.scratch)
                    .map(|_| self.scratch.len() as _)
                    .map_err(EncodeError::Write)
            }
        }
    }

    /// Appends an end-of-stream marker to the encoded bytes. Does not flush.
    ///
    /// This is idempotent. This is called automatically on drop.
    ///
    /// This end-of-stream marker is currently (seemingly?) relied on for:
    /// * Tail mode (where the Viewer continuously poll reads from a file on disk).
    /// * Concatenated RRD file streams (e.g. `cat *.rrd | rerun -`).
    #[inline]
    pub fn finish(&mut self) -> Result<(), EncodeError> {
        if self.is_finished {
            return Ok(());
        }

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        match self.serializer {
            Serializer::Protobuf => {
                file::MessageHeader {
                    kind: file::MessageKind::End,
                    len: 0,
                }
                .encode(w)?;
            }
        }

        self.is_finished = true;

        Ok(())
    }

    #[inline]
    pub fn flush_blocking(&mut self) -> Result<(), EncodeError> {
        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        Ok(w.flush()?)
    }

    #[inline]
    pub fn into_inner(mut self) -> Result<W, EncodeError> {
        self.write.take().ok_or(EncodeError::AlreadyUnwrapped)
    }
}

impl<W: std::io::Write> Encoder<W> {
    /// All-in-one helper to encode a stream of [`LogMsg`]s into an actual RRD stream.
    ///
    /// Returns the size in bytes of the encoded data.
    pub fn encode_into(
        version: CrateVersion,
        options: EncodingOptions,
        messages: impl Iterator<Item = ChunkResult<impl Borrow<LogMsg>>>,
        write: &mut W,
    ) -> Result<u64, EncodeError> {
        re_tracing::profile_function!();
        let mut encoder = Encoder::new(version, options, write)?;
        let mut size_bytes = 0;
        for message in messages {
            size_bytes += encoder.append(message?.borrow())?;
        }
        Ok(size_bytes)
    }
}

// TODO(cmc): It seems a bit suspicious to me that we send an EOS marker on drop, but don't flush.
// But I don't want to change any flushing behavior at the moment, so I'll keep it that way for now.
impl<W: std::io::Write> std::ops::Drop for Encoder<W> {
    fn drop(&mut self) {
        if self.write.is_none() {
            // Already unwrapped: nothing to see here.
            return;
        }

        if let Err(err) = self.finish() {
            re_log::warn!("encoder couldn't be finished: {err}");
        }
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
    let mut encoder = Encoder::new(version, options, write)?;
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
    let mut encoder = Encoder::new(version, options, write)?;
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
    let mut encoder = Encoder::new(version, options, vec![])?;
    for message in messages {
        encoder.append(&message?)?;
    }
    encoder.finish()?;
    encoder.into_inner()
}

#[inline]
pub fn encode_as_bytes_local(
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = Encoder::local()?;
    for message in messages {
        encoder.append(&message?)?;
    }
    encoder.finish()?;
    encoder.into_inner()
}

#[inline]
pub fn encode_ref_as_bytes_local<'a>(
    messages: impl Iterator<Item = ChunkResult<&'a LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = Encoder::local()?;
    for message in messages {
        encoder.append(message?)?;
    }
    encoder.finish()?;
    encoder.into_inner()
}
