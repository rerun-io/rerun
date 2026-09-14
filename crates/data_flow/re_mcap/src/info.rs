//! Structured information about an MCAP file.

use std::collections::BTreeMap;

use crate::Error;

/// Whether the summary backing an [`McapInfo`] came from the file or was reconstructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McapSummarySource {
    /// The file contained a readable MCAP summary section.
    Embedded,

    /// The summary was reconstructed from the readable portion of the file.
    Reconstructed,
}

/// Header and summary information about an MCAP file.
#[derive(Debug, Clone, PartialEq)]
pub struct McapInfo {
    /// MCAP profile declared by the file header.
    pub profile: String,

    /// Writer library declared by the file header.
    pub library: String,

    /// Total number of messages, when available from statistics or message indexes.
    pub message_count: Option<u64>,

    /// Inclusive minimum message log time in nanoseconds.
    pub message_start_time_ns: Option<u64>,

    /// Inclusive maximum message log time in nanoseconds.
    pub message_end_time_ns: Option<u64>,

    /// `message_end_time_ns - message_start_time_ns`, when both bounds are available.
    pub duration_ns: Option<u64>,

    /// Number of schemas present in the parsed summary.
    pub schema_count: usize,

    /// Number of channels present in the parsed summary.
    pub channel_count: usize,

    /// Number of attachment indexes present in the parsed summary.
    pub attachment_count: usize,

    /// Number of metadata indexes present in the parsed summary.
    pub metadata_count: usize,

    /// Whether the file contained an MCAP statistics record.
    pub statistics_present: bool,

    /// Whether the parsed summary was embedded or reconstructed.
    pub summary_source: McapSummarySource,

    /// Aggregate information about indexed MCAP chunks.
    pub chunks: McapChunkInfo,

    /// Compression aggregates grouped by the exact codec identifier stored in chunk indexes.
    pub compression: Vec<McapCompressionInfo>,

    /// Channels ordered by channel ID.
    pub channels: Vec<McapChannelInfo>,
}

/// Aggregate information about indexed MCAP chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McapChunkInfo {
    /// Number of indexed chunks.
    pub count: usize,

    /// Largest uncompressed chunk size in bytes.
    pub max_uncompressed_size_bytes: Option<u64>,

    /// Largest compressed chunk size in bytes.
    pub max_compressed_size_bytes: Option<u64>,

    /// Whether a chunk starts before the maximum end time of an earlier chunk.
    ///
    /// Equal boundary timestamps are not considered overlapping.
    pub has_overlapping_time_ranges: bool,
}

/// Aggregate information for one MCAP chunk compression codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McapCompressionInfo {
    /// Exact codec identifier from MCAP chunk indexes.
    ///
    /// The empty string is the MCAP representation for uncompressed chunks.
    pub codec: String,

    /// Number of chunks using this codec.
    pub chunk_count: usize,

    /// Sum of compressed chunk sizes in bytes.
    pub compressed_size_bytes: u64,

    /// Sum of uncompressed chunk sizes in bytes.
    pub uncompressed_size_bytes: u64,
}

impl McapCompressionInfo {
    /// Fraction of uncompressed bytes removed by compression.
    ///
    /// Returns `None` when the uncompressed size is zero.
    pub fn savings_ratio(&self) -> Option<f64> {
        (self.uncompressed_size_bytes > 0)
            .then(|| 1.0 - self.compressed_size_bytes as f64 / self.uncompressed_size_bytes as f64)
    }
}

/// Structured information about one MCAP channel.
#[derive(Debug, Clone, PartialEq)]
pub struct McapChannelInfo {
    /// MCAP channel ID.
    pub id: u16,

    /// Topic name declared by the channel.
    pub topic: String,

    /// Encoding used for message payloads on this channel.
    pub message_encoding: String,

    /// Channel metadata copied from the summary.
    pub metadata: BTreeMap<String, String>,

    /// Schema descriptor, when the channel references a schema.
    pub schema: Option<McapSchemaInfo>,

    /// Number of messages on the channel, when available.
    pub message_count: Option<u64>,

