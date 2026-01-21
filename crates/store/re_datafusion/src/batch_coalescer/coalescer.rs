use arrow::array::RecordBatch;
use arrow::compute::concat_batches;
use arrow::datatypes::SchemaRef;
use re_arrow_util::garbage_collect_string_view_batch;
use re_dataframe::external::re_chunk::external::re_byte_size::SizeBytes as _;
use std::sync::Arc;

/// Indicates the status of the [`SizedBatchCoalescer`] buffer after the
/// [`SizedBatchCoalescer::push_batch()`] operation.
///
/// The caller should take different actions, depending on the variant returned.
pub enum CoalescerStatus {
    /// Neither the limit nor the target batch size is reached.
    ///
    /// Action: continue pushing batches.
    Continue,

    /// The specified minimum number of rows a batch should have or the target
    /// size limit in bytes has been reached.
    ///
    /// Action: call [`SizedBatchCoalescer::finish_batch()`] to get the current
    /// buffered results as a batch and then continue pushing batches.
    BatchFull,

    /// The max_rows limit has been reached.
    ///
    /// Action: call [`SizedBatchCoalescer::finish_batch()`] to get the final
    /// buffered results as a batch and finish the query.
    EndReached,
}

/// Based directly on `BatchCoalescer` this struct adds support
/// for outputting batches either by reaching a number of rows
/// or a batch size in bytes.
#[derive(Debug)]
pub struct SizedBatchCoalescer {
    /// The input schema
    schema: SchemaRef,

    /// Input options
    options: CoalescerOptions,

    /// Current state
    state: CoalescerState,
}

#[derive(Debug, Clone)]
pub struct CoalescerOptions {
    /// Preferred size in bytes for coalesces batches
    pub target_batch_bytes: u64,

    /// Minimum number of rows for coalesces batches
    pub target_batch_rows: usize,

    /// Maximum number of rows to fetch, `None` means fetch all rows
    pub max_rows: Option<usize>,
}

#[derive(Debug, Clone)]
struct CoalescerState {
    /// Total number of rows returned so far
    total_rows: usize,

    /// Buffered batches
    buffer: Vec<RecordBatch>,

    /// Buffered size in bytes
    buffered_bytes: u64,

    /// Buffered row count
    buffered_rows: usize,
}

impl Default for CoalescerState {
    fn default() -> Self {
        Self {
            total_rows: 0,
            buffer: vec![],
            buffered_bytes: 0,
            buffered_rows: 0,
        }
    }
}

impl SizedBatchCoalescer {
    /// Create a new `SizeBasedBatchCoalescer`
    ///
    /// # Arguments
    /// - `schema` - the schema of the output batches
    /// - `options` - configuration for when to output
    pub fn new(schema: SchemaRef, options: CoalescerOptions) -> Self {
        Self {
            schema,
            options,
            state: CoalescerState::default(),
        }
    }

    /// Return the schema of the output batches
    pub fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    /// Push next batch, and returns [`CoalescerStatus`] indicating the current
    /// state of the buffer.
    pub fn push_batch(&mut self, batch: &RecordBatch) -> CoalescerStatus {
        let batch = garbage_collect_string_view_batch(batch);
        if self.max_rows_reached(&batch) {
            CoalescerStatus::EndReached
        } else if self.target_reached(batch) {
            CoalescerStatus::BatchFull
        } else {
            CoalescerStatus::Continue
        }
    }

    /// Return true if the there is no data buffered
    pub fn is_empty(&self) -> bool {
        self.state.buffer.is_empty()
    }

    /// Checks if the buffer will reach the specified limit after getting
    /// `batch`.
    ///
    /// If max_rows would be exceeded, slices the received batch, updates the
    /// buffer with it, and returns `true`.
    ///
    /// Otherwise: does nothing and returns `false`.
    fn max_rows_reached(&mut self, batch: &RecordBatch) -> bool {
        if let Some(max_rows) = self.options.max_rows
            && self.state.total_rows + batch.num_rows() >= max_rows
        {
            // Limit is reached
            let remaining_rows = max_rows - self.state.total_rows;
            debug_assert!(remaining_rows > 0);

            let batch = batch.slice(0, remaining_rows);
            self.state.buffered_rows += batch.num_rows();
            self.state.buffered_bytes += batch.total_size_bytes();
            self.state.total_rows = max_rows;
            self.state.buffer.push(batch);
            true
        } else {
            false
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
            self.state.total_rows += batch.num_rows();
            self.state.buffered_rows += batch.num_rows();
            self.state.buffered_bytes += batch.total_size_bytes();
            self.state.buffer.push(batch);
            self.state.buffered_rows >= self.options.target_batch_rows
                || self.state.buffered_bytes >= self.options.target_batch_bytes
        }
    }

    /// Concatenates and returns all buffered batches, and clears the buffer.
    pub fn finish_batch(&mut self) -> datafusion::error::Result<RecordBatch> {
        let batch = concat_batches(&self.schema, &self.state.buffer)?;
        self.state.buffer.clear();
        self.state.buffered_rows = 0;
        self.state.buffered_bytes = 0;
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ops::Range;

    use super::*;

    use arrow::array::{
        Array, ArrayRef, RecordBatchOptions, StringViewArray, StringViewBuilder, UInt32Array,
        builder::ArrayBuilder as _,
    };
    use arrow::datatypes::{DataType, Field, Schema};

    #[test]
    fn test_coalesce() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // expected output is batches of at least 20 rows (except for the final batch)
            .with_target_batch_rows(21)
            .with_target_batch_bytes(1024 * 1024)
            .with_expected_output_sizes(vec![24, 24, 24, 8])
            .run();
    }

