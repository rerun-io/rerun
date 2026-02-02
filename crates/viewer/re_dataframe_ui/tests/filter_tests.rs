use std::str::FromStr as _;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, ListArray, ListBuilder, PrimitiveArray, PrimitiveBuilder,
    StringArray,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::compute::cast;
use arrow::datatypes::{
    ArrowPrimitiveType, DataType, Field, FieldRef, Float32Type, Float64Type, Int8Type, Int16Type,
    Int32Type, Int64Type, Schema, TimeUnit, TimestampNanosecondType, UInt8Type, UInt16Type,
    UInt32Type, UInt64Type,
};
use arrow::record_batch::RecordBatch;
use datafusion::catalog::MemTable;
use datafusion::prelude::{DataFrame, SessionContext};
use jiff::ToSpan as _;
use re_dataframe_ui::{
    ColumnFilter, ComparisonOperator, FloatFilter, IntFilter, NonNullableBooleanFilter,
    Nullability, NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
};
use re_viewer_context::external::tokio;
use strum::VariantArray as _;

const COLUMN_NAME: &str = "column";
const SOME_TIMESTAMP: &str = "2025-09-23T11:47Z";

/// A single column of test data, with convenient constructors.
#[derive(Debug, Clone)]
struct TestColumn {
    field: FieldRef,
    array: ArrayRef,
}

impl TestColumn {
    fn new(array: impl Array + 'static, nullable: bool) -> Self {
        let array = Arc::new(array) as ArrayRef;
        let field = Arc::new(Field::new(COLUMN_NAME, array.data_type().clone(), nullable));

        Self { field, array }
    }

    /// Create a primitive array with the provided data.
    fn primitive<T>(data: Vec<T::Native>) -> Self
    where
        T: ArrowPrimitiveType,
        PrimitiveArray<T>: From<Vec<T::Native>>,
    {
        Self::new(PrimitiveArray::<T>::from(data), false)
    }

    /// Create a nullable primitive array with the provided data.
    fn primitive_nulls<T>(data: Vec<Option<T::Native>>) -> Self
    where
        T: ArrowPrimitiveType,
        PrimitiveArray<T>: From<Vec<Option<T::Native>>>,
    {
        Self::new(PrimitiveArray::<T>::from(data), true)
    }

    /// Create a list array with the provided data and automatically injecting nulls as required.
    fn primitive_lists<T>(data: &[Vec<T::Native>], nullability: Nullability) -> Self
    where
        T: ArrowPrimitiveType,
        T::Native: Copy,
    {
        assert!(!data.is_empty());

        let value_builder = PrimitiveBuilder::<T>::new();
        let mut builder = ListBuilder::new(value_builder);

        for item in data {
            for inner_item in item {
                builder.values().append_value(*inner_item);
            }
            builder.append(true);
        }

        // inject outer null
        if nullability.outer {
            builder.append(false);
        }

        // inject inner nulls
        if nullability.inner {
            // lone null
            builder.values().append_null();
            builder.append(true);

            // find a non-empty inner item
            let non_empty_item = data
                .iter()
                .find(|item| !item.is_empty())
                .expect("there should be at least some non-empty data");

            // first null
            builder.values().append_null();
            for inner_item in non_empty_item {
                builder.values().append_value(*inner_item);
            }
            builder.append(true);

            // last null
            for inner_item in non_empty_item {
                builder.values().append_value(*inner_item);
            }
            builder.values().append_null();
            builder.append(true);
        }

        let array = builder.finish();

        Self::new(array, nullability.outer)
    }

    fn strings() -> Self {
        Self::new(
            StringArray::from(vec!["a", "b", "c", "ab", "A B", "aBc"]),
            false,
        )
    }

    fn strings_nulls() -> Self {
        Self::new(
            StringArray::from(vec![
                Some("a"),
                Some("b"),
                None,
                Some("ab"),
                Some("A B"),
                Some("aBc"),
            ]),
            true,
        )
    }