    /// Estimated minimum and maximum message frequency in hertz.
    ///
    /// The estimate follows the MCAP CLI convention: `(count - 1) / duration` through
    /// `count / duration`, using the recording-wide duration.
    /// It is unavailable for fewer than two messages or a zero-duration recording.
    pub frequency_hz: Option<(f64, f64)>,
}

/// Schema metadata referenced by an MCAP channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McapSchemaInfo {
    /// MCAP schema ID.
    pub id: u16,

    /// Schema name.
    pub name: String,

    /// Schema encoding.
    pub encoding: String,

    /// Size of the schema payload in bytes.
    pub data_size_bytes: usize,
}

/// Parse the MCAP header record without scanning message or chunk payloads.
pub(crate) fn read_header(mcap: &[u8]) -> Result<mcap::records::Header, Error> {
    if !mcap.starts_with(mcap::MAGIC) {
        return Err(mcap::McapError::BadMagic.into());
    }

    let record_start = mcap::MAGIC.len();
    let prefix_end = record_start + crate::RECORD_HEADER_LEN;
    let Some(prefix) = mcap.get(record_start..prefix_end) else {
        return Err(mcap::McapError::UnexpectedEof.into());
    };
    let opcode = prefix[0];
    let record_len = u64::from_le_bytes(prefix[1..].try_into().expect("eight-byte record length"));
    let record_len = usize::try_from(record_len)
        .map_err(|_err| Error::other(anyhow::anyhow!("MCAP header record is too large")))?;
    let body_end = prefix_end
        .checked_add(record_len)
        .ok_or_else(|| Error::other(anyhow::anyhow!("MCAP header record length overflow")))?;
    let Some(body) = mcap.get(prefix_end..body_end) else {
        return Err(mcap::McapError::UnexpectedEof.into());
    };

    match mcap::parse_record(opcode, body)? {
        mcap::records::Record::Header(header) => Ok(header),
        record => Err(Error::other(anyhow::anyhow!(
            "Expected MCAP header as the first record, found opcode 0x{:02x}",
            record.opcode()
        ))),
    }
}

impl McapInfo {
    /// Build [`McapInfo`] from an MCAP header and summary.
    ///
    /// `mcap` must be the byte source from which `summary` was read.
    /// Bytes are only consulted when the optional MCAP statistics record is absent and message
    /// counts can be recovered from message indexes without decompressing chunks.
    pub(crate) fn from_summary(
        header: &mcap::records::Header,
        summary: &mcap::Summary,
        mcap: &[u8],
        summary_source: McapSummarySource,
    ) -> Self {
        let statistics_present =
            summary_source == McapSummarySource::Embedded && summary.stats.is_some();
        let channel_message_counts = channel_message_counts(summary, mcap);

        let message_count = summary
            .stats
            .as_ref()
            .map(|stats| stats.message_count)
            .or_else(|| {
                channel_message_counts
                    .as_ref()
                    .map(|counts| counts.values().copied().sum())
            });

        let (message_start_time_ns, message_end_time_ns) = time_bounds(summary, message_count);
        let duration_ns = Option::zip(message_start_time_ns, message_end_time_ns)
            .map(|(start, end)| end.saturating_sub(start));

        let mut channels = summary
            .channels
            .values()
            .map(|channel| {
                let message_count = channel_message_counts
                    .as_ref()
                    .and_then(|counts| counts.get(&channel.id).copied());
                let frequency_hz = Option::zip(message_count, duration_ns)
                    .and_then(|(count, duration)| frequency_hz(count, duration));
                McapChannelInfo {
                    id: channel.id,
                    topic: channel.topic.clone(),
                    message_encoding: channel.message_encoding.clone(),
                    metadata: channel.metadata.clone(),
                    schema: channel.schema.as_ref().map(|schema| McapSchemaInfo {
                        id: schema.id,
                        name: schema.name.clone(),
                        encoding: schema.encoding.clone(),
                        data_size_bytes: schema.data.len(),
                    }),
                    message_count,
                    frequency_hz,
                }
            })
            .collect::<Vec<_>>();
        channels.sort_by_key(|channel| channel.id);

        Self {
            profile: header.profile.clone(),
            library: header.library.clone(),
            message_count,
            message_start_time_ns,
            message_end_time_ns,
            duration_ns,
            schema_count: summary.schemas.len(),
            channel_count: summary.channels.len(),
            attachment_count: summary.attachment_indexes.len(),
            metadata_count: summary.metadata_indexes.len(),
            statistics_present,
            summary_source,
            chunks: chunk_info(summary),
            compression: compression_info(summary),
            channels,
        }
    }
}

