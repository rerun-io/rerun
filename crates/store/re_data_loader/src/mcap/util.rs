use std::io::{Read, Seek};

use anyhow::Context as _;
use mcap::{
    Summary,
    records::ChunkIndex,
    sans_io::{SummaryReadEvent, SummaryReader},
};
use re_chunk::external::nohash_hasher::IntMap;

use crate::mcap::decode::ChannelId;

/// Read out the summary of an MCAP file.
pub fn read_summary<R: Read + Seek>(mut reader: R) -> anyhow::Result<Option<Summary>> {
    let mut summary_reader = SummaryReader::new();
    while let Some(event) = summary_reader.next_event() {
        match event? {
            SummaryReadEvent::SeekRequest(pos) => {
                summary_reader.notify_seeked(reader.seek(pos)?);
            }
            SummaryReadEvent::ReadRequest(need) => {
                let read = reader.read(summary_reader.insert(need))?;
                summary_reader.notify_read(read);
            }
        }
    }

    Ok(summary_reader.finish())
}

/// Counts the number of messages per channel within a specific chunk.
///
/// This function reads the message indexes for the given chunk and returns
/// a mapping of channel IDs to their respective message counts.
#[inline]
pub fn get_chunk_message_count(
    chunk_index: &ChunkIndex,
    summary: &Summary,
    mcap: &[u8],
) -> anyhow::Result<IntMap<ChannelId, usize>> {
    Ok(summary
        .read_message_indexes(mcap, chunk_index)
        .with_context(|| "Failed to read message indexes for chunk")?
        .iter()
        .map(|(channel, msg_offsets)| (channel.id.into(), msg_offsets.len()))
        .collect())
}
