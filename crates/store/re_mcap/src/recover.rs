//! In-memory recovery of a [`Summary`] for truncated / summary-less MCAP files.
//!
//! A recording that was interrupted mid-write has a valid start magic but no footer/summary,
//! this module reconstructs an equivalent [`Summary`] in memory from a
//! front-to-back scan of the data section, tolerating the truncated tail.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use mcap::read::{Options, RawMessageStream, parse_record};
use mcap::records::{self, op};
use mcap::{Channel, McapError, Schema, Summary};

use crate::{Error, McapSummarySource};

/// The result of a cheap, decompression-free scan of an MCAP data section.
///
/// All offsets are absolute byte offsets into the original file, pointing at the opcode byte of the
/// corresponding record — matching what [`Summary::stream_chunk`], [`Summary::read_message_indexes`],
/// and [`mcap::read::metadata`] expect.
#[derive(Debug, Default)]
pub struct ScanResult {
    /// One entry per complete `Chunk` record, in file order. `message_index_offsets` is populated
    /// from the `MessageIndex` records that follow each chunk.
    pub chunk_indexes: Vec<records::ChunkIndex>,

    /// One entry per top-level `Metadata` record, so the metadata decoder still sees them.
    pub metadata_indexes: Vec<records::MetadataIndex>,

    /// The set of channel ids referenced by any `MessageIndex` record.
    pub referenced_channels: BTreeSet<u16>,

    /// Whether any `Message` record was found in the data section outside a chunk. Recovery only
    /// rebuilds the chunk index, so such messages cannot be recovered.
    pub has_unchunked_messages: bool,
}

impl ScanResult {
    /// Errors if the file cannot be recovered from its chunk index alone.
    ///
    /// Recovery rebuilds only the chunk index, so an unchunked file (messages stored outside
    /// chunks) has nothing for us to index and is rejected rather than recovered as empty.
    pub fn reject_if_unrecoverable(&self) -> Result<(), Error> {
        if self.has_unchunked_messages && self.chunk_indexes.is_empty() {
            return Err(Error::Other(anyhow::anyhow!(
                "Cannot recover an unchunked MCAP file: its messages are stored outside chunks. Re-record with chunking enabled, or run `mcap recover`"
            )));
        }
        Ok(())
    }

    /// The chunks that carry message indexes — i.e. the ones a reconstructed [`Summary`] keeps.
    ///
    /// A chunk truncated before its `MessageIndex` records has none, and
    /// [`Summary::read_message_indexes`] errors on an empty offsets map, so it is skipped.
    pub fn usable_chunks(&self) -> impl Iterator<Item = &records::ChunkIndex> {
        self.chunk_indexes
            .iter()
            .filter(|chunk| !chunk.message_index_offsets.is_empty())
    }
}