/// Returns per-channel message counts from statistics or message indexes.
fn channel_message_counts(summary: &mcap::Summary, mcap: &[u8]) -> Option<BTreeMap<u16, u64>> {
    if let Some(stats) = &summary.stats {
        let mut counts = summary
            .channels
            .keys()
            .copied()
            .map(|id| (id, 0))
            .collect::<BTreeMap<_, _>>();
        counts.extend(&stats.channel_message_counts);
        return Some(counts);
    }

    if summary.chunk_indexes.is_empty() {
        return None;
    }

    let mut counts = summary
        .channels
        .keys()
        .copied()
        .map(|id| (id, 0))
        .collect::<BTreeMap<_, _>>();
    for chunk in &summary.chunk_indexes {
        let indexes = summary.read_message_indexes(mcap, chunk).ok()?;
        for (channel, messages) in indexes {
            *counts.entry(channel.id).or_default() += messages.len() as u64;
        }
    }
    Some(counts)
}

/// Returns inclusive message log-time bounds from statistics or chunk indexes.
fn time_bounds(summary: &mcap::Summary, message_count: Option<u64>) -> (Option<u64>, Option<u64>) {
    if message_count == Some(0) {
        return (None, None);
    }

    if let Some(stats) = &summary.stats
        && stats.message_count > 0
    {
        return (Some(stats.message_start_time), Some(stats.message_end_time));
    }

    let start = summary
        .chunk_indexes
        .iter()
        .map(|chunk| chunk.message_start_time)
        .min();
    let end = summary
        .chunk_indexes
        .iter()
        .map(|chunk| chunk.message_end_time)
        .max();
    (start, end)
}

/// Estimates the minimum and maximum whole-recording message frequency.
fn frequency_hz(message_count: u64, duration_ns: u64) -> Option<(f64, f64)> {
    if message_count < 2 || duration_ns == 0 {
        return None;
    }

    let duration_seconds = duration_ns as f64 / 1_000_000_000.0;
    Some((
        (message_count - 1) as f64 / duration_seconds,
        message_count as f64 / duration_seconds,
    ))
}

/// Aggregates chunk sizes and detects overlapping chunk time ranges.
fn chunk_info(summary: &mcap::Summary) -> McapChunkInfo {
    let max_uncompressed_size_bytes = summary
        .chunk_indexes
        .iter()
        .map(|chunk| chunk.uncompressed_size)
        .max();
    let max_compressed_size_bytes = summary
        .chunk_indexes
        .iter()
        .map(|chunk| chunk.compressed_size)
        .max();

    let mut chunks = summary.chunk_indexes.iter().collect::<Vec<_>>();
    chunks.sort_by_key(|chunk| chunk.message_start_time);
    let mut running_end = None;
    let mut has_overlapping_time_ranges = false;
    for chunk in chunks {
        if running_end.is_some_and(|end| chunk.message_start_time < end) {
            has_overlapping_time_ranges = true;
            break;
        }
        running_end = Some(running_end.map_or(chunk.message_end_time, |end: u64| {
            end.max(chunk.message_end_time)
        }));
    }

    McapChunkInfo {
        count: summary.chunk_indexes.len(),
        max_uncompressed_size_bytes,
        max_compressed_size_bytes,
        has_overlapping_time_ranges,
    }
}

