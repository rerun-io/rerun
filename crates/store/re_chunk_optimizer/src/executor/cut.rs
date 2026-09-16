//! Cutting an oversized chunk into pieces that pack with their neighbors.

use std::sync::Arc;

use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, Span};

use super::size_estimate::estimate_cumulative_row_bytes;

/// What one piece may hold.
#[derive(Clone)]
pub struct Budget {
    /// Bytes the piece is cut to, against its estimated row bytes (see
    /// [`estimate_cumulative_row_bytes`]). Whether the chunk's fixed cost comes out of it depends
    /// on the piece: a room piece sets it aside, a full piece leaves it to the slack band.
    pub max_row_bytes: u64,

    /// Rows the piece may hold.
    pub max_rows: u64,

    /// What the sliced piece may measure: the room the accumulator has left, or the slack band
    /// for a piece that seeds an output of its own.
    pub max_measured_bytes: u64,
}

/// Cut `chunk` into pieces: the first under `room` when there is one (e.g. to top up a partially
/// filled accumulator), every following one under `full`, the last being the remainder.
///
/// Returns `None` when no cut makes progress: the chunk has one row, or the cut takes it whole.
pub fn cut_to_fit(
    chunk: &Chunk,
    total_chunk_bytes: u64,
    mut room_budget: Option<Budget>,
    full_budget: &Budget,
) -> Option<Vec<Arc<Chunk>>> {
    re_tracing::profile_function!();

    let num_rows = chunk.num_rows();
    if num_rows <= 1 {
        return None;
    }

    let cumulative = estimate_cumulative_row_bytes(chunk);
    let base_chunk_bytes = total_chunk_bytes.saturating_sub(cumulative[num_rows]);

    let mut pieces = Vec::new();
    let mut start = 0;
    while start < num_rows {
        // A room piece is admitted on its measured size against an exact room, so the fixed cost
        // is set aside; a full piece is confirmed within the band, whose slack absorbs it.
        let (budget, available_bytes) = match room_budget.as_ref() {
            Some(room) => (room, room.max_row_bytes.saturating_sub(base_chunk_bytes)),
            None => (full_budget, full_budget.max_row_bytes),
        };
        let mut end = rows_that_fit(&cumulative, start, available_bytes, budget.max_rows);
        if end == start {
            if room_budget.is_some() {
                // Not even one row joins the accumulator: the piece seeds the next output instead.
                room_budget = None;
                continue;
            }

            // A row over the target on its own stands alone.
            end = start + 1;
        }

        let mut piece = chunk.row_sliced_deep(Span::from_start_len(start, end - start));
        let mut measured = piece.total_size_bytes();
        if measured > budget.max_measured_bytes {
            let shrunk = rows_that_fit(
                &cumulative,
                start,
                (cumulative[end] - cumulative[start])
                    .saturating_sub(measured - budget.max_measured_bytes),
                budget.max_rows,
            );
            if start < shrunk && shrunk < end {
                end = shrunk;
                piece = chunk.row_sliced_deep(Span::from_start_len(start, end - start));
                measured = piece.total_size_bytes();
            }
            if measured > budget.max_measured_bytes && room_budget.is_some() {
                room_budget = None;
                continue;
            }
        }

        pieces.push(Arc::new(piece));
        start = end;
        room_budget = None;
    }

    if pieces.len() <= 1 {
        return None;
    }
    Some(pieces)
}

/// The largest `end` in `[start, num_rows]` whose rows `[start, end)` fit the budgets by their
/// `cumulative` row bytes (see [`estimate_cumulative_row_bytes`]). `start` itself when not even
/// one row fits.
fn rows_that_fit(cumulative: &[u64], start: usize, bytes_budget: u64, rows_budget: u64) -> usize {
    let num_rows = cumulative.len() - 1;
    let fitting_rows = cumulative[start + 1..=num_rows]
        .partition_point(|&size| size - cumulative[start] <= bytes_budget);
    #[expect(clippy::cast_possible_truncation)] // bounded by the row count
    let by_rows = rows_budget.min(num_rows as u64) as usize;
    start + fitting_rows.min(by_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four rows of 10 bytes each.
    const CUMULATIVE: [u64; 5] = [0, 10, 20, 30, 40];

    #[test]
    fn rows_that_fit_by_bytes_then_rows() {
        assert_eq!(rows_that_fit(&CUMULATIVE, 0, 25, u64::MAX), 2);
        assert_eq!(rows_that_fit(&CUMULATIVE, 0, 25, 1), 1);
        assert_eq!(rows_that_fit(&CUMULATIVE, 2, 25, u64::MAX), 4);
        assert_eq!(rows_that_fit(&CUMULATIVE, 0, 100, u64::MAX), 4);
        assert_eq!(rows_that_fit(&CUMULATIVE, 0, 100, 0), 0, "no rows allowed");
        assert_eq!(
            rows_that_fit(&CUMULATIVE, 1, 5, u64::MAX),
            1,
            "nothing fits: `start` itself"
        );
    }
}