/// Cheaply scans the data section of an MCAP file, reconstructing its chunk + metadata index.
///
/// This walks record headers from front to back without decompressing any chunk, and is tolerant of
/// a truncated tail: as soon as a record would run past the end of the buffer (or an unparsable /
/// summary-section record is hit), the scan stops and returns everything collected so far.
///
/// Errors only if the file does not begin with the MCAP start magic (i.e. it is not an MCAP file at
/// all, as opposed to a merely truncated one).
pub fn build_chunk_index(mcap: &[u8]) -> Result<ScanResult, Error> {
    re_tracing::profile_function!();

    if !mcap.starts_with(mcap::MAGIC) {
        return Err(Error::MissingStartMagic);
    }

    let mut scan = ScanResult::default();
    // Index into `scan.chunk_indexes` of the chunk that subsequent `MessageIndex` records belong to.
    let mut current_chunk: Option<usize> = None;

    // We walk the raw record framing (a 1-byte opcode + `u64` length prefix) by hand rather than
    // going through `mcap::read::LinearReader`: that iterator yields parsed records but not their
    // byte offsets, and the reconstructed `ChunkIndex` / `MetadataIndex` need each record's absolute
    // offset. Record *bodies* are still handed to `parse_record`, so only the envelope is manual.
    //
    let mut off = mcap::MAGIC.len();
    while off + crate::RECORD_HEADER_LEN <= mcap.len() {
        let opcode = mcap[off];
        let len = u64::from_le_bytes(
            mcap[off + 1..off + crate::RECORD_HEADER_LEN]
                .try_into()
                .expect("slice is exactly 8 bytes"),
        );
        let body_start = off + crate::RECORD_HEADER_LEN;
        let Some(body_end) = usize::try_from(len)
            .ok()
            .and_then(|len| body_start.checked_add(len))
        else {
            re_log::warn!("MCAP record length overflows the file; dropping the truncated tail");
            break;
        };
        if body_end > mcap.len() {
            re_log::warn!(
                "MCAP file appears truncated; dropping {} trailing byte(s) after the last complete record",
                mcap.len() - off
            );
            break;
        }
        let body = &mcap[body_start..body_end];

        match opcode {
            op::CHUNK => {
                if let Ok(records::Record::Chunk { header, .. }) = parse_record(op::CHUNK, body) {
                    scan.chunk_indexes.push(records::ChunkIndex {
                        message_start_time: header.message_start_time,
                        message_end_time: header.message_end_time,
                        chunk_start_offset: off as u64,
                        chunk_length: (body_end - off) as u64,
                        message_index_offsets: Default::default(),
                        message_index_length: 0,
                        compression: header.compression,
                        compressed_size: header.compressed_size,
                        uncompressed_size: header.uncompressed_size,
                    });
                    current_chunk = Some(scan.chunk_indexes.len() - 1);
                } else {
                    re_log::warn!(
                        "Failed to parse an MCAP chunk header. Stopping chunk index recovery."
                    );
                    break;
                }
            }

            op::MESSAGE_INDEX => {
                // Record the offset so `Summary::read_message_indexes` can find this index.
                let Ok(records::Record::MessageIndex(index)) =
                    parse_record(op::MESSAGE_INDEX, body)
                else {
                    // Carrying on would leave this chunk short an index record, with nothing later
                    // to tell us which one, so treat it as corruption and drop the tail.
                    re_log::warn!("Failed to parse an MCAP message index; dropping the tail");
                    break;
                };
                if let Some(idx) = current_chunk {
                    scan.chunk_indexes[idx]
                        .message_index_offsets
                        .insert(index.channel_id, off as u64);
                    scan.referenced_channels.insert(index.channel_id);
                }
            }

            op::METADATA => {
                // Reconstruct a `MetadataIndex` so `McapMetadataDecoder` still resolves it.
                if let Ok(records::Record::Metadata(metadata)) = parse_record(op::METADATA, body) {
                    scan.metadata_indexes.push(records::MetadataIndex {
                        offset: off as u64,
                        length: (body_end - off) as u64,
                        name: metadata.name,
                    });
                }
            }

            // A `Message` outside a chunk means the file is unchunked; the index-only scan cannot
            // recover it. Flag it so `reconstruct_summary` can reject the file explicitly.
            op::MESSAGE => scan.has_unchunked_messages = true,

            // Other data-section records (header, schema, channel, attachment) carry nothing we
            // need here — the scan is index-only.
            op::HEADER | op::SCHEMA | op::CHANNEL | op::ATTACHMENT => {}

            // The data section ends here; everything after is the summary section (or the footer),
            // which is exactly what a truncated file lacks and what we are reconstructing.
            op::DATA_END
            | op::FOOTER
            | op::CHUNK_INDEX
            | op::ATTACHMENT_INDEX
            | op::STATISTICS
            | op::METADATA_INDEX
            | op::SUMMARY_OFFSET => break,

            // Unknown opcode — treat as corruption and drop the tail.
            _ => {
                re_log::warn!(
                    "Encountered an unexpected MCAP record (opcode {opcode:#04x}); dropping the tail"
                );
                break;
            }
        }

        off = body_end;
    }

    Ok(scan)
}

