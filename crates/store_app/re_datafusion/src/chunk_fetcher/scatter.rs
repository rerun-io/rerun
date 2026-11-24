//! Splits a merged HTTP range body into one buffer per chunk as it is received.
//!
//! Each chunk ends up in its own exactly sized allocation and the bytes between chunks are
//! discarded, so a decoded chunk that aliases its payload pins only its own bytes, never its
//! neighbors or the gaps of the merged range.

use bytes::{BufMut as _, Bytes, BytesMut, buf::Limit};
use re_span::Span;

/// One chunk of a [`RangeLayout`]: `gap` body bytes to discard, then `len` chunk bytes.
struct Segment {
    gap: usize,
    len: usize,
}

/// Where each chunk sits in a merged range body.
///
/// Cheap to turn into a fresh [`ScatterSink`], so a request that fails partway through the
/// body can be retried from the start.
pub struct RangeLayout {
    /// In body order. The first segment has no gap, and there is no gap after the last one.
    segments: Vec<Segment>,

    /// Sum of all gaps and chunk lengths: the exact body length the layout expects.
    total_len: usize,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidLayout {
    /// There are no spans.
    #[error("merged range has no chunks")]
    Empty,

    /// The first span does not start at offset 0 of the merged range.
    #[error("first chunk does not start at the start of the merged range")]
    NotAtStart,

    /// A span starts before the previous one ends: the spans overlap or are not sorted.
    #[error("chunks in the merged range overlap or are not sorted")]
    Overlap,

    /// The merged range does not fit in memory on this platform.
    #[error("merged range is too large to fit in memory")]
    TooLarge,
}

impl RangeLayout {
    /// `spans` are the chunks' byte spans relative to the start of the merged range.
    ///
    /// They must be sorted by start, must not overlap, and the first must start at 0.
    /// The merged range is taken to end where the last span ends.
    pub fn new<'a>(
        mut spans: impl ExactSizeIterator<Item = &'a Span<u64>>,
    ) -> Result<Self, InvalidLayout> {
        let first = spans.next().ok_or(InvalidLayout::Empty)?;
        if first.start != 0 {
            return Err(InvalidLayout::NotAtStart);
        }

        let mut segments = Vec::with_capacity(spans.len() + 1);
        segments.push(Segment {
            gap: 0,
            len: first.len as usize,
        });

        let mut prev_end = first.len;
        for span in spans {
            let gap = span
                .start
                .checked_sub(prev_end)
                .ok_or(InvalidLayout::Overlap)?;
            // These `as` casts are fine because total_len is checked at the end
            segments.push(Segment {
                gap: gap as usize,
                len: span.len as usize,
            });
            prev_end = span.end();
        }

        let total_len = usize::try_from(prev_end).map_err(|_too_large| InvalidLayout::TooLarge)?;

        Ok(Self {
            segments,
            total_len,
        })
    }

    /// The exact number of body bytes a sink from this layout accepts.
    pub const fn total_len(&self) -> usize {
        self.total_len
    }

    /// Fails if a body of `content_length` bytes would run past the last chunk.
    ///
    /// A shorter body is not rejected here: [`ScatterSink::finish`] reports it once it ends.
    pub fn check_content_length(&self, content_length: u64) -> Result<(), TooManyBytes> {
        if content_length > self.total_len as u64 {
            Err(TooManyBytes {
                expected: self.total_len,
            })
        } else {
            Ok(())
        }
    }

    /// A sink that has received no bytes yet.
    pub fn sink(&self) -> ScatterSink<'_> {
        let (first, layout) = self
            .segments
            .split_first()
            .expect("RangeLayout::new rejects empty layouts");
        re_log::debug_assert_eq!(first.gap, 0, "First span starts at 0");
        ScatterSink {
            layout,
            total_len: self.total_len,
            next: Some(ScatterTarget::Chunk(
                BytesMut::with_capacity(first.len).limit(first.len),
            )),
            bufs: Vec::with_capacity(self.segments.len()),
        }
    }
}