    fn strings_lists(nullability: Nullability) -> Self {
        // the primitive array stuff doesn't work for strings, so we go the manual way.
        let values = if nullability.inner {
            StringArray::from(vec![
                Some("hello_ab"),
                Some("a"),
                Some("b"),
                None,
                Some("ab"),
                None,
                Some("aBc"),
                Some("bla_AB"),
            ])
        } else {
            StringArray::from(vec![
                "hello_ab", "a", "b", "c", "ab", "A B", "aBc", "bla_AB",
            ])
        };
        let offsets = OffsetBuffer::new(vec![0i32, 2, 4, 6].into());
        let strings_lists = ListArray::try_new(
            Arc::new(Field::new(COLUMN_NAME, DataType::Utf8, nullability.inner)),
            offsets,
            Arc::new(values),
            nullability
                .outer
                .then(|| NullBuffer::from(vec![true, false, true])),
        )
        .expect("failed to create a string list array");

        Self::new(strings_lists, nullability.outer)
    }

    fn bools() -> Self {
        Self::new(BooleanArray::from(vec![true, true, false]), false)
    }

    fn bools_nulls() -> Self {
        Self::new(
            BooleanArray::from(vec![Some(true), Some(true), None, Some(false)]),
            true,
        )
    }

    fn bool_lists(nullability: Nullability) -> Self {
        // the primitive array stuff doesn't work for bools, so we go the manual way.
        let values = if nullability.inner {
            BooleanArray::from(vec![
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(false),
            ])
        } else {
            BooleanArray::from(vec![true, false, true, true, false, false, true, false])
        };

        let offsets = OffsetBuffer::new(vec![0i32, 1, 2, 4, 6, 8].into());
        let bool_lists = ListArray::try_new(
            Arc::new(Field::new(
                COLUMN_NAME,
                DataType::Boolean,
                nullability.inner,
            )),
            offsets,
            Arc::new(values),
            nullability
                .outer
                .then(|| NullBuffer::from(vec![true, false, true, true, true])),
        )
        .expect("failed to create a bool list array");

        Self::new(bool_lists, nullability.outer)
    }

    fn timestamps(unit: TimeUnit) -> Self {
        let some_date = jiff::Timestamp::from_str(SOME_TIMESTAMP).expect("valid");

        let nano_column = Self::primitive::<TimestampNanosecondType>(vec![
            timestamp_to_nanos(some_date),
            timestamp_to_nanos(some_date - 1.hours()),
            timestamp_to_nanos(some_date - 24.hours()),
            timestamp_to_nanos(some_date - 8760.hours()),
            timestamp_to_nanos(some_date + 1.hours()),
            timestamp_to_nanos(some_date + 24.hours()),
            timestamp_to_nanos(some_date + 8760.hours()),
        ]);

        convert_timestamp_column(nano_column, unit)
    }

    fn timestamps_nulls(unit: TimeUnit) -> Self {
        let some_date = jiff::Timestamp::from_str(SOME_TIMESTAMP).expect("valid");

        let nano_column = Self::primitive_nulls::<TimestampNanosecondType>(vec![
            Some(timestamp_to_nanos(some_date)),
            Some(timestamp_to_nanos(some_date - 1.hours())),
            Some(timestamp_to_nanos(some_date - 24.hours())),
            Some(timestamp_to_nanos(some_date - 8760.hours())),
            None,
            Some(timestamp_to_nanos(some_date + 1.hours())),
            Some(timestamp_to_nanos(some_date + 24.hours())),
            None,
            Some(timestamp_to_nanos(some_date + 8760.hours())),
        ]);

        convert_timestamp_column(nano_column, unit)
    }