    #[test]
    fn test_coalesce_with_max_rows_larger_than_input_size() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with max_rows limit of 100
            // expected to behave the same as `test_concat_batches`
            .with_target_batch_rows(21)
            .with_max_rows(Some(100))
            .with_expected_output_sizes(vec![24, 24, 24, 8])
            .run();
    }

    #[test]
    fn test_coalesce_with_max_rows_less_than_input_size() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with max_rows limit of 50
            .with_target_batch_rows(21)
            .with_max_rows(Some(50))
            .with_expected_output_sizes(vec![24, 24, 2])
            .run();
    }

    #[test]
    fn test_coalesce_with_max_rows_less_than_target_and_no_remaining_rows() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with max_rows limit of 48
            .with_target_batch_rows(21)
            .with_max_rows(Some(48))
            .with_expected_output_sizes(vec![24, 24])
            .run();
    }

    #[test]
    fn test_coalesce_with_max_rows_less_target_batch_rows() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 10))
            // input is 10 batches x 8 rows (80 rows) with max_rows limit of 10
            .with_target_batch_rows(21)
            .with_max_rows(Some(10))
            .with_expected_output_sizes(vec![10])
            .run();
    }

    #[test]
    fn test_coalesce_single_large_batch_over_max_rows() {
        let large_batch = uint32_batch(0..100);
        Test::new()
            .with_batch(large_batch)
            .with_target_batch_rows(20)
            .with_max_rows(Some(7))
            .with_expected_output_sizes(vec![7])
            .run();
    }

    #[test]
    fn test_coalesce_batch_limited_by_bytes() {
        let batch = uint32_batch(0..8);
        Test::new()
            .with_batches(std::iter::repeat_n(batch, 4))
            .with_target_batch_bytes(20)
            .with_expected_output_sizes(vec![8, 8, 8, 8])
            .run();
    }

    /// Test for [`SizedBatchCoalescer`]
    ///
    /// Pushes the input batches to the coalescer and verifies that the resulting
    /// batches have the expected number of rows and contents.
    #[derive(Debug, Clone)]
    struct Test {
        /// Batches to feed to the coalescer. Tests must have at least one
        /// schema
        input_batches: Vec<RecordBatch>,

        /// Expected output sizes of the resulting batches
        expected_output_sizes: Vec<usize>,

        /// Input options
        options: CoalescerOptions,
    }

    impl Default for Test {
        fn default() -> Self {
            Self {
                // Initialize with large values in case these are not set explicitly
                input_batches: vec![],
                options: CoalescerOptions {
                    target_batch_rows: 1000,
                    target_batch_bytes: 1024 * 1024,
                    max_rows: None,
                },
                expected_output_sizes: vec![],
            }
        }
    }

    impl Test {
        fn new() -> Self {
            Self::default()
        }

        /// Set the target batch size
        fn with_target_batch_rows(mut self, target_batch_rows: usize) -> Self {
            self.options.target_batch_rows = target_batch_rows;
            self
        }

        fn with_target_batch_bytes(mut self, target_batch_bytes: u64) -> Self {
            self.options.target_batch_bytes = target_batch_bytes;
            self
        }

        /// Set the max_rows (limit)
        fn with_max_rows(mut self, max_rows: Option<usize>) -> Self {
            self.options.max_rows = max_rows;
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
                options,
                expected_output_sizes,
            } = self;

            let schema = input_batches[0].schema();

            // create a single large input batch for output comparison
            let single_input_batch = concat_batches(&schema, &input_batches).unwrap();

            let mut coalescer = SizedBatchCoalescer::new(Arc::clone(&schema), options);

            let mut output_batches = vec![];
            for batch in input_batches {
                match coalescer.push_batch(&batch) {
                    CoalescerStatus::Continue => {}
                    CoalescerStatus::BatchFull => {
                        output_batches.push(coalescer.finish_batch().unwrap());
                    }
                    CoalescerStatus::EndReached => {
                        output_batches.push(coalescer.finish_batch().unwrap());
                        break;
                    }
                }
            }
            if coalescer.state.buffered_rows != 0 {
                output_batches.push(coalescer.finish_batch().unwrap());
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

    /// Return a batch of `UInt32` with the specified range
    fn uint32_batch(range: Range<u32>) -> RecordBatch {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("c0", DataType::UInt32, false)],
            HashMap::default(),
        ));

        let mut options = RecordBatchOptions::new();
        options = options.with_row_count(Some(range.len()));
        RecordBatch::try_new_with_options(
            Arc::clone(&schema),
            vec![Arc::new(UInt32Array::from_iter_values(range))],
            &options,
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
        let output_batch = garbage_collect_string_view_batch(&batch);
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
        let gc_batch = garbage_collect_string_view_batch(&batch);
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
                for &v in &self.strings {
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
