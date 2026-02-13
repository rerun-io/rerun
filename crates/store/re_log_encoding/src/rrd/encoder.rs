//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::borrow::Borrow;
use std::collections::HashMap;

use re_build_info::CrateVersion;
use re_chunk::{ChunkError, ChunkResult};
use re_log_types::{LogMsg, StoreId};
use re_sorbet::SorbetError;

use crate::{
    CodecError, Compression, Encodable as _, EncodingOptions, MessageHeader, MessageKind,
    RrdManifestBuilder, Serializer, StreamFooter, StreamHeader, ToTransport as _,
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

    #[error("Sorbet error: {0}")]
    Sorbet(Box<SorbetError>),
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

impl From<SorbetError> for EncodeError {
    fn from(err: SorbetError) -> Self {
        Self::Sorbet(Box::new(err))
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

    /// How many bytes written out so far?
    num_written: u64,

    /// * So we don't ever successfully write partial messages.
    /// * Because `prost` only supports buffers, not IO traits.
    scratch: Vec<u8>,

    /// Tracks the state required to build the RRD footer for this stream.
    ///
    /// If set to `None`, the footer will not be computed.
    ///
    /// Calling [`Self::append_transport`] will automatically disable footers.
    footer_state: Option<FooterState>,

    /// Tracks whether the end-of-stream marker, and optionally the associated footer, have been
    /// written out already.
    is_finished: bool,
}

/// The accumulated state used to build the footer when closing the [`Encoder`].
///
/// This is automatically updated when calling [`Encoder::append`].
#[derive(Default)]
struct FooterState {
    /// What is the currently active recording ID according to the state of the encoder, if any?
    ///
    /// Put another way: was there a `SetStoreInfo` message earlier in the stream? If so, we will
    /// want to override the recording ID of each chunk with that one (because that's the existing
    /// behavior, certainly not because it's nice).
    recording_id_scope: Option<re_log_types::StoreId>,

    manifests: HashMap<re_log_types::StoreId, ManifestState>,
}

/// The accumulated state for a specific RRD manifest.
#[derive(Default)]
struct ManifestState {
    /// The accumulated recording IDs of each individual chunk, extracted from their `LogMsg`.
    ///
    /// In most normal scenarios, this will just be the same value repeated N times.
    ///
    /// This will only be used if [`FooterState::recording_id_scope`] is empty.
    recording_ids: Vec<re_log_types::StoreId>,

    /// The state of the RRD manifest currently being built.
    manifest: RrdManifestBuilder,
}

impl FooterState {
    fn append(
        &mut self,
        msg: &re_log_types::LogMsg,
        byte_span_excluding_header: re_span::Span<u64>,
        byte_size_uncompressed: u64,
    ) -> Result<(), EncodeError> {
        match msg {
            LogMsg::SetStoreInfo(msg) => {
                self.recording_id_scope = Some(msg.info.store_id.clone());
            }

            LogMsg::ArrowMsg(store_id, msg) => {
                // NOTE(1): The fact that this parses the `RecordBatch` back into an actual `Chunk`
                // is a bit unfortunate, but really it's nowhere near as bad as one might think:
                // the real costly work is to parse the IPC payload into a `RecordBatch` in the
                // first place, but thankfully we don't have to repay that cost here.
                // Not only that: keep in mind that this entire codepath is only taken when writing
                // actual RRD files, so performance is generally IO bound anyway.
                //
                // NOTE(2): The fact that we also perform a Sorbet migration in the process is a
                // bit weirder on the other hand, but then again this is generally not a new a
                // problem: we tend to perform Sorbet migrations a bit too aggressively all over
                // the place. We really need a layer that sits between the transport and
                // application layer where one can accessed the parsed, unmigrated data.
                let chunk_batch = re_sorbet::ChunkBatch::try_from(&msg.batch)?;

                // See `self.recording_id_scope` for some explanations.
                let recording_id = self
                    .recording_id_scope
                    .clone()
                    .unwrap_or_else(|| store_id.clone());

                // This line is important: it implies that if a recording doesn't have any data
                // chunks at all, we do not even reserve an RRD manifest for it in the footer.
                let ManifestState {
                    recording_ids,
                    manifest,
                } = self.manifests.entry(recording_id.clone()).or_default();

                recording_ids.push(recording_id);
                manifest.append(
                    &chunk_batch,
                    byte_span_excluding_header,
                    byte_size_uncompressed,
                )?;
            }

            LogMsg::BlueprintActivationCommand(_) => {}
        }

        Ok(())
    }

    fn finish(self) -> Result<crate::RrdFooter, EncodeError> {
        let manifests: Result<HashMap<StoreId, crate::RawRrdManifest>, _> = self
            .manifests
            .into_iter()
            .map(|(store_id, state)| {
                state
                    .manifest
                    .build(store_id.clone())
                    .map(|m| (store_id, m))
            })
            .collect();

        Ok(crate::RrdFooter {
            manifests: manifests?,
        })
    }
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
            num_written: out.len() as u64,
            scratch: Vec::new(),
            footer_state: Some(FooterState::default()),
            is_finished: false,
        })
    }

    /// Returns the size in bytes of the encoded data.
    pub fn append(&mut self, message: &re_log_types::LogMsg) -> Result<u64, EncodeError> {
        if self.is_finished {
            return Err(EncodeError::AlreadyFinished);
        }

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        re_tracing::profile_function!();

        let transport = message.to_transport(self.compression)?;

        let byte_offset_excluding_header =
            self.num_written + crate::MessageHeader::ENCODED_SIZE_BYTES as u64;

        self.scratch.clear();
        let n = match self.serializer {
            Serializer::Protobuf => {
                transport.to_rrd_bytes(&mut self.scratch)?;
                let n = w
                    .write_all(&self.scratch)
                    .map(|_| self.scratch.len() as u64)
                    .map_err(EncodeError::Write)?;
                self.num_written += n;
                n
            }
        };

        let byte_size_excluding_header = n - crate::MessageHeader::ENCODED_SIZE_BYTES as u64;

        let byte_span_excluding_header = re_span::Span {
            start: byte_offset_excluding_header,
            len: byte_size_excluding_header,
        };

        if let Some(footer_state) = self.footer_state.as_mut() {
            footer_state.append(
                message,
                byte_span_excluding_header,
                transport.byte_size_uncompressed(),
            )?;
        }

        Ok(n)
    }

    /// Instructs the encoder to _not_ emit a footer at the end of the stream.
    ///
    /// This cannot be reverted.
    pub fn do_not_emit_footer(&mut self) {
        self.footer_state = None;
    }

    /// Returns the size in bytes of the encoded data.
    ///
    /// ⚠️ This implies [`Self::do_not_emit_footer`]. ⚠️
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

        re_tracing::profile_function!();

        // We cannot update the RRD manifest without decoding the message, which would defeat the
        // entire purposes of using this method in the first place.
        // Therefore, we disable footers if and when this method is used.
        self.do_not_emit_footer();

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
        };

        self.scratch.clear();
        match self.serializer {
            Serializer::Protobuf => {
                message.to_rrd_bytes(&mut self.scratch)?;
                let n = w
                    .write_all(&self.scratch)
                    .map(|_| self.scratch.len() as u64)
                    .map_err(EncodeError::Write)?;
                self.num_written += n;
                Ok(n)
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

        self.is_finished = true;

        let Some(footer_state) = self.footer_state.take() else {
            return Ok(());
        };

        // TODO(cmc): the extra heap-allocs and copies could be easily avoided with the
        // introduction of an InMemoryWriter trait or similar. In practice it makes no
        // difference and the cognitive overhead of this crate is already through the roof.

        use re_protos::external::prost::Message as _;

        // Message Header (::End)

        let rrd_footer = footer_state.finish()?;
        let rrd_footer = rrd_footer.to_transport(())?;

        let mut out_header = Vec::new();
        MessageHeader {
            kind: MessageKind::End,
            len: rrd_footer.encoded_len() as u64,
        }
        .to_rrd_bytes(&mut out_header)?;
        w.write_all(&out_header).map_err(EncodeError::Write)?;
        self.num_written += out_header.len() as u64;

        let end_msg_byte_offset_from_start_excluding_header = self.num_written;

        // Message payload (re_protos::RrdFooter)

        let mut out_rrd_footer = Vec::new();
        rrd_footer.to_rrd_bytes(&mut out_rrd_footer)?;
        w.write_all(&out_rrd_footer).map_err(EncodeError::Write)?;
        self.num_written += out_rrd_footer.len() as u64;

        // StreamFooter

        let mut out_stream_footer = Vec::new();
        StreamFooter::from_rrd_footer_bytes(
            end_msg_byte_offset_from_start_excluding_header,
            &out_rrd_footer,
        )
        .to_rrd_bytes(&mut out_stream_footer)?;
        w.write_all(&out_stream_footer)
            .map_err(EncodeError::Write)?;
        self.num_written += out_stream_footer.len() as u64;

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

impl<W: std::io::Write> std::ops::Drop for Encoder<W> {
    fn drop(&mut self) {
        if self.write.is_none() {
            // Already unwrapped: nothing to see here.
            return;
        }

        if let Err(err) = self.finish() {
            re_log::warn!("encoder couldn't be finished: {err}");
        }

        if let Err(err) = self.flush_blocking() {
            re_log::warn!("encoder couldn't be flushed: {err}");
        }
    }
}