    fn timestamps_lists(unit: TimeUnit, nullability: Nullability) -> Self {
        let some_date = jiff::Timestamp::from_str(SOME_TIMESTAMP).expect("valid");

        let nano_column = Self::primitive_lists::<TimestampNanosecondType>(
            &[
                vec![],
                vec![timestamp_to_nanos(some_date)],
                vec![
                    timestamp_to_nanos(some_date - 1.hours()),
                    timestamp_to_nanos(some_date - 24.hours()),
                ],
                vec![
                    timestamp_to_nanos(some_date),
                    timestamp_to_nanos(some_date - 168.hours()),
                    timestamp_to_nanos(some_date - 8760.hours()),
                ],
                vec![
                    timestamp_to_nanos(some_date + 1.hours()),
                    timestamp_to_nanos(some_date + 24.hours()),
                ],
                vec![
                    timestamp_to_nanos(some_date),
                    timestamp_to_nanos(some_date + 168.hours()),
                    timestamp_to_nanos(some_date + 8760.hours()),
                ],
                vec![
                    timestamp_to_nanos(some_date),
                    timestamp_to_nanos(some_date - 8760.hours()),
                    timestamp_to_nanos(some_date + 8760.hours()),
                ],
            ],
            nullability,
        );

        convert_timestamp_column(nano_column, unit)
    }
}

fn timestamp_to_nanos(ts: jiff::Timestamp) -> i64 {
    ts.as_nanosecond()
        .try_into()
        .expect("timestamp is too large")
}

fn convert_timestamp_column(nano_column: TestColumn, unit: TimeUnit) -> TestColumn {
    if unit == TimeUnit::Nanosecond {
        nano_column
    } else {
        TestColumn::new(
            cast(nano_column.array.as_ref(), &DataType::Timestamp(unit, None))
                .expect("timestamp column cast failed"),
            nano_column.field.is_nullable(),
        )
    }
}

/// A temporary session context populated with a "test" dataframe and constructed from a bunch
/// of test columns.
struct TestSessionContext {
    ctx: SessionContext,
}

impl TestSessionContext {
    fn new(columns: impl IntoIterator<Item = TestColumn>) -> Self {
        let ctx = SessionContext::new();

        let (fields, arrays): (Vec<_>, Vec<_>) =
            columns.into_iter().map(|c| (c.field, c.array)).unzip();

        let schema = Arc::new(Schema::new_with_metadata(fields, Default::default()));

        ctx.register_table(
            "__test",
            Arc::new(
                MemTable::try_new(
                    Arc::clone(&schema),
                    vec![vec![
                        RecordBatch::try_new_with_options(schema, arrays, &Default::default())
                            .expect("failed to create the record batch"),
                    ]],
                )
                .expect("failed to create mem table"),
            ),
        )
        .expect("failed to register table");

        Self { ctx }
    }

    async fn df(&self) -> DataFrame {
        self.ctx
            .table("__test")
            .await
            .expect("test table not found")
    }

    async fn to_filtered_record_batch(&self, filter: &ColumnFilter) -> RecordBatch {
        let df = self.df().await;

        let filter_expr = filter
            .as_filter_expression()
            .expect("couldn't create an expression from filter");

        let mut record_batches = df
            .filter(filter_expr)
            .expect("failed to apply filter")
            .collect()
            .await
            .expect("failed to collect");

        assert_eq!(record_batches.len(), 1);

        record_batches
            .pop()
            .expect("we just checked that there is one record batch")
    }
}

#[derive(Debug)]
#[expect(dead_code)] // debug is excluded from dead code analysis
struct TestResult<'a> {
    filter: TypedFilter,
    field: FieldRef,
    unfiltered: ArrayRef,
    filtered: &'a ArrayRef,
}

//note: this is a macro so insta is exposed to the actual test function
macro_rules! filter_snapshot {
    ($filter_op:expr, $test_column:expr, $case:expr) => {
        let filter: TypedFilter = $filter_op.into();
        let column_filter = ColumnFilter::new(Arc::clone(&$test_column.field), filter.clone());
        let initial_field = Arc::clone(&$test_column.field);
        let initial_column = Arc::clone(&$test_column.array);

        let result = TestSessionContext::new([$test_column])
            .to_filtered_record_batch(&column_filter)
            .await;

        assert_eq!(result.columns().len(), 1);
        assert_eq!(initial_field.as_ref(), result.schema().field(0));

        let final_column = result.column(0);


        let test_results = TestResult {
            filter,
            field: initial_field,
            unfiltered: initial_column,
            filtered: final_column,
        };

        insta::with_settings!({
           snapshot_suffix => $case,
        },
        {
            insta::assert_debug_snapshot!(test_results);
        });
    };
}