/// Reconstructs an in-memory [`Summary`] for a truncated / summary-less MCAP file.
///
/// Calls [`build_chunk_index`] for the chunk + metadata index, then harvests the channel/schema
/// definitions — which live *inside* chunks — via a [`RawMessageStream`] that decompresses chunks
/// front-to-back only until every referenced channel is resolved (typically the first few chunks).
///
/// The reconstruction is conservative: a chunk with no message indexes at all is dropped, and any
/// channel that could not be resolved (declared only inside a dropped chunk) is stripped from the
/// remaining chunks. This guarantees the invariants the decode path relies on — every kept chunk has
/// a non-empty `message_index_offsets`, and every referenced channel has a definition.
///
/// The recovered statistics count the decodable messages — see [`channel_message_counts`].
pub(crate) fn reconstruct_summary(mcap: &[u8]) -> Result<Summary, Error> {
    re_tracing::profile_function!();

    let scan = build_chunk_index(mcap)?;
    scan.reject_if_unrecoverable()?;

    // Keep only the chunks that carry message indexes; a chunk truncated before its `MessageIndex`
    // records is unusable.
    let mut chunk_indexes: Vec<records::ChunkIndex> = scan.usable_chunks().cloned().collect();
    if chunk_indexes.len() != scan.chunk_indexes.len() {
        re_log::warn!(
            "Dropping {} MCAP chunk(s) with no message indexes (truncated tail)",
            scan.chunk_indexes.len() - chunk_indexes.len()
        );
    }

    // Recompute the referenced channels from the *kept* chunks only.
    let referenced_channels: BTreeSet<u16> = chunk_indexes
        .iter()
        .flat_map(|chunk| chunk.message_index_offsets.keys().copied())
        .collect();

    // Harvest channel/schema definitions. `RawMessageStream` decompresses chunks front-to-back;
    // we stop as soon as every referenced channel resolves (channels are declared before their
    // first message, so this converges near the last channel's first appearance).
    let mut stream = RawMessageStream::new_with_options(mcap, Options::IgnoreEndMagic.into())?;
    loop {
        if referenced_channels
            .iter()
            .all(|id| stream.get_channel(*id).is_some())
        {
            break;
        }
        match stream.next() {
            Some(Ok(_)) => {}
            // A clean end of stream, or a truncated / corrupt tail that ends it mid-record
            // (`UnexpectedEof`) or mid-chunk (`UnexpectedEoc`).
            // Either way we stop, as `RawMessageStream` yields nothing more after an error.
            None | Some(Err(McapError::UnexpectedEof | McapError::UnexpectedEoc)) => break,
            Some(Err(err)) => {
                re_log::warn!(
                    "Stopped recovering MCAP channel definitions early after an unexpected error: {err}"
                );
                break;
            }
        }
    }

    let mut channels: HashMap<u16, Arc<Channel<'static>>> = HashMap::new();
    let mut schemas: HashMap<u16, Arc<Schema<'static>>> = HashMap::new();
    for &id in &referenced_channels {
        // `get_channel` borrows the mmap (`Arc<Channel<'a>>`); deep-copy to `'static` for `Summary`.
        if let Some(channel) = stream.get_channel(id) {
            let channel = Arc::new(channel_to_static(&channel));
            if let Some(schema) = &channel.schema {
                schemas.insert(schema.id, schema.clone());
            }
            channels.insert(id, channel);
        }
    }

    // Reconcile any channel we couldn't resolve (declared only inside a dropped/corrupt chunk):
    // strip it from every kept chunk, then drop any chunk left with no message indexes.
    let unresolved: Vec<u16> = referenced_channels
        .iter()
        .copied()
        .filter(|id| !channels.contains_key(id))
        .collect();
    if !unresolved.is_empty() {
        re_log::warn!(
            "Dropping {} MCAP channel(s) whose definition was lost in the truncated tail",
            unresolved.len()
        );
        for chunk in &mut chunk_indexes {
            for id in &unresolved {
                chunk.message_index_offsets.remove(id);
            }
        }
        chunk_indexes.retain(|chunk| !chunk.message_index_offsets.is_empty());
    }

    // Counting needs the resolved channels, so build the summary first and fill in its statistics.
    let summary = Summary {
        stats: None,
        channels,
        schemas,
        chunk_indexes,
        attachment_indexes: Vec::new(),
        metadata_indexes: scan.metadata_indexes,
    };

    let channel_message_counts = channel_message_counts(mcap, &summary);
    let (message_start_time, message_end_time) =
        summary
            .chunk_indexes
            .iter()
            .fold((u64::MAX, 0_u64), |(lo, hi), chunk| {
                (
                    lo.min(chunk.message_start_time),
                    hi.max(chunk.message_end_time),
                )
            });
    let stats = records::Statistics {
        message_count: channel_message_counts.values().copied().sum(),
        schema_count: summary.schemas.len() as u16,
        channel_count: summary.channels.len() as u32,
        attachment_count: 0,
        metadata_count: summary.metadata_indexes.len() as u32,
        chunk_count: summary.chunk_indexes.len() as u32,
        // If every chunk was dropped, the fold leaves `lo > hi`; normalize to zeroes.
        message_start_time: if summary.chunk_indexes.is_empty() {
            0
        } else {
            message_start_time
        },
        message_end_time,
        channel_message_counts,
    };

    Ok(Summary {
        stats: Some(stats),
        ..summary
    })
}

