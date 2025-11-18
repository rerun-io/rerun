use arrow::array::{
    Array as _, ArrayRef, RecordBatch, RecordBatchOptions, builder::StringViewBuilder,
    cast::AsArray as _,
};
use arrow::compute::concat_batches;
use arrow::datatypes::SchemaRef;
use re_dataframe::external::re_chunk::external::re_byte_size::SizeBytes as _;
use std::sync::Arc;

/// Based directly on `BatchCoalescer`
#[derive(Debug)]
pub struct SizedBatchCoalescer {
    /// The input schema
    schema: SchemaRef,

    /// Maximum preferred size in bytes for coalesces batches
    target_batch_bytes: usize,

    /// Minimum number of rows for coalesces batches
    target_batch_rows: usize,

    /// Total number of rows returned so far
    total_rows: usize,

    /// Buffered batches
    buffer: Vec<RecordBatch>,

    /// Buffered size in bytes
    buffered_bytes: usize,

    /// Buffered row count
    buffered_rows: usize,

    /// Limit: maximum number of rows to fetch, `None` means fetch all rows
    fetch: Option<usize>,
}

impl SizedBatchCoalescer {
    /// Create a new `BatchCoalescer`
    ///
    /// # Arguments
    /// - `schema` - the schema of the output batches
    /// - `target_batch_bytes` - the ideal maximum size in total bytes
    /// - `target_batch_rows` - the minimum number of rows for each
    ///   output batch (until limit reached)
    /// - `fetch` - the maximum number of rows to fetch, `None` means fetch all rows
    pub fn new(
        schema: SchemaRef,
        target_batch_bytes: usize,
        target_batch_rows: usize,
        fetch: Option<usize>,
    ) -> Self {
        Self {
            schema,
            target_batch_bytes,
            target_batch_rows,
            total_rows: 0,
            buffer: vec![],
            buffered_bytes: 0,
            buffered_rows: 0,
            fetch,
        }
    }

    /// Return the schema of the output batches
    pub fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    /// Push next batch, and returns [`CoalescerState`] indicating the current
    /// state of the buffer.
    pub fn push_batch(&mut self, batch: &RecordBatch) -> CoalescerState {
        let batch = gc_string_view_batch(batch);
        if self.limit_reached(&batch) {
            CoalescerState::LimitReached
        } else if self.target_reached(batch) {
            CoalescerState::TargetReached
        } else {
            CoalescerState::Continue
        }
    }

    /// Return true if the there is no data buffered
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Checks if the buffer will reach the specified limit after getting
    /// `batch`.
    ///
    /// If fetch would be exceeded, slices the received batch, updates the
    /// buffer with it, and returns `true`.
    ///
    /// Otherwise: does nothing and returns `false`.
    fn limit_reached(&mut self, batch: &RecordBatch) -> bool {
        match self.fetch {
            Some(fetch) if self.total_rows + batch.num_rows() >= fetch => {
                // Limit is reached
                let remaining_rows = fetch - self.total_rows;
                debug_assert!(remaining_rows > 0);

                let batch = batch.slice(0, remaining_rows);
                self.buffered_rows += batch.num_rows();
                self.buffered_bytes += batch.total_size_bytes() as usize;
                self.total_rows = fetch;
                self.buffer.push(batch);
                true
            }
            _ => false,
        }
    }

    /// Updates the buffer with the given batch.
    ///
    /// If the target batch size is reached, returns `true`. Otherwise, returns
    /// `false`.
    fn target_reached(&mut self, batch: RecordBatch) -> bool {
        if batch.num_rows() == 0 {
            false
        } else {
            self.total_rows += batch.num_rows();
            self.buffered_rows += batch.num_rows();
            self.buffered_bytes += batch.total_size_bytes() as usize;
            self.buffer.push(batch);
            self.buffered_rows >= self.target_batch_rows
                || self.buffered_bytes >= self.target_batch_bytes
        }
    }

    /// Concatenates and returns all buffered batches, and clears the buffer.
    pub fn finish_batch(&mut self) -> datafusion::error::Result<RecordBatch> {
        let batch = concat_batches(&self.schema, &self.buffer)?;
        self.buffer.clear();
        self.buffered_rows = 0;
        self.buffered_bytes = 0;
        Ok(batch)
    }
}

/// Indicates the state of the [`SizedBatchCoalescer`] buffer after the
/// [`SizedBatchCoalescer::push_batch()`] operation.
///
/// The caller should take different actions, depending on the variant returned.
pub enum CoalescerState {
    /// Neither the limit nor the target batch size is reached.
    ///
    /// Action: continue pushing batches.
    Continue,