#[tokio::test]
async fn test_int_compares() {
    let ints = TestColumn::primitive::<Int64Type>(vec![1, 2, 3, 4, 5]);
    let ints_nulls =
        TestColumn::primitive_nulls::<Int64Type>(vec![Some(1), Some(2), None, Some(4), Some(5)]);

    for op in ComparisonOperator::VARIANTS {
        filter_snapshot!(
            IntFilter::new(*op, Some(3)),
            ints.clone(),
            format!("{}_3", op.as_ascii())
        );

        filter_snapshot!(
            IntFilter::new(*op, Some(4)),
            ints_nulls.clone(),
            format!("nulls_{}_4", op.as_ascii())
        );

        filter_snapshot!(
            IntFilter::new(*op, None),
            ints_nulls.clone(),
            format!("nulls_{}_unspecified", op.as_ascii())
        );
    }
}

/// Make sure we correctly handle all integer types.
#[tokio::test]
async fn test_int_all_types() {
    macro_rules! test_int_all_types_impl {
        ($ty:tt) => {
            filter_snapshot!(
                IntFilter::new(ComparisonOperator::Eq, Some(3)),
                TestColumn::primitive::<$ty>(vec![1, 2, 3, 4, 5]),
                format!("{:?}", $ty {})
            )
        };
    }

    test_int_all_types_impl!(Int8Type);
    test_int_all_types_impl!(Int16Type);
    test_int_all_types_impl!(Int32Type);
    test_int_all_types_impl!(Int64Type);
    test_int_all_types_impl!(UInt8Type);
    test_int_all_types_impl!(UInt16Type);
    test_int_all_types_impl!(UInt32Type);
    test_int_all_types_impl!(UInt64Type);
}

#[tokio::test]
async fn test_int_lists() {
    let data = vec![
        vec![1, 2, 3],
        vec![2],
        vec![2, 2],
        vec![4, 5, 6],
        vec![7, 4, 9],
        vec![5, 2, 1],
    ];
    let int_lists = TestColumn::primitive_lists::<Int64Type>(&data, Nullability::NONE);
    let int_lists_nulls = TestColumn::primitive_lists::<Int64Type>(&data, Nullability::BOTH);

    for op in ComparisonOperator::VARIANTS {
        filter_snapshot!(
            IntFilter::new(*op, Some(2)),
            int_lists.clone(),
            format!("{}_2", op.as_ascii())
        );

        filter_snapshot!(
            IntFilter::new(*op, Some(2)),
            int_lists_nulls.clone(),
            format!("nulls_{}_2", op.as_ascii())
        );
    }
}