/// Per-channel message counts for a reconstructed [`Summary`].
///
/// Messages on a channel we could not resolve are left out, since they are not decodable either.
fn channel_message_counts(mcap: &[u8], summary: &Summary) -> BTreeMap<u16, u64> {
    re_tracing::profile_function!();

    let Some((last, rest)) = summary.chunk_indexes.split_last() else {
        return BTreeMap::new();
    };

    // Only the final chunk can be short a `MessageIndex` record, so it is counted from its body.
    let mut counts = counts_from_body(mcap, summary, last)
        .or_else(|| counts_from_indexes(mcap, summary, last))
        .unwrap_or_default();

    for chunk in rest {
        for (channel_id, count) in counts_from_indexes(mcap, summary, chunk).unwrap_or_default() {
            *counts.entry(channel_id).or_default() += count;
        }
    }

    counts
}

/// Counts the messages of one chunk from its `MessageIndex` entries, which needs no decompression.
fn counts_from_indexes(
    mcap: &[u8],
    summary: &Summary,
    chunk: &records::ChunkIndex,
) -> Option<BTreeMap<u16, u64>> {
    match summary.read_message_indexes(mcap, chunk) {
        Ok(indexes) => Some(
            indexes
                .into_iter()
                .map(|(channel, entries)| (channel.id, entries.len() as u64))
                .collect(),
        ),
        Err(err) => {
            re_log::warn_once!("Failed to read the message indexes of an MCAP chunk: {err}");
            None
        }
    }
}

/// Counts the messages of one chunk from its body, decompressing it if necessary.
///
/// Use this only if the chunk's `MessageIndex` block is missing or incomplete
/// (otherwise: [`counts_from_indexes`]).
fn counts_from_body(
    mcap: &[u8],
    summary: &Summary,
    chunk: &records::ChunkIndex,
) -> Option<BTreeMap<u16, u64>> {
    let messages = match summary.stream_chunk(mcap, chunk) {
        Ok(messages) => messages,
        Err(err) => {
            re_log::warn_once!("Failed to read the messages of an MCAP chunk: {err}");
            return None;
        }
    };

    let mut counts = BTreeMap::new();
    for msg in messages.flatten() {
        *counts.entry(msg.channel.id).or_default() += 1;
    }
    Some(counts)
}