/// Receives a merged range body in order and splits it into per-chunk buffers.
///
/// Feed it either through [`Self::write`], for bodies that arrive as frames, or by reading
/// directly into [`Self::target`], then call [`Self::finish`] once the body has ended.
/// Each chunk's buffer is allocated only when the body reaches that chunk.
pub struct ScatterSink<'layout> {
    /// Segments not yet started.
    layout: &'layout [Segment],
    total_len: usize,

    /// The chunk or gap being received. `None` once every chunk is complete.
    next: Option<ScatterTarget>,

    /// Completed chunks, in layout order.
    bufs: Vec<Bytes>,
}

/// The body ended before every chunk was received.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("body ended after {received} of {expected} bytes")]
pub struct UnexpectedEof {
    received: usize,
    expected: usize,
}

/// The body continued past the end of the last chunk.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("body is longer than the {expected} requested bytes")]
pub struct TooManyBytes {
    expected: usize,
}

/// Body bytes between two chunks, which the caller discards.
pub struct Gap {
    /// Gap bytes not yet discarded. Always non-zero when returned from [`ScatterSink::target`].
    remaining: usize,

    /// Length of the chunk that follows the gap.
    chunk_len: usize,
}

impl Gap {
    const fn remaining(&self) -> usize {
        self.remaining
    }

    /// Marks the next `n` body bytes as discarded.
    ///
    /// Panics if `n` exceeds the bytes left in the gap.
    pub fn skip(&mut self, n: usize) {
        assert!(n <= self.remaining);
        self.remaining -= n;
    }
}

/// Where the next body bytes go.
pub enum ScatterTarget {
    /// Read into this, at most up to the end of the current chunk.
    Chunk(Limit<BytesMut>),

    /// The next `n` body bytes are a gap: discard them, then call `skip(n)`.
    Gap(Gap),
}

impl ScatterTarget {
    /// Body bytes this target takes before the next boundary.
    pub fn len(&self) -> usize {
        match self {
            Self::Chunk(limit) => limit.remaining_mut(),
            Self::Gap(gap) => gap.remaining(),
        }
    }
}