#[tokio::test]
async fn test_float_compares() {
    let floats = TestColumn::primitive::<Float64Type>(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    let floats_nulls = TestColumn::primitive_nulls::<Float64Type>(vec![
        Some(1.0),
        Some(2.0),
        None,
        Some(4.0),
        Some(5.0),
    ]);

    for op in ComparisonOperator::VARIANTS {
        filter_snapshot!(
            FloatFilter::new(*op, Some(3.0)),
            floats.clone(),
            format!("{}_3.0", op.as_ascii())
        );

        filter_snapshot!(
            FloatFilter::new(*op, Some(4.0)),
            floats_nulls.clone(),
            format!("nulls_{}_4", op.as_ascii())
        );
    }
}

/// Make sure we correctly handle all float types.
#[tokio::test]
async fn test_float_all_types() {
    macro_rules! test_float_all_types_impl {
        ($ty:tt) => {
            filter_snapshot!(
                FloatFilter::new(ComparisonOperator::Eq, Some(3.0)),
                TestColumn::primitive::<$ty>(vec![1.0, 2.0, 3.0, 4.0, 5.0]),
                format!("{:?}", $ty {})
            )
        };
    }

    test_float_all_types_impl!(Float32Type);
    test_float_all_types_impl!(Float64Type);
}

#[tokio::test]
async fn test_float_lists() {
    let data = vec![
        vec![1.0, 2.0, 3.0],
        vec![2.0],
        vec![2.0, 2.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 4.0, 9.0],
        vec![5.0, 2.0, 1.0],
    ];
    let float_lists = TestColumn::primitive_lists::<Float64Type>(&data, Nullability::NONE);
    let float_lists_nulls = TestColumn::primitive_lists::<Float64Type>(&data, Nullability::BOTH);

    for op in ComparisonOperator::VARIANTS {
        filter_snapshot!(
            FloatFilter::new(*op, Some(2.0)),
            float_lists.clone(),
            format!("{}_2.0", op.as_ascii())
        );

        filter_snapshot!(
            FloatFilter::new(*op, Some(2.0)),
            float_lists_nulls.clone(),
            format!("nulls_{}_2.0", op.as_ascii())
        );
    }
}

#[tokio::test]
async fn test_string_contains() {
    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, String::new()),
        TestColumn::strings(),
        "empty"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, "a".to_owned()),
        TestColumn::strings(),
        "a"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, "a".to_owned()),
        TestColumn::strings(),
        "ab"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, "A".to_owned()),
        TestColumn::strings(),
        "a_uppercase"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, String::new()),
        TestColumn::strings_nulls(),
        "nulls_empty"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::Contains, "a".to_owned()),
        TestColumn::strings_nulls(),
        "nulls_a"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::StartsWith, "b".to_owned()),
        TestColumn::strings(),
        "starts_with_b"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::StartsWith, "b".to_owned()),
        TestColumn::strings_nulls(),
        "nulls_starts_with_b"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::EndsWith, "c".to_owned()),
        TestColumn::strings(),
        "ends_with_c"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::EndsWith, "c".to_owned()),
        TestColumn::strings_nulls(),
        "nulls_ends_with_c"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::DoesNotContain, "b".to_owned()),
        TestColumn::strings(),
        "does_not_contain_b"
    );

    filter_snapshot!(
        StringFilter::new(StringOperator::DoesNotContain, "b".to_owned()),
        TestColumn::strings_nulls(),
        "nulls_does_not_contain_b"
    );
}

#[tokio::test]
async fn test_string_list() {
    for op in StringOperator::VARIANTS {
        for &nullability in Nullability::ALL {
            filter_snapshot!(
                StringFilter::new(*op, "ab".to_owned()),
                TestColumn::strings_lists(nullability),
                format!("{nullability:?}_{op:?}_ab")
            );
        }
    }
}

/// Non-nullable filter should work regardless of nullability.
#[tokio::test]
async fn test_non_nullable_boolean_equals() {
    filter_snapshot!(
        NonNullableBooleanFilter::IsTrue,
        TestColumn::bools(),
        "true"
    );

    filter_snapshot!(
        NonNullableBooleanFilter::IsFalse,
        TestColumn::bools(),
        "false"
    );

    filter_snapshot!(
        NonNullableBooleanFilter::IsTrue,
        TestColumn::bools_nulls(),
        "nulls_true"
    );

    filter_snapshot!(
        NonNullableBooleanFilter::IsFalse,
        TestColumn::bools_nulls(),
        "nulls_false"
    );
}