/// Reads an MCAP [`Summary`], falling back to `reconstruct_summary` when `recover` is set and the
/// file has no valid summary.
///
/// With `recover` disabled, a missing summary is an error and a read failure is propagated.
pub fn read_or_reconstruct_summary(mcap: &[u8], recover: bool) -> Result<Summary, Error> {
    read_or_reconstruct_summary_with_source(mcap, recover).map(|(summary, _source)| summary)
}

/// Like [`read_or_reconstruct_summary`], but also reports where the summary came from.
pub(crate) fn read_or_reconstruct_summary_with_source(
    mcap: &[u8],
    recover: bool,
) -> Result<(Summary, McapSummarySource), Error> {
    if !mcap.starts_with(mcap::MAGIC) {
        return Err(Error::MissingStartMagic);
    }

    match crate::read_summary(std::io::Cursor::new(mcap)) {
        Ok(Some(summary)) => return Ok((summary, McapSummarySource::Embedded)),
        Ok(None) if !recover => {
            return Err(Error::SummaryRequiresRecovery {
                details: anyhow::anyhow!("MCAP file does not contain a summary"),
            });
        }
        Err(details) if !recover => return Err(Error::SummaryRequiresRecovery { details }),
        // Recover mode: a missing or unreadable summary falls through to reconstruction.
        Ok(None) => {
            re_log::warn!(
                "MCAP file has no summary; reconstructing one in memory. The file may be truncated"
            );
        }
        Err(err) => {
            re_log::warn!(
                "Failed to read the MCAP summary ({err}); reconstructing one in memory. The file may be truncated"
            );
        }
    }

    reconstruct_summary(mcap).map(|summary| (summary, McapSummarySource::Reconstructed))
}

