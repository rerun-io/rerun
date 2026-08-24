//! Bounded parquet reading: decode only the row groups intersecting a row range.

use std::path::Path;

use anyhow::anyhow;
use arrow::array::RecordBatch;
use parquet::arrow::arrow_reader::{
    ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder, RowSelection, RowSelector,
};
use parquet::file::reader::ChunkReader;
use re_span::Span;

use crate::LeRobotError;

/// Open a record-batch reader over `rows`; `None` reads the whole file.
///
/// A bounded span decodes only the row groups that intersect it, with a [`RowSelection`]
/// trimming the partial groups at the edges. A span that reaches outside the file, or an
/// empty one, is an error.
///
/// There is no separate schema accessor by design: the schema travels with the batches
/// (`RecordBatch::schema` on any yielded batch).
pub fn read_row_range(
    path: &Path,
    rows: Option<Span<u64>>,
) -> Result<impl Iterator<Item = Result<RecordBatch, arrow::error::ArrowError>> + use<>, LeRobotError>
{
    re_tracing::profile_function!();
    let file = std::fs::File::open(path).map_err(|err| LeRobotError::io(err, path))?;
    open_row_range(file, rows, path)
}

/// `path` is error context only; the bytes come from `input`.
fn open_row_range<R: ChunkReader + 'static>(
    input: R,
    rows: Option<Span<u64>>,
    path: &Path,
) -> Result<ParquetRecordBatchReader, LeRobotError> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(input)?;

    let Some(rows) = rows else {
        return Ok(builder.build()?);
    };

    let num_rows = row_count(builder.metadata().file_metadata().num_rows(), path)?;
    if rows.is_empty() || num_rows < rows.end() {
        return Err(LeRobotError::InvalidRowRange {
            rows,
            num_rows,
            path: path.to_path_buf(),
        });
    }

    let mut row_groups = Vec::new();
    let mut selected_start = None;
    let mut selected_len: u64 = 0;
    let mut group_start: u64 = 0;
    for (index, row_group) in builder.metadata().row_groups().iter().enumerate() {
        let group_end = group_start + row_count(row_group.num_rows(), path)?;
        if group_start < rows.end() && rows.start < group_end {
            row_groups.push(index);
            selected_start.get_or_insert(group_start);
            selected_len += group_end - group_start;
        }
        group_start = group_end;
    }

    // The selection is relative to the concatenation of the selected groups; `rows`
    // always starts inside the first of them (groups are contiguous from row 0 and the
    // range is in bounds), so one skip/select/skip triple covers it.
    let selected_start = selected_start.unwrap_or(rows.start);
    let leading = rows.start - selected_start;
    let taken = rows.len;
    re_log::debug_assert!(
        leading + taken <= selected_len,
        "the selected groups must cover the in-bounds range"
    );
    let selectors = [
        RowSelector::skip(as_row_count(leading, path)?),
        RowSelector::select(as_row_count(taken, path)?),
        RowSelector::skip(as_row_count(selected_len - leading - taken, path)?),
    ]
    .into_iter()
    .filter(|selector| selector.row_count > 0)
    .collect::<Vec<_>>();

    Ok(builder
        .with_row_groups(row_groups)
        .with_row_selection(RowSelection::from(selectors))
        .build()?)
}

/// Parquet types row counts as `i64`; a negative count is corrupt metadata.
fn row_count(count: i64, path: &Path) -> Result<u64, LeRobotError> {
    u64::try_from(count).map_err(|_err| {
        anyhow!(
            "Parquet metadata reports a negative row count ({count})\nFile path: {}",
            path.display()
        )
        .into()
    })
}

