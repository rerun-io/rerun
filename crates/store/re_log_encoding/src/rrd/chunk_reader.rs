use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::sync::Arc;

use re_chunk::{Chunk, ChunkId};
use re_span::Span;

use crate::RrdManifest;
use crate::ToApplication as _;
use crate::rrd::CodecError;

/// Maximum gap between two chunk spans that will still be merged into a single I/O read.
/// Spans separated by more than this are read independently.
const MERGE_GAP_BYTES: u64 = 64 * 1024; // 64 KiB

/// Read chunks from an open RRD file by their IDs, using byte offsets from the manifest.
///
/// Internally sorts requested chunks by byte offset for sequential I/O,
/// and merges adjacent/nearby spans (within 64 kB) into single reads.
///
/// Returns [`CodecError::ChunkNotInManifest`] if any chunk ID is not in the manifest.
/// Aborts on first error (no partial results).
pub fn read_chunks(
    file: &mut File,
    manifest: &RrdManifest,
    chunk_ids: &[ChunkId],
) -> Result<Vec<Arc<Chunk>>, CodecError> {
    if chunk_ids.is_empty() {
        return Ok(Vec::new());
    }

    let all_ids = manifest.col_chunk_ids();
    let offsets = manifest.col_chunk_byte_offset();
    let sizes = manifest.col_chunk_byte_size();

    // Build a temporary lookup for the manifest's chunk IDs.
    let id_to_row: std::collections::HashMap<ChunkId, usize> =
        all_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();

    // Resolve chunk IDs to (chunk_id, byte_span).
    let mut entries: Vec<(ChunkId, Span<u64>)> = chunk_ids
        .iter()
        .map(|&id| -> Result<_, CodecError> {
            let &row = id_to_row
                .get(&id)
                .ok_or(CodecError::ChunkNotInManifest { chunk_id: id })?;
            Ok((
                id,
                Span {
                    start: offsets[row],
                    len: sizes[row],
                },
            ))
        })
        .collect::<Result<_, _>>()?;

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    // Sort by offset for sequential I/O.
    entries.sort_by_key(|&(_, span)| span.start);

    // Merge nearby spans into coalesced reads.
    let groups = coalesce_spans(&entries);

    let mut result = Vec::with_capacity(entries.len());

    for group in &groups {
        // Read the entire merged span in one I/O call.
        file.seek(SeekFrom::Start(group.byte_span.start))?;
        let mut buf = vec![0u8; usize::try_from(group.byte_span.len)?];
        file.read_exact(&mut buf)?;

        // Slice out individual chunks and decode them.
        for &(_chunk_id, chunk_span) in &entries[group.entry_range.clone()] {
            let local_span = Span {
                start: usize::try_from(chunk_span.start - group.byte_span.start)?,
                len: usize::try_from(chunk_span.len)?,
            };
            let chunk = decode_chunk_from_bytes(&buf[local_span.range()])?;
            result.push(Arc::new(chunk));
        }
    }

    Ok(result)
}

/// A contiguous byte range covering one or more chunk spans.
struct CoalescedSpan {
    byte_span: Span<u64>,

    /// Which entries (index range into the sorted entries slice) this span covers.
    entry_range: std::ops::Range<usize>,
}

/// Merge chunk spans that are adjacent or within [`MERGE_GAP_BYTES`] of each other.
/// Input must be sorted by offset.
fn coalesce_spans(entries: &[(ChunkId, Span<u64>)]) -> Vec<CoalescedSpan> {
    let mut groups: Vec<CoalescedSpan> = Vec::new();

    for (i, &(_id, span)) in entries.iter().enumerate() {
        if let Some(last) = groups.last_mut() {
            let last_end = last.byte_span.end();
            if span.start <= last_end + MERGE_GAP_BYTES {
                // Extend the current group.
                last.byte_span.len = span.end().max(last_end) - last.byte_span.start;
                last.entry_range.end = i + 1;
                continue;
            }
        }
        // Start a new group.
        groups.push(CoalescedSpan {
            byte_span: span,
            entry_range: i..i + 1,
        });
    }

    groups
}