#[tokio::test]
async fn test_nullable_boolean_equals() {
    filter_snapshot!(
        NullableBooleanFilter::new_is_true(),
        TestColumn::bools_nulls(),
        "nulls_true"
    );

    filter_snapshot!(
        NullableBooleanFilter::new_is_false(),
        TestColumn::bools_nulls(),
        "nulls_false"
    );

    filter_snapshot!(
        NullableBooleanFilter::new_is_null(),
        TestColumn::bools_nulls(),
        "nulls_null"
    );

    filter_snapshot!(
        NullableBooleanFilter::new_is_true().with_is_not(),
        TestColumn::bools_nulls(),
        "nulls_is_not_true"
    );

    filter_snapshot!(
        NullableBooleanFilter::new_is_null().with_is_not(),
        TestColumn::bools_nulls(),
        "nulls_is_not_null"
    );
}

#[tokio::test]
async fn test_boolean_equals_list_non_nullable() {
    for &nullability in Nullability::ALL {
        filter_snapshot!(
            NonNullableBooleanFilter::IsTrue,
            TestColumn::bool_lists(nullability),
            format!("{nullability:?}_true")
        );
    }
}

#[tokio::test]
async fn test_boolean_equals_list_nullable() {
    let filters = [
        (NullableBooleanFilter::new_is_true(), "is_true"),
        (
            NullableBooleanFilter::new_is_true().with_is_not(),
            "is_not_true",
        ),
        (NullableBooleanFilter::new_is_null(), "is_null"),
    ];

    // Note: NullableBooleanFilter doesn't support Nullability::NONE, but that's ok because
    // NonNullableBooleanFilter is used in this case.
    for (filter, filter_str) in filters {
        for nullability in [Nullability::BOTH, Nullability::INNER, Nullability::OUTER] {
            filter_snapshot!(
                filter.clone(),
                TestColumn::bool_lists(nullability),
                format!("{nullability:?}_{filter_str}")
            );
        }
    }
}

const ALL_TIME_UNITS: &[TimeUnit] = &[
    TimeUnit::Second,
    TimeUnit::Millisecond,
    TimeUnit::Microsecond,
    TimeUnit::Nanosecond,
];

#[tokio::test]
async fn test_timestamps() {
    // Note: this test intends to cover all column datatypes. It does not intend to cover all kinds
    // of timestamp filtering, which is already covered by unit tests.

    let some_date = jiff::Timestamp::from_str(SOME_TIMESTAMP).expect("valid");

    let all_filters = [
        (TimestampFilter::after(some_date), "after"),
        (TimestampFilter::after(some_date).with_is_not(), "not_after"),
        (
            TimestampFilter::after(some_date + 1.seconds()),
            "after_strict",
        ),
        (
            TimestampFilter::between(some_date - 168.hours(), some_date - 2.hours()),
            "between",
        ),
    ];

    for time_unit in ALL_TIME_UNITS {
        for (filter, case) in &all_filters {
            for nullable in [true, false] {
                filter_snapshot!(
                    filter.clone(),
                    if nullable {
                        TestColumn::timestamps_nulls(*time_unit)
                    } else {
                        TestColumn::timestamps(*time_unit)
                    },
                    format!(
                        "{case}_{time_unit:?}{}",
                        if nullable { "_nulls" } else { "" }
                    )
                );
            }
        }
    }
}

#[tokio::test]
async fn test_timestamps_list() {
    // Note: this test intends to cover all column datatypes. It does not intend to cover all kinds
    // of timestamp filtering, which is already covered by unit tests.

    let some_date = jiff::Timestamp::from_str(SOME_TIMESTAMP).expect("valid");

    let all_filters = [
        (TimestampFilter::after(some_date), "after"),
        (TimestampFilter::after(some_date).with_is_not(), "not_after"),
        (
            TimestampFilter::after(some_date + 1.seconds()),
            "after_strict",
        ),
        (
            TimestampFilter::between(some_date - 168.hours(), some_date - 2.hours()),
            "between",
        ),
    ];

    for (filter, case) in &all_filters {
        for &nullability in Nullability::ALL {
            filter_snapshot!(
                filter.clone(),
                TestColumn::timestamps_lists(TimeUnit::Nanosecond, nullability),
                format!("{case}_{nullability:?}")
            );
        }
    }
}