impl ScatterSink<'_> {
    /// Where the next body bytes go.
    ///
    /// The returned target is never empty: completed chunks and gaps are retired first.
    /// Returns `None` once every chunk is complete, after which any further body byte is
    /// an error.
    pub fn target(&mut self) -> Option<&mut ScatterTarget> {
        loop {
            match self.next.take() {
                // Retire gap
                Some(ScatterTarget::Gap(Gap {
                    remaining: 0,
                    chunk_len,
                })) => {
                    self.next = Some(ScatterTarget::Chunk(
                        BytesMut::with_capacity(chunk_len).limit(chunk_len),
                    ));
                    if chunk_len > 0 {
                        break;
                    }
                    // Empty chunk: loop again to the next gap
                }
                Some(ScatterTarget::Chunk(limit)) if !limit.has_remaining_mut() => {
                    // Retire chunk
                    let buf = limit.into_inner().freeze();
                    self.bufs.push(buf);

                    // Push the next gap
                    let Some(segment) = self.layout.split_off_first() else {
                        // EOF
                        break;
                    };
                    self.next = Some(ScatterTarget::Gap(Gap {
                        remaining: segment.gap,
                        chunk_len: segment.len,
                    }));
                    if segment.gap > 0 {
                        break;
                    }
                    // Empty gap: loop again to next chunk
                }
                other => {
                    self.next = other;
                    break;
                }
            }
        }
        self.next.as_mut()
    }

    /// Accepts the next `data` bytes of the body, copying chunk bytes and dropping gap bytes.
    ///
    /// On error, the bytes up to the end of the last chunk have been accepted, and the sink
    /// should be discarded.
    pub fn write(&mut self, mut data: &[u8]) -> Result<(), TooManyBytes> {
        while !data.is_empty() {
            let Some(target) = self.target() else {
                return Err(TooManyBytes {
                    expected: self.total_len,
                });
            };
            let len = data.len().min(target.len());
            let src = data.split_off(..len).expect("len <= data.len()");
            match target {
                ScatterTarget::Chunk(limit) => limit.put_slice(src),
                ScatterTarget::Gap(gap) => gap.skip(len),
            }
        }
        Ok(())
    }

    /// One buffer per span passed to [`RangeLayout::new`], in the same order.
    ///
    /// Fails if the body has not been received in full.
    pub fn finish(mut self) -> Result<Vec<Bytes>, UnexpectedEof> {
        // Retires a chunk that was just completed.
        if self.target().is_some() {
            return Err(UnexpectedEof {
                received: self.total_len - self.outstanding(),
                expected: self.total_len,
            });
        }
        Ok(self.bufs)
    }

    /// Body bytes not yet received. Walks the remaining segments, so only call it on error paths.
    fn outstanding(&self) -> usize {
        let current = match &self.next {
            None => 0,
            Some(ScatterTarget::Chunk(limit)) => limit.remaining_mut(),
            Some(ScatterTarget::Gap(gap)) => gap.remaining + gap.chunk_len,
        };
        let rest: usize = self.layout.iter().map(|s| s.gap + s.len).sum();
        current + rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP_BYTE: u8 = 0xEE;

    fn spans(spans: &[(u64, u64)]) -> Vec<Span<u64>> {
        spans
            .iter()
            .map(|&(start, len)| Span::from_start_len(start, len))
            .collect()
    }

    fn layout(raw: &[(u64, u64)]) -> RangeLayout {
        RangeLayout::new(spans(raw).iter()).expect("valid layout")
    }

    /// A body for `raw` where gap bytes are `GAP_BYTE` and each chunk byte is unique-ish,
    /// together with the chunks the sink should produce.
    fn body_and_chunks(raw: &[(u64, u64)]) -> (Vec<u8>, Vec<Vec<u8>>) {
        let total = raw.last().map_or(0, |&(start, len)| start + len) as usize;
        let mut body = vec![GAP_BYTE; total];
        let mut chunks = Vec::new();
        for (i, &(start, len)) in raw.iter().enumerate() {
            let chunk: Vec<u8> = (0..len)
                .map(|j| (i as u8).wrapping_mul(31) ^ j as u8)
                .collect();
            body[start as usize..(start + len) as usize].copy_from_slice(&chunk);
            chunks.push(chunk);
        }
        (body, chunks)
    }

    fn as_vecs(bufs: Vec<Bytes>) -> Vec<Vec<u8>> {
        bufs.into_iter().map(|b| b.to_vec()).collect()
    }

    /// Gaps between chunks, adjacent chunks, and zero-length chunks at the start, middle and end.
    const LAYOUTS: &[&[(u64, u64)]] = &[
        &[(0, 10)],
        &[(0, 4), (7, 5)],
        &[(0, 3), (3, 3), (10, 2)],
        &[(0, 0), (2, 3)],
        &[(0, 2), (5, 0), (8, 3)],
        &[(0, 2), (2, 0)],
        &[(0, 3), (6, 0)],
    ];

    #[test]
    fn rejects_invalid_layouts() {
        assert_eq!(
            RangeLayout::new([].iter()).err(),
            Some(InvalidLayout::Empty)
        );
        assert_eq!(
            RangeLayout::new(spans(&[(1, 3)]).iter()).err(),
            Some(InvalidLayout::NotAtStart)
        );
        assert_eq!(
            RangeLayout::new(spans(&[(0, 4), (3, 2)]).iter()).err(),
            Some(InvalidLayout::Overlap)
        );
        assert_eq!(
            RangeLayout::new(spans(&[(0, 2), (6, 2), (3, 2)]).iter()).err(),
            Some(InvalidLayout::Overlap)
        );
    }

    #[test]
    fn total_len() {
        let layout = layout(&[(0, 4), (7, 5), (13, 1)]);
        assert_eq!(layout.total_len(), 14);
    }

    #[test]
    fn content_length_past_the_last_chunk_is_rejected() {
        let layout = layout(&[(0, 4), (7, 5)]);
        assert_eq!(layout.check_content_length(0), Ok(()));
        assert_eq!(layout.check_content_length(12), Ok(()));
        assert_eq!(
            layout.check_content_length(13),
            Err(TooManyBytes { expected: 12 })
        );
    }

    #[test]
    fn single_write_splits_chunks_and_drops_gaps() {
        for raw in LAYOUTS {
            let layout = layout(raw);
            let (body, chunks) = body_and_chunks(raw);
            let mut sink = layout.sink();
            sink.write(&body).unwrap();
            assert_eq!(as_vecs(sink.finish().unwrap()), chunks, "layout {raw:?}");
        }
    }

    #[test]
    fn any_frame_boundary_gives_the_same_chunks() {
        for raw in LAYOUTS {
            let layout = layout(raw);
            let (body, chunks) = body_and_chunks(raw);

            for split in 0..=body.len() {
                let mut sink = layout.sink();
                sink.write(&body[..split]).unwrap();
                sink.write(&body[split..]).unwrap();
                assert_eq!(
                    as_vecs(sink.finish().unwrap()),
                    chunks,
                    "layout {raw:?}, split at {split}"
                );
            }

            let mut sink = layout.sink();
            for byte in &body {
                sink.write(std::slice::from_ref(byte)).unwrap();
            }
            assert_eq!(
                as_vecs(sink.finish().unwrap()),
                chunks,
                "layout {raw:?}, byte by byte"
            );
        }
    }

    #[test]
    fn empty_writes_are_accepted() {
        let raw = &[(0, 2), (4, 2)];
        let (body, chunks) = body_and_chunks(raw);
        let layout = layout(raw);
        let mut sink = layout.sink();
        sink.write(&[]).unwrap();
        sink.write(&body).unwrap();
        sink.write(&[]).unwrap();
        assert_eq!(as_vecs(sink.finish().unwrap()), chunks);
    }

    /// Drives the sink the way a reader that fills buffers in place does.
    #[test]
    fn reading_in_place_through_target() {
        for raw in LAYOUTS {
            let layout = layout(raw);
            let (body, chunks) = body_and_chunks(raw);
            let mut remaining: &[u8] = &body;
            let mut sink = layout.sink();

            // At most 3 bytes per "read", to cross boundaries mid-read.
            while let Some(target) = sink.target() {
                let n = target.len().min(3).min(remaining.len());
                assert!(n > 0, "target must never be empty");
                let read = remaining.split_off(..n).unwrap();
                match target {
                    ScatterTarget::Chunk(limit) => limit.put_slice(read),
                    ScatterTarget::Gap(gap) => {
                        assert!(read.iter().all(|&b| b == GAP_BYTE));
                        gap.skip(n);
                    }
                }
            }

            assert!(remaining.is_empty());
            assert_eq!(as_vecs(sink.finish().unwrap()), chunks, "layout {raw:?}");
        }
    }

    #[test]
    fn bytes_past_the_end_are_rejected() {
        let raw = &[(0, 2), (4, 2)];
        let (mut body, _) = body_and_chunks(raw);
        body.push(0);
        let layout = layout(raw);

        let mut sink = layout.sink();
        assert_eq!(sink.write(&body), Err(TooManyBytes { expected: 6 }));

        let mut sink = layout.sink();
        sink.write(&body[..6]).unwrap();
        assert_eq!(sink.write(&body[6..]), Err(TooManyBytes { expected: 6 }));
    }

    #[test]
    fn short_body_reports_how_much_was_received() {
        let raw = &[(0, 4), (6, 4)];
        let (body, _) = body_and_chunks(raw);
        let layout = layout(raw);

        // Ends inside the first chunk, inside the gap, at the gap's end, and inside the last chunk.
        for received in [0, 2, 4, 5, 6, 9] {
            let mut sink = layout.sink();
            sink.write(&body[..received]).unwrap();
            assert_eq!(
                sink.finish(),
                Err(UnexpectedEof {
                    received,
                    expected: 10
                }),
                "received {received}"
            );
        }
    }

    #[test]
    fn sinks_from_one_layout_are_independent() {
        let raw = &[(0, 3), (5, 3)];
        let (body, chunks) = body_and_chunks(raw);
        let layout = layout(raw);

        let mut abandoned = layout.sink();
        abandoned.write(&body[..4]).unwrap();

        let mut retry = layout.sink();
        retry.write(&body).unwrap();
        assert_eq!(as_vecs(retry.finish().unwrap()), chunks);
    }
}