/// `usize` is 32-bit on wasm; a row count that cannot index there is an error.
fn as_row_count(count: u64, path: &Path) -> Result<usize, LeRobotError> {
    usize::try_from(count).map_err(|_err| {
        anyhow!(
            "Row count {count} does not fit this platform's pointer size\nFile path: {}",
            path.display()
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::sync::Arc;

    use arrow::array::{Int64Array, RecordBatch, RecordBatchOptions};
    use arrow::compute::concat_batches;
    use arrow::datatypes::{DataType, Field, Schema};
    use parking_lot::Mutex;
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;
    use parquet::file::reader::Length;
    use re_arrow_util::ArrowArrayDowncastRef as _;

    use super::*;

    /// Write `num_rows` rows of one `i64` column (`value[i] = i`), `rows_per_group` per row group.
    fn write_parquet(path: &Path, num_rows: usize, rows_per_group: usize) {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("value", DataType::Int64, false)],
            Default::default(),
        ));
        let values: Vec<i64> = (0..i64::try_from(num_rows).unwrap()).collect();
        let batch = RecordBatch::try_new_with_options(
            schema.clone(),
            vec![Arc::new(Int64Array::from(values))],
            &RecordBatchOptions::default(),
        )
        .unwrap();

        let properties = WriterProperties::builder()
            .set_max_row_group_row_count(Some(rows_per_group))
            .build();
        let mut writer =
            ArrowWriter::try_new(File::create(path).unwrap(), schema, Some(properties)).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }

    fn read_values(
        reader: impl Iterator<Item = Result<RecordBatch, arrow::error::ArrowError>>,
    ) -> Vec<i64> {
        reader
            .map(Result::unwrap)
            .flat_map(|batch| {
                batch
                    .column(0)
                    .downcast_array_ref::<Int64Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect()
    }

    /// A bounded read yields exactly the requested rows, including ranges that straddle
    /// row-group boundaries.
    #[test]
    fn row_ranges_yield_exactly_the_requested_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.parquet");
        write_parquet(&path, 10, 4); // row groups: [0,4), [4,8), [8,10)

        for (range, expected) in [
            (Span::from_start_end(0, 10), (0..10).collect::<Vec<i64>>()),
            (Span::from_start_end(3, 7), vec![3, 4, 5, 6]),
            (Span::from_start_end(4, 8), vec![4, 5, 6, 7]),
            (Span::from_start_end(9, 10), vec![9]),
        ] {
            let reader = read_row_range(&path, Some(range)).unwrap();
            assert_eq!(read_values(reader), expected, "range {range:?}");
        }
    }

    /// An empty or out-of-bounds span is a loud error, not a silently clamped read.
    #[test]
    fn empty_and_out_of_bounds_ranges_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.parquet");
        write_parquet(&path, 10, 4);

        // An inverted range like `7..3` is unrepresentable as a `Span`:
        // it must be rejected at construction.
        assert_eq!(Span::try_from_start_end(7_u64, 3), None);

        for range in [
            Span::from_start_end(5, 5),
            Span::from_start_end(8, 11),
            Span::from_start_end(10, 12),
        ] {
            let Err(err) = read_row_range(&path, Some(range)) else {
                panic!("range {range:?} must be rejected")
            };
            assert!(
                matches!(err, LeRobotError::InvalidRowRange { .. }),
                "range {range:?} gave: {err}"
            );
        }
    }

    /// Reading the full range produces the same rows as an unbounded whole-file read.
    #[test]
    fn full_range_equals_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.parquet");
        write_parquet(&path, 10, 4);

        let whole: Vec<RecordBatch> = read_row_range(&path, None)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let ranged: Vec<RecordBatch> = read_row_range(&path, Some(Span::from_start_end(0, 10)))
            .unwrap()
            .map(Result::unwrap)
            .collect();

        let schema = whole[0].schema();
        assert_eq!(
            concat_batches(&schema, &whole).unwrap(),
            concat_batches(&schema, &ranged).unwrap()
        );
    }

    /// Delegates to a [`File`] while recording the byte offset of every read.
    struct ReadLogger {
        file: File,
        offsets: Arc<Mutex<Vec<u64>>>,
    }

    struct LoggedRead<R> {
        inner: R,
    }

    impl<R: Read> Read for LoggedRead<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Length for ReadLogger {
        fn len(&self) -> u64 {
            self.file.len()
        }
    }

    impl ChunkReader for ReadLogger {
        type T = LoggedRead<<File as ChunkReader>::T>;

        fn get_read(&self, start: u64) -> parquet::errors::Result<Self::T> {
            self.offsets.lock().push(start);
            Ok(LoggedRead {
                inner: self.file.get_read(start)?,
            })
        }

        fn get_bytes(&self, start: u64, length: usize) -> parquet::errors::Result<bytes::Bytes> {
            self.offsets.lock().push(start);
            self.file.get_bytes(start, length)
        }
    }

    /// The pruning law: a bounded read of the last row group never touches the bytes of
    /// the first one — only intersecting row groups (plus the footer) are read.
    #[test]
    fn bounded_reads_prune_non_intersecting_row_groups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.parquet");
        write_parquet(&path, 10, 4); // row groups: [0,4), [4,8), [8,10)

        let offsets = Arc::new(Mutex::new(Vec::new()));
        let logger = ReadLogger {
            file: File::open(&path).unwrap(),
            offsets: Arc::clone(&offsets),
        };

        let reader = open_row_range(
            logger,
            Some(Span::from_start_end(8, 10)),
            Path::new("logged.parquet"),
        )
        .unwrap();
        let metadata = {
            let file = File::open(&path).unwrap();
            ParquetRecordBatchReaderBuilder::try_new(file)
                .unwrap()
                .metadata()
                .clone()
        };
        assert_eq!(read_values(reader), vec![8, 9]);

        // Attribute each read to the byte range its start offset falls in: footer and
        // metadata reads start past the data, so the law is that no read starts inside
        // row group 0's byte span.
        let first_group_range = metadata.row_groups()[0].column(0).byte_range();
        let first_group_range = first_group_range.0..first_group_range.0 + first_group_range.1;
        let offsets = offsets.lock();
        assert!(
            offsets.iter().all(|o| !first_group_range.contains(o)),
            "no read may start inside row group 0 ({first_group_range:?}), got {offsets:?}"
        );
    }
}