/// Deep-copies a [`Schema`] into an owned, `'static` value.
fn schema_to_static(schema: &Schema<'_>) -> Schema<'static> {
    Schema {
        id: schema.id,
        name: schema.name.clone(),
        encoding: schema.encoding.clone(),
        data: Cow::Owned(schema.data.to_vec()),
    }
}

/// Deep-copies a [`Channel`] (and its schema) into an owned, `'static` value.
///
/// [`RawMessageStream::get_channel`] hands back an `Arc<Channel<'a>>` borrowing the mapped file,
/// but [`Summary`] needs `Arc<Channel<'static>>`, so we copy field-by-field.
fn channel_to_static(channel: &Channel<'_>) -> Channel<'static> {
    Channel {
        id: channel.id,
        topic: channel.topic.clone(),
        schema: channel
            .schema
            .as_ref()
            .map(|schema| Arc::new(schema_to_static(schema))),
        message_encoding: channel.message_encoding.clone(),
        metadata: channel.metadata.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::io;

    use super::*;

    /// Builds a healthy MCAP with three channels across three flushed chunks. Channels `a` and `b`
    /// appear from the first chunk; channel `c` is introduced only in the third chunk (its `Channel`
    /// record lands there), exercising late-channel resolution.
    ///
    /// Returns `(buffer, [id_a, id_b, id_c])`.
    fn healthy_mcap() -> (Vec<u8>, [u16; 3]) {
        let cursor = io::Cursor::new(Vec::new());
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

        let id_a = writer
            .add_channel(0, "/a", "raw", &Default::default())
            .expect("add channel a");
        let id_b = writer
            .add_channel(0, "/b", "raw", &Default::default())
            .expect("add channel b");

        let write = |writer: &mut mcap::Writer<_>, channel_id: u16, log_time: u64| {
            writer
                .write_to_known_channel(
                    &mcap::records::MessageHeader {
                        channel_id,
                        sequence: 0,
                        log_time,
                        publish_time: log_time,
                    },
                    &[1, 2, 3],
                )
                .expect("write message");
        };

        // Chunk 1: a, b
        write(&mut writer, id_a, 10);
        write(&mut writer, id_b, 11);
        writer.flush().expect("flush chunk 1");

        // Chunk 2: a, b
        write(&mut writer, id_a, 20);
        write(&mut writer, id_b, 21);
        writer.flush().expect("flush chunk 2");

        // Channel c is added only now, so its `Channel` record lands in chunk 3.
        let id_c = writer
            .add_channel(0, "/c", "raw", &Default::default())
            .expect("add channel c");
        write(&mut writer, id_c, 30);
        write(&mut writer, id_a, 31);
        writer.flush().expect("flush chunk 3");

        writer.finish().expect("finish writer");
        let buffer = writer.into_inner().into_inner();
        (buffer, [id_a, id_b, id_c])
    }

    #[test]
    fn reconstruct_matches_read_summary_on_healthy_file() {
        let (buffer, _ids) = healthy_mcap();

        let real = crate::read_summary(io::Cursor::new(&buffer))
            .expect("read summary")
            .expect("summary present");
        let recovered = reconstruct_summary(&buffer).expect("reconstruct");

        // Same chunks in the same order, with matching offsets, time bounds, and message indexes.
        assert_eq!(recovered.chunk_indexes.len(), real.chunk_indexes.len());
        assert_eq!(recovered.chunk_indexes.len(), 3);
        for (got, want) in std::iter::zip(&recovered.chunk_indexes, &real.chunk_indexes) {
            assert_eq!(got.chunk_start_offset, want.chunk_start_offset);
            assert_eq!(got.chunk_length, want.chunk_length);
            assert_eq!(got.message_start_time, want.message_start_time);
            assert_eq!(got.message_end_time, want.message_end_time);
            assert_eq!(got.message_index_offsets, want.message_index_offsets);
        }

        // All three channels resolved (including the late one), each with a non-empty index.
        assert_eq!(recovered.channels.len(), 3);
        for chunk in &recovered.chunk_indexes {
            assert!(!chunk.message_index_offsets.is_empty());
        }

        // The fixture writes three messages to `/a`, two to `/b`, and one to `/c`
        // → six total across three channels.
        let stats = recovered.stats.as_ref().expect("recovered stats");
        assert_eq!(stats.message_count, 6);
        assert_eq!(stats.channel_count, 3);
        assert_eq!(
            stats.channel_message_counts.values().copied().sum::<u64>(),
            6
        );
        assert!(
            stats
                .channel_message_counts
                .values()
                .all(|&count| count > 0)
        );
    }

    /// Runs the per-message decode path with the given summary.
    ///
    /// Static chunks (e.g. the file-level statistics) are dropped, so callers only see that path.
    fn decode(mcap: &[u8], summary: &mcap::Summary) -> Vec<re_chunk::Chunk> {
        use crate::decoders::{DecoderRegistry, TestEmitter, TopicFilter};
        use re_log_types::TimeType;

        let plan = DecoderRegistry::all_with_raw_fallback()
            .plan(mcap, summary, &TopicFilter::default())
            .expect("plan");
        let emitter = TestEmitter::default();
        plan.run(mcap, summary, TimeType::TimestampNs, &*emitter)
            .expect("run");
        emitter
            .finish()
            .into_iter()
            .filter(|chunk| !chunk.is_static())
            .collect()
    }

    #[test]
    fn decode_parity_between_reconstructed_and_real_summary() {
        let (buffer, _ids) = healthy_mcap();

        let real = crate::read_summary(io::Cursor::new(&buffer))
            .expect("read summary")
            .expect("summary present");
        let recovered = reconstruct_summary(&buffer).expect("reconstruct");

        assert_eq!(
            decode(&buffer, &recovered).len(),
            decode(&buffer, &real).len()
        );
    }

    /// Builds a healthy MCAP where both channels appear in all three chunks, so the final chunk has
    /// two message indexes to cut between.
    fn two_channels_per_chunk_mcap() -> Vec<u8> {
        let cursor = io::Cursor::new(Vec::new());
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");
        let id_a = writer
            .add_channel(0, "/a", "raw", &Default::default())
            .expect("add channel a");
        let id_b = writer
            .add_channel(0, "/b", "raw", &Default::default())
            .expect("add channel b");

        for chunk in 0..3 {
            for (offset, channel_id) in [id_a, id_b].into_iter().enumerate() {
                let log_time = chunk * 10 + offset as u64;
                writer
                    .write_to_known_channel(
                        &mcap::records::MessageHeader {
                            channel_id,
                            sequence: 0,
                            log_time,
                            publish_time: log_time,
                        },
                        &[1, 2, 3],
                    )
                    .expect("write message");
            }
            writer.flush().expect("flush chunk");
        }

        writer.finish().expect("finish writer");
        writer.into_inner().into_inner()
    }

    /// A recording interrupted while the final chunk's `MessageIndex` records were being written
    /// still decodes every message of that chunk, because the chunk body itself is complete.
    ///
    // TODO(isaac): a channel whose *only* index record is the truncated one is still stripped from
    // the reconstructed summary, and its messages are lost.
    #[test]
    fn truncated_mid_index_block_keeps_every_message_of_the_complete_chunk() {
        let buffer = two_channels_per_chunk_mcap();

        let real = crate::read_summary(io::Cursor::new(&buffer))
            .expect("read summary")
            .expect("summary present");
        let last = real.chunk_indexes.last().expect("a chunk");

        // Cut one byte into the second `MessageIndex` record of the final chunk, so that the chunk
        // body and the first index record survive.
        let mut index_offsets: Vec<u64> = last.message_index_offsets.values().copied().collect();
        index_offsets.sort_unstable();
        let [first_index, second_index, ..] = index_offsets.as_slice() else {
            panic!("the final chunk needs at least two message indexes to cut between");
        };
        assert!(
            *first_index >= last.chunk_start_offset + last.chunk_length,
            "the index block must follow the chunk body"
        );
        let truncated = &buffer[..=(*second_index as usize)];

        let recovered = reconstruct_summary(truncated).expect("reconstruct");
        assert_eq!(
            recovered.chunk_indexes.len(),
            real.chunk_indexes.len(),
            "every chunk body is complete, so none may be dropped"
        );

        let num_rows = |chunks: Vec<re_chunk::Chunk>| -> u64 {
            chunks.iter().map(|chunk| chunk.num_rows() as u64).sum()
        };
        assert_eq!(
            num_rows(decode(truncated, &recovered)),
            num_rows(decode(&buffer, &real)),
            "no message may be lost just because its `MessageIndex` record was not written"
        );

        // The statistics must agree with what the decode path produced.
        let stats = recovered.stats.as_ref().expect("recovered stats");
        assert_eq!(stats.message_count, num_rows(decode(truncated, &recovered)));
        assert_eq!(
            stats.channel_message_counts,
            real.stats
                .as_ref()
                .expect("real stats")
                .channel_message_counts
        );
    }

    #[test]
    fn truncated_before_summary_recovers_all_chunks() {
        let (buffer, _ids) = healthy_mcap();

        // Truncate at the start of the summary section: keeps the full data section (all chunks +
        // their message indexes + `DataEnd`), drops summary + footer + end magic.
        let footer = mcap::read::footer(&buffer).expect("footer");
        let truncated = &buffer[..footer.summary_start as usize];

        // The real summary reader can no longer parse this.
        assert!(crate::read_summary(io::Cursor::new(truncated)).is_err());

        let recovered = reconstruct_summary(truncated).expect("reconstruct");
        assert_eq!(recovered.chunk_indexes.len(), 3);
        assert_eq!(recovered.channels.len(), 3, "late channel must resolve");
        for chunk in &recovered.chunk_indexes {
            assert!(!chunk.message_index_offsets.is_empty());
        }
    }

    #[test]
    fn truncated_without_recovery_returns_dedicated_error() {
        let (buffer, _ids) = healthy_mcap();
        let footer = mcap::read::footer(&buffer).expect("footer");
        let truncated = &buffer[..footer.summary_start as usize];

        let err = read_or_reconstruct_summary(truncated, false).expect_err("summary must fail");
        let Error::SummaryRequiresRecovery { details } = err else {
            panic!("expected recovery error, got {err}");
        };
        assert_eq!(
            details.to_string(),
            "MCAP file does not end with the expected magic bytes"
        );
    }

    #[test]
    fn invalid_start_magic_returns_error() {
        let err = read_or_reconstruct_summary(b"not an mcap file at all", false)
            .expect_err("invalid start magic must fail");
        assert_matches!(err, Error::MissingStartMagic);
    }

    #[test]
    fn truncated_mid_final_chunk_drops_that_chunk() {
        let (buffer, _ids) = healthy_mcap();

        let real = crate::read_summary(io::Cursor::new(&buffer))
            .expect("read summary")
            .expect("summary present");
        let last = real.chunk_indexes.last().expect("a chunk");
        // Cut a few bytes into the final chunk's record: its declared length now runs past EOF, so
        // the scan drops it (and channel `c`, which only appears in that chunk).
        let cut = last.chunk_start_offset as usize + 15;
        let truncated = &buffer[..cut];

        let recovered = reconstruct_summary(truncated).expect("reconstruct");
        assert_eq!(recovered.chunk_indexes.len(), 2);
        assert_eq!(recovered.channels.len(), 2);
        for chunk in &recovered.chunk_indexes {
            assert!(!chunk.message_index_offsets.is_empty());
        }

        // The dropped channel `c` must not appear in the recovered statistics: only the four
        // messages on the two surviving channels (`/a` and `/b`) are decodable.
        let stats = recovered.stats.as_ref().expect("recovered stats");
        assert_eq!(stats.message_count, 4);
        assert_eq!(stats.channel_message_counts.len(), 2);
        for &id in stats.channel_message_counts.keys() {
            assert!(recovered.channels.contains_key(&id));
        }
    }

    #[test]
    fn truncated_after_final_chunk_before_its_indexes_drops_that_chunk() {
        let (buffer, _ids) = healthy_mcap();

        let real = crate::read_summary(io::Cursor::new(&buffer))
            .expect("read summary")
            .expect("summary present");
        let last = real.chunk_indexes.last().expect("a chunk");
        // Cut right at the end of the final chunk's data, before any of its `MessageIndex` records:
        // the chunk is complete but has no indexes, so it is dropped.
        let cut = (last.chunk_start_offset + last.chunk_length) as usize;
        let truncated = &buffer[..cut];

        let recovered = reconstruct_summary(truncated).expect("reconstruct");
        assert_eq!(recovered.chunk_indexes.len(), 2);
        assert!(
            !recovered
                .chunk_indexes
                .iter()
                .any(|c| c.message_index_offsets.is_empty())
        );
    }

    #[test]
    fn scan_rejects_non_mcap() {
        assert!(build_chunk_index(b"not an mcap file at all").is_err());
    }

    #[test]
    fn reconstruct_rejects_unchunked_file() {
        // A file whose messages live outside chunks cannot be recovered by the index-only scan.
        let cursor = io::Cursor::new(Vec::new());
        let mut writer =
            mcap::Writer::with_options(cursor, mcap::WriteOptions::new().use_chunks(false))
                .expect("writer");
        let channel_id = writer
            .add_channel(0, "/a", "raw", &Default::default())
            .expect("add channel");
        writer
            .write_to_known_channel(
                &mcap::records::MessageHeader {
                    channel_id,
                    sequence: 0,
                    log_time: 1,
                    publish_time: 1,
                },
                &[1, 2, 3],
            )
            .expect("write message");
        writer.finish().expect("finish");
        let buffer = writer.into_inner().into_inner();

        assert!(
            build_chunk_index(&buffer)
                .expect("scan")
                .has_unchunked_messages
        );
        assert!(reconstruct_summary(&buffer).is_err());
    }
}
