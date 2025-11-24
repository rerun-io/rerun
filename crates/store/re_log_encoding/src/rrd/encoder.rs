//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::borrow::Borrow;

use re_build_info::CrateVersion;
use re_chunk::{ChunkError, ChunkResult};
use re_log_types::LogMsg;

use crate::{
    ToTransport as _,
    rrd::{
        CodecError, Compression, Encodable as _, EncodingOptions, MessageHeader, MessageKind,
        Serializer, StreamHeader,
    },
};

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Called append on already finished encoder")]
    AlreadyFinished,

    #[error("Called append on already unwrapped encoder")]
    AlreadyUnwrapped,

    #[error("Failed to write: {0}")]
    Write(#[from] std::io::Error),

    #[error("{0}")]
    Codec(Box<crate::rrd::CodecError>),

    #[error("Chunk error: {0}")]
    Chunk(Box<ChunkError>),
}

const _: () = assert!(
    std::mem::size_of::<EncodeError>() <= 48,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl From<CodecError> for EncodeError {
    fn from(err: CodecError) -> Self {
        Self::Codec(Box::new(err))
    }
}

impl From<ChunkError> for EncodeError {
    fn from(err: ChunkError) -> Self {
        Self::Chunk(Box::new(err))
    }
}

// ----------------------------------------------------------------------------

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
///
/// When dropped, it will automatically insert an end-of-stream marker, if that wasn't already done manually.
//
// TODO(cmc): I hate not having a `BufWrite` trait. This is just asking for trouble.
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
        Self::new_eager(
            CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            Vec::new(),
        )
    }

    /// All-in-one helper to encode a stream of [`LogMsg`]s into an actual RRD stream.
    ///
    /// This always uses the local version and its default encoding options.
    ///
    /// Returns the encoded data in a newly allocated vector.
    pub fn encode(
        messages: impl IntoIterator<Item = ChunkResult<impl Borrow<LogMsg>>>,
    ) -> Result<Vec<u8>, EncodeError> {
        re_tracing::profile_function!();
        let mut encoder = Self::local()?;
        for message in messages {
            encoder.append(message?.borrow())?;
        }
        encoder.finish()?;
        encoder.into_inner()
    }
}

impl<W: std::io::Write> Encoder<W> {
    /// Creates a new [`Encoder`].
    ///
    /// This will immediately write the [`StreamHeader`] to the output stream as part of
    /// initialization (hence `_eager`).
    ///
    /// There is no `_lazy` version. Make one if you need one.
    pub fn new_eager(
        version: CrateVersion,
        options: EncodingOptions,
        mut write: W,
    ) -> Result<Self, EncodeError> {
        // TODO(cmc): the extra heap-alloc and copy could be easily avoided with the
        // introduction of an InMemoryWriter trait or similar. In practice it makes no
        // difference and the cognitive overhead of this crate is already through the roof.
        let mut out = Vec::new();
        StreamHeader {
            fourcc: crate::rrd::RRD_FOURCC,
            version: version.to_bytes(),
            options,
        }
        .to_rrd_bytes(&mut out)?;
        write.write_all(&out)?;

        Ok(Self {
            serializer: options.serializer,
            compression: options.compression,
            write: Some(write),
            scratch: Vec::new(),
            is_finished: false,
        })
    }

    /// Returns the size in bytes of the encoded data.
    pub fn append(&mut self, message: &re_log_types::LogMsg) -> Result<u64, EncodeError> {
        if self.is_finished {
            return Err(EncodeError::AlreadyFinished);
        }

        if self.write.is_none() {
            return Err(EncodeError::AlreadyUnwrapped);
        }

        re_tracing::profile_function!();

        let message = message.to_transport(self.compression)?;
        // Safety: the compression settings of this message are consistent with this stream.
        #[expect(unsafe_code)]
        unsafe {
            self.append_transport(&message)
        }
    }

    /// Returns the size in bytes of the encoded data.
    ///
    /// ## Safety
    ///
    /// `message` must respect the global settings of the encoder (e.g. the compression used),
    /// otherwise the resulting RRD stream will be corrupt and unreadable.
    #[expect(unsafe_code)]
    pub unsafe fn append_transport(
        &mut self,
        message: &re_protos::log_msg::v1alpha1::log_msg::Msg,
    ) -> Result<u64, EncodeError> {
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
                message.to_rrd_bytes(&mut self.scratch)?;
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
                // TODO(cmc): the extra heap-alloc and copy could be easily avoided with the
                // introduction of an InMemoryWriter trait or similar. In practice it makes no
                // difference and the cognitive overhead of this crate is already through the roof.
                let mut header = Vec::new();
                MessageHeader {
                    kind: MessageKind::End,
                    len: 0,
                }
                .to_rrd_bytes(&mut header)?;
                w.write_all(&header)?;
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
        messages: impl IntoIterator<Item = ChunkResult<impl Borrow<LogMsg>>>,
        write: &mut W,
    ) -> Result<u64, EncodeError> {
        re_tracing::profile_function!();
        let mut encoder = Encoder::new_eager(version, options, write)?;
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
