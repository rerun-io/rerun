use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, ListArray, ListBuilder, PrimitiveArray, PrimitiveBuilder,
    StringArray,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{
    ArrowPrimitiveType, DataType, Field, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type,
    Int64Type, Schema, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow::record_batch::RecordBatch;
use datafusion::catalog::MemTable;
use datafusion::prelude::{DataFrame, SessionContext};

use re_dataframe_ui::{ComparisonOperator, Filter, FilterOperation};
use re_viewer_context::external::tokio;

const COLUMN_NAME: &str = "column";

/// A single column of test data, with convenient constructors.
#[derive(Debug, Clone)]
struct TestColumn {
    field: Field,
    array: ArrayRef,
}

impl TestColumn {
    fn new(array: impl Array + 'static, nullable: bool) -> Self {
        let array = Arc::new(array) as ArrayRef;
        let field = Field::new(COLUMN_NAME, array.data_type().clone(), nullable);

        Self { array, field }
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
    fn primitive_lists<T>(
        data: &[Vec<T::Native>],
        inner_nullable: bool,
        outer_nullable: bool,
    ) -> Self
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
        if outer_nullable {
            builder.append(false);
        }

        // inject inner nulls
        if inner_nullable {
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

        Self::new(array, outer_nullable)
    }

    fn strings(nullable: bool) -> Self {
        Self::new(
            StringArray::from(vec!["a", "b", "c", "ab", "A B", "aBc"]),
            nullable,
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

    //TODO(ab): rewrite using `primitive_lists`
    fn strings_lists(inner_nullable: bool, outer_nullable: bool) -> Self {
        let values = StringArray::from(vec!["a", "b", "c", "ab", "A B", "aBc"]);
        let offsets = OffsetBuffer::new(vec![0i32, 2, 4, 6].into());
        let strings_lists = ListArray::try_new(
            Arc::new(Field::new("item", DataType::Utf8, inner_nullable)),
            offsets,
            Arc::new(values),
            None,
        )
        .expect("failed to create a string list array");

        Self::new(strings_lists, outer_nullable)
    }

    fn bools(nullable: bool) -> Self {
        Self::new(BooleanArray::from(vec![true, true, false]), nullable)
    }

    fn bools_nulls() -> Self {
        Self::new(
            BooleanArray::from(vec![Some(true), Some(true), None, Some(false)]),
            true,
        )
    }

    //TODO(ab): rewrite using `primitive_lists`
    fn bool_lists(inner_nullable: bool, outer_nullable: bool) -> Self {
        let values = BooleanArray::from(vec![true, false, true, true, false, false, true, false]);
        let offsets = OffsetBuffer::new(vec![0i32, 1, 2, 4, 6, 8].into());
        let bool_lists = ListArray::try_new(
            Arc::new(Field::new("item", DataType::Boolean, inner_nullable)),
            offsets,
            Arc::new(values),
            None,
        )
        .expect("failed to create a bool list array");

        Self::new(bool_lists, outer_nullable)
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

    async fn to_filtered_record_batch(&self, filter: &Filter) -> RecordBatch {
        let df = self.df().await;

        let schema = df.schema();
        let filter_expr = filter
            .as_filter_expression(schema)
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

//TODO(ab): display the unfiltered data as well and make the snapshot less verbose.
macro_rules! filter_snapshot {
    ($filter:expr, $col:expr, $case:expr) => {
        let filter = Filter::new(COLUMN_NAME, $filter);
        let result = TestSessionContext::new([$col])
            .to_filtered_record_batch(&filter)
            .await;

        insta::with_settings!({
           snapshot_suffix => $case,
        },
        {
            insta::assert_debug_snapshot!((filter, result));
        });
    };
}

#[tokio::test]
async fn test_int_compares() {
    let ints = TestColumn::primitive::<Int64Type>(vec![1, 2, 3, 4, 5]);
    let ints_nulls =
        TestColumn::primitive_nulls::<Int64Type>(vec![Some(1), Some(2), None, Some(4), Some(5)]);

    for op in ComparisonOperator::ALL {
        filter_snapshot!(
            FilterOperation::IntCompares {
                operator: *op,
                value: 3
            },
            ints.clone(),
            format!("{}_3", op.as_ascii())
        );

        filter_snapshot!(
            FilterOperation::IntCompares {
                operator: *op,
                value: 4
            },
            ints_nulls.clone(),
            format!("nulls_{}_4", op.as_ascii())
        );
    }
}

/// Make sure we correctly handle all integer types.
#[tokio::test]
async fn test_int_all_types() {
    macro_rules! test_int_all_types_impl {
        ($ty:tt) => {
            filter_snapshot!(
                FilterOperation::IntCompares {
                    operator: ComparisonOperator::Eq,
                    value: 3
                },
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
    let int_lists = TestColumn::primitive_lists::<Int64Type>(&data, false, false);
    let int_lists_nulls = TestColumn::primitive_lists::<Int64Type>(&data, true, true);

    for op in ComparisonOperator::ALL {
        filter_snapshot!(
            FilterOperation::IntCompares {
                operator: *op,
                value: 2
            },
            int_lists.clone(),
            format!("{}_2", op.as_ascii())
        );

        filter_snapshot!(
            FilterOperation::IntCompares {
                operator: *op,
                value: 2
            },
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

    for op in ComparisonOperator::ALL {
        filter_snapshot!(
            FilterOperation::FloatCompares {
                operator: *op,
                value: 3.0
            },
            floats.clone(),
            format!("{}_3.0", op.as_ascii())
        );

        filter_snapshot!(
            FilterOperation::FloatCompares {
                operator: *op,
                value: 4.0
            },
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
                FilterOperation::FloatCompares {
                    operator: ComparisonOperator::Eq,
                    value: 3.0
                },
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
    let float_lists = TestColumn::primitive_lists::<Float64Type>(&data, false, false);
    let float_lists_nulls = TestColumn::primitive_lists::<Float64Type>(&data, true, true);

    for op in ComparisonOperator::ALL {
        filter_snapshot!(
            FilterOperation::FloatCompares {
                operator: *op,
                value: 2.0
            },
            float_lists.clone(),
            format!("{}_2.0", op.as_ascii())
        );

        filter_snapshot!(
            FilterOperation::FloatCompares {
                operator: *op,
                value: 2.0
            },
            float_lists_nulls.clone(),
            format!("nulls_{}_2.0", op.as_ascii())
        );
    }
}

#[tokio::test]
async fn test_string_contains() {
    filter_snapshot!(
        FilterOperation::StringContains(String::new()),
        TestColumn::strings(false),
        "empty"
    );

    filter_snapshot!(
        FilterOperation::StringContains("a".to_owned()),
        TestColumn::strings(false),
        "a"
    );

    filter_snapshot!(
        FilterOperation::StringContains("A".to_owned()),
        TestColumn::strings(false),
        "a_uppercase"
    );

    filter_snapshot!(
        FilterOperation::StringContains("A".to_owned()),
        TestColumn::strings(true),
        "nullable_a_uppercase"
    );

    filter_snapshot!(
        FilterOperation::StringContains(String::new()),
        TestColumn::strings_nulls(),
        "nulls_empty"
    );

    filter_snapshot!(
        FilterOperation::StringContains("a".to_owned()),
        TestColumn::strings_nulls(),
        "nulls_a"
    );
}

#[tokio::test]
async fn test_string_contains_list() {
    filter_snapshot!(
        FilterOperation::StringContains("c".to_owned()),
        TestColumn::strings_lists(true, true),
        "inner_outer_nullable_c"
    );

    filter_snapshot!(
        FilterOperation::StringContains("c".to_owned()),
        TestColumn::strings_lists(true, false),
        "inner_nullable_c"
    );

    filter_snapshot!(
        FilterOperation::StringContains("c".to_owned()),
        TestColumn::strings_lists(false, true),
        "outer_nullable_c"
    );

    filter_snapshot!(
        FilterOperation::StringContains("c".to_owned()),
        TestColumn::strings_lists(false, false),
        "c"
    );
}

#[tokio::test]
async fn test_boolean_equals() {
    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bools(false),
        "true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(false),
        TestColumn::bools(false),
        "false"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bools(true),
        "nullable_true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bools_nulls(),
        "nulls_true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(false),
        TestColumn::bools_nulls(),
        "nulls_false"
    );
}

#[tokio::test]
async fn test_boolean_equals_list() {
    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bool_lists(true, true),
        "inner_outer_nullable_true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bool_lists(true, false),
        "inner_nullable_true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bool_lists(false, true),
        "nullable_true"
    );

    filter_snapshot!(
        FilterOperation::BooleanEquals(true),
        TestColumn::bool_lists(false, false),
        "true"
    );
}