/// Decode a chunk from raw protobuf `ArrowMsg` bytes.
fn decode_chunk_from_bytes(buf: &[u8]) -> Result<Chunk, CodecError> {
    use crate::rrd::Decodable as _;

    let transport_arrow_msg = re_protos::log_msg::v1alpha1::ArrowMsg::from_rrd_bytes(buf)?;
    let app_arrow_msg = transport_arrow_msg.to_application(())?;
    let chunk = Chunk::from_arrow_msg(&app_arrow_msg)?;
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rrd::test_util::{
        encode_test_rrd, encode_test_rrd_to_file_with_options, make_test_chunks,
    };

    #[test]
    fn test_read_chunks_roundtrip() {
        let chunks = make_test_chunks(5);
        let (rrd, store_id) = encode_test_rrd(&chunks);
        let mut file = File::open(rrd.path()).unwrap();

        let footer = crate::read_rrd_footer(&mut file).unwrap().unwrap();
        let raw_manifest = &footer.manifests[&store_id];
        let manifest = RrdManifest::try_new(raw_manifest).unwrap();

        let chunk_ids = manifest.col_chunk_ids();
        assert_eq!(chunk_ids.len(), chunks.len());

        // Read all chunks.
        let loaded = read_chunks(&mut file, &manifest, chunk_ids).unwrap();
        assert_eq!(loaded.len(), chunks.len());

        for (i, loaded_chunk) in loaded.iter().enumerate() {
            assert_eq!(loaded_chunk.entity_path(), chunks[i].entity_path());
            assert_eq!(loaded_chunk.num_rows(), chunks[i].num_rows());
        }
    }

    #[test]
    fn test_read_chunks_subset() {
        let chunks = make_test_chunks(5);
        let (rrd, store_id) = encode_test_rrd(&chunks);
        let mut file = File::open(rrd.path()).unwrap();

        let footer = crate::read_rrd_footer(&mut file).unwrap().unwrap();
        let raw_manifest = &footer.manifests[&store_id];
        let manifest = RrdManifest::try_new(raw_manifest).unwrap();

        // Read only the first and last chunk.
        let chunk_ids = manifest.col_chunk_ids();
        let subset = [chunk_ids[0], chunk_ids[chunk_ids.len() - 1]];
        let loaded = read_chunks(&mut file, &manifest, &subset).unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_read_chunks_unknown_id_errors() {
        let chunks = make_test_chunks(3);
        let (rrd, store_id) = encode_test_rrd(&chunks);
        let mut file = File::open(rrd.path()).unwrap();

        let footer = crate::read_rrd_footer(&mut file).unwrap().unwrap();
        let raw_manifest = &footer.manifests[&store_id];
        let manifest = RrdManifest::try_new(raw_manifest).unwrap();

        let bogus_id = ChunkId::new();
        let result = read_chunks(&mut file, &manifest, &[bogus_id]);
        assert!(
            matches!(result, Err(crate::CodecError::ChunkNotInManifest { .. })),
            "Expected ChunkNotInManifest error, got: {result:?}"
        );
    }

    /// Shorthand for tests.
    fn span(start: u64, len: u64) -> Span<u64> {
        Span { start, len }
    }

    #[test]
    fn test_coalesce_spans_single_group() {
        // All entries within MERGE_GAP_BYTES of each other → one group.
        let entries = vec![
            (ChunkId::new(), span(100, 50)),
            (ChunkId::new(), span(150, 50)), // adjacent
            (ChunkId::new(), span(200, 50)), // adjacent
        ];
        let groups = coalesce_spans(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].byte_span, span(100, 150)); // 100..250
        assert_eq!(groups[0].entry_range, 0..3);
    }

    #[test]
    fn test_coalesce_spans_multiple_groups() {
        // Two clusters separated by more than MERGE_GAP_BYTES.
        let gap = MERGE_GAP_BYTES + 1;
        let entries = vec![
            (ChunkId::new(), span(0, 100)),
            (ChunkId::new(), span(100, 100)), // adjacent to first → same group
            (ChunkId::new(), span(200 + gap, 100)), // far from second → new group
            (ChunkId::new(), span(200 + gap + 100, 50)), // adjacent to third → same group
        ];
        let groups = coalesce_spans(&entries);
        assert_eq!(groups.len(), 2);

        assert_eq!(groups[0].byte_span, span(0, 200)); // 0..200
        assert_eq!(groups[0].entry_range, 0..2);

        assert_eq!(groups[1].byte_span, span(200 + gap, 150)); // (200+gap)..(200+gap+150)
        assert_eq!(groups[1].entry_range, 2..4);
    }

    #[test]
    fn test_coalesce_spans_merge_gap_boundary() {
        // Exactly at MERGE_GAP_BYTES → should still merge.
        let entries = vec![
            (ChunkId::new(), span(0, 100)),
            (ChunkId::new(), span(100 + MERGE_GAP_BYTES, 50)), // gap == MERGE_GAP_BYTES → merged
        ];
        let groups = coalesce_spans(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].byte_span, span(0, 100 + MERGE_GAP_BYTES + 50));
        assert_eq!(groups[0].entry_range, 0..2);

        // One byte beyond → separate groups.
        let entries = vec![
            (ChunkId::new(), span(0, 100)),
            (ChunkId::new(), span(100 + MERGE_GAP_BYTES + 1, 50)),
        ];
        let groups = coalesce_spans(&entries);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_coalesce_spans_empty() {
        let entries: Vec<(ChunkId, Span<u64>)> = vec![];
        let groups = coalesce_spans(&entries);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_read_chunks_uncompressed() {
        let chunks = make_test_chunks(3);
        let rrd = tempfile::NamedTempFile::new().unwrap();

        let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test");
        encode_test_rrd_to_file_with_options(
            rrd.path(),
            &chunks,
            &store_id,
            true,
            crate::EncodingOptions::PROTOBUF_UNCOMPRESSED,
        );

        let mut file = File::open(rrd.path()).unwrap();
        let footer = crate::read_rrd_footer(&mut file).unwrap().unwrap();
        let raw_manifest = &footer.manifests[&store_id];
        let manifest = RrdManifest::try_new(raw_manifest).unwrap();

        let loaded = read_chunks(&mut file, &manifest, manifest.col_chunk_ids()).unwrap();
        assert_eq!(loaded.len(), chunks.len());

        for (i, loaded_chunk) in loaded.iter().enumerate() {
            assert_eq!(loaded_chunk.entity_path(), chunks[i].entity_path());
            assert_eq!(loaded_chunk.num_rows(), chunks[i].num_rows());
        }
    }
}
