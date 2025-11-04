//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::{borrow::Borrow, collections::HashMap};

use itertools::Itertools as _;

use re_build_info::CrateVersion;
use re_chunk::{Chunk, ChunkError, ChunkResult};
use re_log_types::{LogMsg, StoreId};

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
    // TODO: why is this suddenly unused, wat?
    version: CrateVersion,
    serializer: Serializer,
    compression: Compression,

    /// Optional so that we can `take()` it in `into_inner`, while still being allowed to implement `Drop`.
    write: Option<W>,

    /// How many bytes written out so far?
    num_written: u64,

    /// * So we don't ever successfully write partial messages.
    /// * Because `prost` only supports buffers, not IO traits.
    scratch: Vec<u8>,

    // TODO: one possibility here is that we actually make use of the option, and use both to make
    // it possible to disable footer and to force disable it in case of unsafe appends.
    // -> in which case is_finished cannot, in fact, go away.
    footer_state: Option<FooterState>,

    /// Tracks whether the end-of-stream marker has been written out already.
    //
    // TODO: i think this can go away in favor of whether the footer_state exists or not now.
    is_finished: bool,
}

// TODO: try loading old RRDs tho -- well actually i already know this works
// TODO: but still it'd be nice to make the footer generation configurable, just so that we can run
// tests with and without it, just in case.

/// The accumulated state used to build the footer when closing the [`Encoder`].
///
/// This is automatically updated when calling [`Encoder::append`].
//
// TODO: _but_, mention the role of the unsafe append in this picture.
#[derive(Default)]
struct FooterState {
    /// What is the currently active partition ID according to the state of the encoder, if any?
    ///
    /// Put another way: was there a `SetStoreInfo` message earlier in the stream? If so, we will
    /// want to override the partition ID of each chunk with that one (because that's the existing
    /// behavior, certainly not because it's nice).
    //
    // TODO: doc probably outta date now
    partition_id_scope: Option<re_log_types::StoreId>,

    manifests: HashMap<re_log_types::StoreId, ManifestState>,
}

#[derive(Default)]
struct ManifestState {
    /// The accumulated partition ID of each individual chunks, extracted from their `LogMsg`.
    ///
    /// This will only be used if [`Self::partition_id_scope`] is empty.
    //
    // TODO: partition ID doesn't make sense in this context (that is: OSS recordings)
    partition_ids: Vec<re_log_types::StoreId>,

    /// The state of the Sorbet schema currently being built.
    sorbet_schema_builder: re_sorbet::SchemaBuilder,

    /// The state of the RRD manifest currently being built.
    manifest: RrdManifestBuilder,
}

// TODO: nothing prevents us from having a `repeated RrdManifest`... and if we do, then suddenly a
// lot of things become less weird when it comes to filtering and such.
impl FooterState {
    fn append(&mut self, byte_offset: u64, byte_size: u64, msg: &re_log_types::LogMsg) {
        match msg {
            LogMsg::SetStoreInfo(msg) => {
                self.partition_id_scope = Some(msg.info.store_id.clone());
            }

            LogMsg::ArrowMsg(store_id, msg) => {
                // TODO: yeah, that probably has to go... or does it? technically it's fairly cheap really
                // or at least just about as cheap as performing the same kind of logic ourselves
                // in the end, ye?
                //
                // TODO: either way we should have a setting to disable footers for A) tests and B)
                // when you want to generate those laters anyhow?
                //
                // TODO: well, we have a write benchmark, dont we? let's use it.

                // TODO: aaaaaaaaaaaaahhhhhhhhhhhhhhh

                // TODO: also keep in mind that this means we're performing sorbet migration before
                // indexing.
                let chunk_batch = re_sorbet::ChunkBatch::try_from(&msg.batch).unwrap();
                let chunk = Chunk::from_chunk_batch(&chunk_batch).unwrap();

                // TODO: there are no partition IDs here, these are recording IDs.
                // TODO: explain that logic (which im not sure is correct anymore..)
                let partition_id = self
                    .partition_id_scope
                    .clone()
                    .unwrap_or_else(|| store_id.clone());

                let ManifestState {
                    partition_ids,
                    manifest,
                    sorbet_schema_builder,
                } = self.manifests.entry(partition_id.clone()).or_default();

                partition_ids.push(partition_id);
                sorbet_schema_builder.add_chunk(&chunk_batch);
                manifest.append(&chunk, byte_offset, byte_size).unwrap(); // TODO
            }

            LogMsg::BlueprintActivationCommand(_) => {}
        }
    }

    // TODO: what happens if the recording is empty? did we take care about that?
    fn finish(self) -> ChunkResult<crate::RrdFooter> {
        use std::sync::Arc;

        use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, StringArray};
        use arrow::datatypes::Schema;