    /// The limit has been reached.
    ///
    /// Action: call [`SizedBatchCoalescer::finish_batch()`] to get the final
    /// buffered results as a batch and finish the query.
    LimitReached,

    /// The specified minimum number of rows a batch should have is reached.
    ///
    /// Action: call [`SizedBatchCoalescer::finish_batch()`] to get the current
    /// buffered results as a batch and then continue pushing batches.
    TargetReached,
}

/// Heuristically compact `StringViewArray`s to reduce memory usage, if needed
///
/// Decides when to consolidate the `StringView` into a new buffer to reduce
/// memory usage and improve string locality for better performance.
///
/// This differs from `StringViewArray::gc` because:
/// 1. It may not compact the array depending on a heuristic.
/// 2. It uses a precise block size to reduce the number of buffers to track.
///
/// # Heuristic
///
/// If the average size of each view is larger than 32 bytes, we compact the array.
///
/// `StringViewArray` include pointers to buffer that hold the underlying data.
/// One of the great benefits of `StringViewArray` is that many operations
/// (e.g., `filter`) can be done without copying the underlying data.
///
/// However, after a while (e.g., after `FilterExec` or `HashJoinExec`) the
/// `StringViewArray` may only refer to a small portion of the buffer,
/// significantly increasing memory usage.
fn gc_string_view_batch(batch: &RecordBatch) -> RecordBatch {
    let new_columns: Vec<ArrayRef> = batch
        .columns()
        .iter()
        .map(|c| {
            // Try to re-create the `StringViewArray` to prevent holding the underlying buffer too long.
            let Some(s) = c.as_string_view_opt() else {
                return Arc::clone(c);
            };

            // Fast path: if the data buffers are empty, we can return the original array
            if s.data_buffers().is_empty() {
                return Arc::clone(c);
            }

            let ideal_buffer_size: usize = s
                .views()
                .iter()
                .map(|v| {
                    let len = (*v as u32) as usize;
                    if len > 12 { len } else { 0 }
                })
                .sum();

            // We don't use get_buffer_memory_size here, because gc is for the contents of the
            // data buffers, not views and nulls.
            let actual_buffer_size = s.data_buffers().iter().map(|b| b.capacity()).sum::<usize>();

            // Re-creating the array copies data and can be time consuming.
            // We only do it if the array is sparse
            if actual_buffer_size > (ideal_buffer_size * 2) {
                // We set the block size to `ideal_buffer_size` so that the new StringViewArray only has one buffer, which accelerate later concat_batches.
                // See https://github.com/apache/arrow-rs/issues/6094 for more details.
                let mut builder = StringViewBuilder::with_capacity(s.len());
                if ideal_buffer_size > 0 {
                    builder = builder.with_fixed_block_size(ideal_buffer_size as u32);
                }

                for v in s {
                    builder.append_option(v);
                }

                let gc_string = builder.finish();

                debug_assert!(gc_string.data_buffers().len() <= 1); // buffer count can be 0 if the `ideal_buffer_size` is 0

                Arc::new(gc_string)
            } else {
                Arc::clone(c)
            }
        })
        .collect();
    let mut options = RecordBatchOptions::new();
    options = options.with_row_count(Some(batch.num_rows()));
    RecordBatch::try_new_with_options(batch.schema(), new_columns, &options)
        .expect("Failed to re-create the gc'ed record batch")
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::*;

    use arrow::array::{StringViewArray, UInt32Array, builder::ArrayBuilder};
    use arrow::datatypes::{DataType, Field, Schema};

    #[test]
    fn test_coalesce() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // expected output is batches of at least 20 rows (except for the final batch)
            .with_target_batch_rows(21)
            .with_expected_output_sizes(vec![24, 24, 24, 8])
            .run()
    }

    #[test]
    fn test_coalesce_with_fetch_larger_than_input_size() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with fetch limit of 100
            // expected to behave the same as `test_concat_batches`
            .with_target_batch_rows(21)
            .with_fetch(Some(100))
            .with_expected_output_sizes(vec![24, 24, 24, 8])
            .run();
    }

    #[test]
    fn test_coalesce_with_fetch_less_than_input_size() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with fetch limit of 50
            .with_target_batch_rows(21)
            .with_fetch(Some(50))
            .with_expected_output_sizes(vec![24, 24, 2])
            .run();
    }

    #[test]
    fn test_coalesce_with_fetch_less_than_target_and_no_remaining_rows() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with fetch limit of 48
            .with_target_batch_rows(21)
            .with_fetch(Some(48))
            .with_expected_output_sizes(vec![24, 24])
            .run();
    }

    #[test]
    fn test_coalesce_with_fetch_less_target_batch_rows() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with fetch limit of 10
            .with_target_batch_rows(21)
            .with_fetch(Some(10))
            .with_expected_output_sizes(vec![10])
            .run();
    }

    #[test]
    fn test_coalesce_single_large_batch_over_fetch() {
        let large_batch = uint32_batch(0..100);
        Test::new()
            .with_batch(large_batch)
            .with_target_batch_rows(20)
            .with_fetch(Some(7))
            .with_expected_output_sizes(vec![7])
            .run()
    }

    /// Test for [`SizedBatchCoalescer`]
    ///
    /// Pushes the input batches to the coalescer and verifies that the resulting
    /// batches have the expected number of rows and contents.
    #[derive(Debug, Clone, Default)]
    struct Test {
        /// Batches to feed to the coalescer. Tests must have at least one
        /// schema
        input_batches: Vec<RecordBatch>,

        /// Expected output sizes of the resulting batches
        expected_output_sizes: Vec<usize>,

        /// target batch by bytes
        target_batch_bytes: usize,

        /// target batch by rows
        target_batch_rows: usize,

        /// Fetch (limit)
        fetch: Option<usize>,
    }

    impl Test {
        fn new() -> Self {
            Self::default()
        }

        /// Set the target batch size
        fn with_target_batch_rows(mut self, target_batch_rows: usize) -> Self {
            self.target_batch_rows = target_batch_rows;
            self
        }

        fn with_target_batch_bytes(mut self, target_batch_bytes: usize) -> Self {
            self.target_batch_bytes = target_batch_bytes;
            self
        }

        /// Set the fetch (limit)
        fn with_fetch(mut self, fetch: Option<usize>) -> Self {
            self.fetch = fetch;
            self
        }

        /// Extend the input batches with `batch`
        fn with_batch(mut self, batch: RecordBatch) -> Self {
            self.input_batches.push(batch);
            self
        }

        /// Extends the input batches with `batches`
        fn with_batches(mut self, batches: impl IntoIterator<Item = RecordBatch>) -> Self {
            self.input_batches.extend(batches);
            self
        }

        /// Extends `sizes` to expected output sizes
        fn with_expected_output_sizes(mut self, sizes: impl IntoIterator<Item = usize>) -> Self {
            self.expected_output_sizes.extend(sizes);
            self
        }

        /// Runs the test -- see documentation on [`Test`] for details
        fn run(self) {
            let Self {
                input_batches,
                target_batch_bytes,
                target_batch_rows,
                fetch,
                expected_output_sizes,
            } = self;

            let schema = input_batches[0].schema();

            // create a single large input batch for output comparison
            let single_input_batch = concat_batches(&schema, &input_batches).unwrap();

            let mut coalescer = SizedBatchCoalescer::new(
                Arc::clone(&schema),
                target_batch_bytes,
                target_batch_rows,
                fetch,
            );

            let mut output_batches = vec![];
            for batch in input_batches {
                match coalescer.push_batch(&batch) {
                    CoalescerState::Continue => {}
                    CoalescerState::LimitReached => {
                        output_batches.push(coalescer.finish_batch().unwrap());
                        break;
                    }
                    CoalescerState::TargetReached => {
                        coalescer.buffered_rows = 0;
                        coalescer.buffered_bytes = 0;
                        output_batches.push(coalescer.finish_batch().unwrap());
                    }
                }
            }
            if coalescer.buffered_rows != 0 {
                output_batches.extend(coalescer.buffer);
            }

            // make sure we got the expected number of output batches and content
            let mut starting_idx = 0;
            assert_eq!(expected_output_sizes.len(), output_batches.len());
            for (i, (expected_size, batch)) in
                expected_output_sizes.iter().zip(output_batches).enumerate()
            {
                assert_eq!(
                    *expected_size,
                    batch.num_rows(),
                    "Unexpected number of rows in Batch {i}"
                );

                // compare the contents of the batch (using `==` compares the
                // underlying memory layout too)
                let expected_batch = single_input_batch.slice(starting_idx, *expected_size);
                let batch_strings = batch_to_pretty_strings(&batch);
                let expected_batch_strings = batch_to_pretty_strings(&expected_batch);
                let batch_strings = batch_strings.lines().collect::<Vec<_>>();
                let expected_batch_strings = expected_batch_strings.lines().collect::<Vec<_>>();
                assert_eq!(
                    expected_batch_strings, batch_strings,
                    "Unexpected content in Batch {i}:\
                    \n\nExpected:\n{expected_batch_strings:#?}\n\nActual:\n{batch_strings:#?}"
                );
                starting_idx += *expected_size;
            }
        }
    }

    /// Return a batch of UInt32 with the specified range
    fn uint32_batch(range: Range<u32>) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![Field::new("c0", DataType::UInt32, false)]));

        RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(UInt32Array::from_iter_values(range))],
        )
        .unwrap()
    }

    #[test]
    fn test_gc_string_view_batch_small_no_compact() {
        // view with only short strings (no buffers) --> no need to compact
        let array = StringViewTest {
            rows: 1000,
            strings: vec![Some("a"), Some("b"), Some("c")],
        }
        .build();

        let gc_array = do_gc(array.clone());
        compare_string_array_values(&array, &gc_array);
        assert_eq!(array.data_buffers().len(), 0);
        assert_eq!(array.data_buffers().len(), gc_array.data_buffers().len()); // no compaction
    }

    #[test]
    fn test_gc_string_view_test_batch_empty() {
        let schema = Schema::empty();
        let batch = RecordBatch::new_empty(schema.into());
        let output_batch = gc_string_view_batch(&batch);
        assert_eq!(batch.num_columns(), output_batch.num_columns());
        assert_eq!(batch.num_rows(), output_batch.num_rows());
    }

    #[test]
    fn test_gc_string_view_batch_large_no_compact() {
        // view with large strings (has buffers) but full --> no need to compact
        let array = StringViewTest {
            rows: 1000,
            strings: vec![Some("This string is longer than 12 bytes")],
        }
        .build();

        let gc_array = do_gc(array.clone());
        compare_string_array_values(&array, &gc_array);
        assert_eq!(array.data_buffers().len(), 5);
        assert_eq!(array.data_buffers().len(), gc_array.data_buffers().len()); // no compaction
    }

    #[test]
    fn test_gc_string_view_batch_large_slice_compact() {
        // view with large strings (has buffers) and only partially used  --> no need to compact
        let array = StringViewTest {
            rows: 1000,
            strings: vec![Some("this string is longer than 12 bytes")],
        }
        .build();

        // slice only 11 rows, so most of the buffer is not used
        let array = array.slice(11, 22);

        let gc_array = do_gc(array.clone());
        compare_string_array_values(&array, &gc_array);
        assert_eq!(array.data_buffers().len(), 5);
        assert_eq!(gc_array.data_buffers().len(), 1); // compacted into a single buffer
    }

    /// Compares the values of two string view arrays
    fn compare_string_array_values(arr1: &StringViewArray, arr2: &StringViewArray) {
        assert_eq!(arr1.len(), arr2.len());
        for (s1, s2) in arr1.iter().zip(arr2.iter()) {
            assert_eq!(s1, s2);
        }
    }

    /// runs garbage collection on string view array
    /// and ensures the number of rows are the same
    fn do_gc(array: StringViewArray) -> StringViewArray {
        let batch = RecordBatch::try_from_iter(vec![("a", Arc::new(array) as ArrayRef)]).unwrap();
        let gc_batch = gc_string_view_batch(&batch);
        assert_eq!(batch.num_rows(), gc_batch.num_rows());
        assert_eq!(batch.schema(), gc_batch.schema());
        gc_batch
            .column(0)
            .as_any()
            .downcast_ref::<StringViewArray>()
            .unwrap()
            .clone()
    }

    /// Describes parameters for creating a `StringViewArray`
    struct StringViewTest {
        /// The number of rows in the array
        rows: usize,

        /// The strings to use in the array (repeated over and over
        strings: Vec<Option<&'static str>>,
    }

    impl StringViewTest {
        /// Create a `StringViewArray` with the parameters specified in this struct
        fn build(self) -> StringViewArray {
            let mut builder = StringViewBuilder::with_capacity(100).with_fixed_block_size(8192);
            loop {
                for &v in self.strings.iter() {
                    builder.append_option(v);
                    if builder.len() >= self.rows {
                        return builder.finish();
                    }
                }
            }
        }
    }

    fn batch_to_pretty_strings(batch: &RecordBatch) -> String {
        arrow::util::pretty::pretty_format_batches(std::slice::from_ref(batch))
            .unwrap()
            .to_string()
    }
}