/// Aggregates chunk sizes by exact compression codec identifier.
fn compression_info(summary: &mcap::Summary) -> Vec<McapCompressionInfo> {
    let mut by_codec: BTreeMap<String, McapCompressionInfo> = BTreeMap::new();
    for chunk in &summary.chunk_indexes {
        let info = by_codec
            .entry(chunk.compression.clone())
            .or_insert_with(|| McapCompressionInfo {
                codec: chunk.compression.clone(),
                chunk_count: 0,
                compressed_size_bytes: 0,
                uncompressed_size_bytes: 0,
            });
        info.chunk_count += 1;
        info.compressed_size_bytes = info
            .compressed_size_bytes
            .saturating_add(chunk.compressed_size);
        info.uncompressed_size_bytes = info
            .uncompressed_size_bytes
            .saturating_add(chunk.uncompressed_size);
    }
    by_codec.into_values().collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use mcap::records::MessageHeader;

    use super::*;

    /// Builds an MCAP fixture containing one populated and one empty channel.
    fn fixture() -> Vec<u8> {
        let mut writer = mcap::Writer::with_options(
            Cursor::new(Vec::new()),
            mcap::WriteOptions::new()
                .profile("test-profile")
                .library("test-library"),
        )
        .expect("create writer");
        let schema_id = writer
            .add_schema("example.Message", "protobuf", b"schema")
            .expect("add schema");
        let channel_id = writer
            .add_channel(schema_id, "/example", "protobuf", &BTreeMap::new())
            .expect("add channel");
        writer
            .add_channel(schema_id, "/empty", "protobuf", &BTreeMap::new())
            .expect("add empty channel");
        for (sequence, time) in [1_000_000_000, 2_000_000_000, 3_000_000_000]
            .into_iter()
            .enumerate()
        {
            writer
                .write_to_known_channel(
                    &MessageHeader {
                        channel_id,
                        sequence: sequence as u32,
                        log_time: time,
                        publish_time: time,
                    },
                    b"message",
                )
                .expect("write message");
        }
        writer.finish().expect("finish writer");
        writer.into_inner().into_inner()
    }

    /// Tests file, channel, schema, time, and frequency information derived from a summary.
    #[test]
    fn info_is_derived_from_header_and_summary() {
        let bytes = fixture();
        let header = read_header(&bytes).expect("read header");
        let summary = mcap::Summary::read(&bytes)
            .expect("read summary")
            .expect("summary present");
        let info = McapInfo::from_summary(&header, &summary, &bytes, McapSummarySource::Embedded);

        assert_eq!(info.profile, "test-profile");
        assert_eq!(info.library, "test-library");
        assert_eq!(info.message_count, Some(3));
        assert_eq!(info.message_start_time_ns, Some(1_000_000_000));
        assert_eq!(info.message_end_time_ns, Some(3_000_000_000));
        assert_eq!(info.duration_ns, Some(2_000_000_000));
        assert_eq!(info.schema_count, 1);
        assert_eq!(info.channel_count, 2);
        let example = info
            .channels
            .iter()
            .find(|channel| channel.topic == "/example")
            .expect("example channel");
        let empty = info
            .channels
            .iter()
            .find(|channel| channel.topic == "/empty")
            .expect("empty channel");
        assert_eq!(example.message_count, Some(3));
        assert_eq!(example.frequency_hz, Some((1.0, 1.5)));
        assert_eq!(empty.message_count, Some(0));
        assert_eq!(empty.frequency_hz, None);
        assert_eq!(
            example.schema.as_ref().map(|schema| schema.name.as_str()),
            Some("example.Message")
        );
    }

    /// Tests that an empty recording reports zero messages without time or chunk-size bounds.
    #[test]
    fn empty_file_has_no_time_bounds() {
        let mut writer = mcap::Writer::with_options(
            Cursor::new(Vec::new()),
            mcap::WriteOptions::new()
                .profile("empty")
                .library("test-library"),
        )
        .expect("create writer");
        writer.finish().expect("finish writer");
        let bytes = writer.into_inner().into_inner();
        let header = read_header(&bytes).expect("read header");
        let summary = mcap::Summary::read(&bytes)
            .expect("read summary")
            .expect("summary present");
        let info = McapInfo::from_summary(&header, &summary, &bytes, McapSummarySource::Embedded);

        assert_eq!(info.message_count, Some(0));
        assert_eq!(info.message_start_time_ns, None);
        assert_eq!(info.message_end_time_ns, None);
        assert_eq!(info.duration_ns, None);
        assert_eq!(info.chunks.max_compressed_size_bytes, None);
    }
}