        let manifests: Result<HashMap<StoreId, crate::RrdManifest>, _> = self
            .manifests
            .into_iter()
            .map(|(store_id, state)| {
                let ManifestState {
                    partition_ids,
                    sorbet_schema_builder,
                    manifest,
                } = state;

                let num_rows = partition_ids.len();

                let manifest = {
                    let (schema, columns) = {
                        // TODO: we can remove this now that we make that choice on each and every incoming chunk.
                        // let column_partition_ids = if let Some(partition_id) = partition_id_scope {
                        //     Arc::new(arrow::array::StringArray::from_iter_values(
                        //         std::iter::repeat_n(
                        //             re_protos::common::v1alpha1::StoreId::from(partition_id)
                        //                 .recording_id
                        //                 .as_str(),
                        //             partition_ids.len(),
                        //         ),
                        //     )) as ArrayRef
                        // } else {
                        //     Arc::new(StringArray::from_iter_values(
                        //         partition_ids
                        //             .into_iter()
                        //             .map(|id| re_protos::common::v1alpha1::StoreId::from(id).recording_id),
                        //     )) as ArrayRef
                        // };

                        // TODO: at this point we should probably get rid of this column entirely no?
                        let column_partition_ids = Arc::new(StringArray::from_iter_values(
                            partition_ids.into_iter().map(|id| {
                                re_protos::common::v1alpha1::StoreId::from(id).recording_id
                            }),
                        )) as ArrayRef;

                        let fields = std::iter::once(RrdManifestBuilder::partition_id_field())
                            .chain(manifest.fields())
                            .collect_vec();
                        let schema =
                            Arc::new(Schema::new_with_metadata(fields, Default::default()));

                        let columns = std::iter::once(column_partition_ids)
                            .chain(manifest.into_columns())
                            .collect_vec();

                        (schema, columns)
                    };

                    RecordBatch::try_new_with_options(
                        schema,
                        columns,
                        &RecordBatchOptions::new().with_row_count(Some(num_rows)),
                    )?
                };

                // TODO: yeah i mean the fact that the manifest builder doesn't actually build the full
                // manifest (hash, schema, and manifest) is just fucked

                let sorbet_schema = arrow::datatypes::Schema::new_with_metadata(
                    sorbet_schema_builder.build(),
                    Default::default(),
                );
                let sorbet_schema_sha256 =
                    crate::RrdManifest::compute_sorbet_schema_sha256(&sorbet_schema)?;

                // TODO: actually not sure why we have ChunkError here
                Ok::<_, ChunkError>((
                    store_id.clone(),
                    crate::RrdManifest {
                        store_id: store_id.clone(),
                        sorbet_schema,
                        sorbet_schema_sha256,
                        data: manifest,
                    },
                ))
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
            version,
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

        if self.write.is_none() {
            return Err(EncodeError::AlreadyUnwrapped);
        }

        re_tracing::profile_function!();

        let transport = message.to_transport(self.compression)?;

        let byte_offset_excluding_header =
            self.num_written + crate::MessageHeader::ENCODED_SIZE_BYTES as u64;

        // Safety: the compression settings of this message are consistent with this stream.
        #[expect(unsafe_code)]
        let n = unsafe { self.append_transport(&transport)? };

        let byte_size_excluding_header = n - crate::MessageHeader::ENCODED_SIZE_BYTES as u64;

        let Some(footer_state) = self.footer_state.as_mut() else {
            // TODO: bit disgusting doing that here I guess, but borrowck is so annoying tho..
            return Err(EncodeError::AlreadyFinished);
        };

        footer_state.append(
            byte_offset_excluding_header,
            byte_size_excluding_header,
            message,
        );

        Ok(n)
    }

    /// Returns the size in bytes of the encoded data.
    ///
    /// ## Safety
    ///
    /// `message` must respect the global settings of the encoder (e.g. the compression used),
    /// otherwise the resulting RRD stream will be corrupt and unreadable.
    //
    // TODO: it used to be a bit unsafe, but now it's very _very_ unsafe, since it bypasses footer
    // maintenance altogether. wat do?
    // TODO: it might be that we have to force users of this to opt out of footer generation, and
    // if they still want one they have to do it themselves (very likely by modifying an existing
    // one, since you generally already have one if you went through here).
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

        // TODO: probably redundant with the check above then.
        let Some(footer_state) = self.footer_state.take() else {
            return Ok(());
        };

        let Some(w) = self.write.as_mut() else {
            return Err(EncodeError::AlreadyUnwrapped);
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

        // Message payload (re_protos::RrdManifest)

        let mut out_rrd_footer = Vec::new();
        rrd_footer.to_rrd_bytes(&mut out_rrd_footer)?;
        w.write_all(&out_rrd_footer).map_err(EncodeError::Write)?;
        self.num_written += out_rrd_footer.len() as u64;

        // StreamFooter

        // TODO: we really need to make sure that the distinction between StreamFooter vs.
        // RrdFooter is crystal clear everywhere, it's very clusterfuckery otherwise.
        let mut out_stream_footer = Vec::new();
        StreamFooter::from_rrd_footer_bytes(
            end_msg_byte_offset_from_start_excluding_header,
            &out_rrd_footer,
        )
        .to_rrd_bytes(&mut out_stream_footer)?;
        w.write_all(&out_stream_footer)
            .map_err(EncodeError::Write)?;
        self.num_written += out_stream_footer.len() as u64;

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
