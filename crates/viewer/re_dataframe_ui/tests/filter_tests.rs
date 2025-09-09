use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, Int64Builder, ListArray, ListBuilder,
    StringArray,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use datafusion::catalog::MemTable;
use datafusion::prelude::{DataFrame, SessionContext};

use re_dataframe_ui::{ComparisonOperator, Filter, FilterOperation};
use re_viewer_context::external::tokio;

/// A single column of test data, with convenient constructors.
struct TestColumn {
    field: Field,
    array: ArrayRef,
}

impl TestColumn {
    fn new(name: impl Into<String>, array: impl Array + 'static, nullable: bool) -> Self {
        let name = name.into();
        let array = Arc::new(array) as ArrayRef;
        let field = Field::new(name, array.data_type().clone(), nullable);

        Self { array, field }
    }

    fn ints() -> Self {
        Self::new("int", Int64Array::from(vec![1, 2, 3, 4, 5]), false)
    }

    fn ints_nulls() -> Self {
        Self::new(
            "int",
            Int64Array::from(vec![Some(1), Some(2), None, Some(4), Some(5)]),
            true,
        )
    }

    fn int_lists(inner_nullable: bool, outer_nullable: bool) -> Self {
        let values_builder = Int64Builder::new();
        let mut builder = ListBuilder::new(values_builder);

        builder.values().append_value(1);
        builder.append(true);

        builder.values().append_value(2);
        builder.append(true);

        builder.append(true);

        builder.values().append_value(3);
        builder.values().append_value(4);
        builder.values().append_value(5);
        builder.append(true);

        if outer_nullable {
            builder.append(false);
        }

        if inner_nullable {
            builder.values().append_null();
            builder.append(true);

            builder.values().append_null();
            builder.values().append_value(6);
            builder.append(true);

            builder.values().append_value(7);
            builder.values().append_null();
            builder.append(true);
        }
        let int_lists = builder.finish();

        Self::new("int_list", int_lists, outer_nullable)
    }

    fn floats(nullable: bool) -> Self {
        Self::new(
            "float",
            Float64Array::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]),
            nullable,
        )
    }

    fn floats_nulls() -> Self {
        Self::new(
            "float",
            Float64Array::from(vec![Some(1.0), Some(2.0), None, Some(4.0), Some(5.0)]),
            true,
        )
    }

    fn float_lists(inner_nullable: bool, outer_nullable: bool) -> Self {
        let values = Float64Array::from(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let offsets = OffsetBuffer::new(vec![0i32, 2, 4, 6].into());
        let float_lists = ListArray::try_new(
            Arc::new(Field::new("item", DataType::Float64, inner_nullable)),
            offsets,
            Arc::new(values),
            None,
        )
        .expect("failed to create a float list array");

        Self::new("float_list", float_lists, outer_nullable)
    }

    fn strings(nullable: bool) -> Self {
        Self::new(
            "string",
            StringArray::from(vec!["a", "b", "c", "ab", "A B", "aBc"]),
            nullable,
        )
    }

    fn strings_nulls() -> Self {
        Self::new(
            "string",
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

        Self::new("string_list", strings_lists, outer_nullable)
    }

    fn bools(nullable: bool) -> Self {
        Self::new(
            "bool",
            BooleanArray::from(vec![true, true, false]),
            nullable,
        )
    }

    fn bools_nulls() -> Self {
        Self::new(
            "bool",
            BooleanArray::from(vec![Some(true), Some(true), None, Some(false)]),
            true,
        )
    }

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

        Self::new("bool_list", bool_lists, outer_nullable)
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

macro_rules! filter_snapshot {
    ($filter:expr, $col:expr, $case:expr) => {
        let filter = $filter;
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
    for op in ComparisonOperator::ALL {
        filter_snapshot!(
            Filter::new(
                "int",
                FilterOperation::IntCompares {
                    operator: *op,
                    value: 3
                }
            ),
            TestColumn::ints(),
            format!("{}3", op.operator_text())
        );

        filter_snapshot!(
            Filter::new(
                "int",
                FilterOperation::IntCompares {
                    operator: *op,
                    value: 4
                }
            ),
            TestColumn::ints_nulls(),
            format!("nulls_{}4", op.operator_text())
        );
    }
}

#[tokio::test]
async fn test_string_contains() {
    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains(String::new())),
        TestColumn::strings(false),
        "empty"
    );

    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains("a".to_owned())),
        TestColumn::strings(false),
        "a"
    );

    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains("A".to_owned())),
        TestColumn::strings(false),
        "a_uppercase"
    );

    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains("A".to_owned())),
        TestColumn::strings(true),
        "nullable_a_uppercase"
    );

    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains(String::new())),
        TestColumn::strings_nulls(),
        "nulls_empty"
    );

    filter_snapshot!(
        Filter::new("string", FilterOperation::StringContains("a".to_owned())),
        TestColumn::strings_nulls(),
        "nulls_a"
    );
}

#[tokio::test]
async fn test_string_contains_list() {
    filter_snapshot!(
        Filter::new(
            "string_list",
            FilterOperation::StringContains("c".to_owned())
        ),
        TestColumn::strings_lists(true, true),
        "inner_outer_nullable_c"
    );

    filter_snapshot!(
        Filter::new(
            "string_list",
            FilterOperation::StringContains("c".to_owned())
        ),
        TestColumn::strings_lists(true, false),
        "inner_nullable_c"
    );

    filter_snapshot!(
        Filter::new(
            "string_list",
            FilterOperation::StringContains("c".to_owned())
        ),
        TestColumn::strings_lists(false, true),
        "outer_nullable_c"
    );

    filter_snapshot!(
        Filter::new(
            "string_list",
            FilterOperation::StringContains("c".to_owned())
        ),
        TestColumn::strings_lists(false, false),
        "c"
    );
}

#[tokio::test]
async fn test_boolean_equals() {
    filter_snapshot!(
        Filter::new("bool", FilterOperation::BooleanEquals(true)),
        TestColumn::bools(false),
        "true"
    );

    filter_snapshot!(
        Filter::new("bool", FilterOperation::BooleanEquals(false)),
        TestColumn::bools(false),
        "false"
    );

    filter_snapshot!(
        Filter::new("bool", FilterOperation::BooleanEquals(true)),
        TestColumn::bools(true),
        "nullable_true"
    );

    filter_snapshot!(
        Filter::new("bool", FilterOperation::BooleanEquals(true)),
        TestColumn::bools_nulls(),
        "nulls_true"
    );

    filter_snapshot!(
        Filter::new("bool", FilterOperation::BooleanEquals(false)),
        TestColumn::bools_nulls(),
        "nulls_false"
    );
}

#[tokio::test]
async fn test_boolean_equals_list() {
    filter_snapshot!(
        Filter::new("bool_list", FilterOperation::BooleanEquals(true)),
        TestColumn::bool_lists(true, true),
        "inner_outer_nullable_true"
    );

    filter_snapshot!(
        Filter::new("bool_list", FilterOperation::BooleanEquals(true)),
        TestColumn::bool_lists(true, false),
        "inner_nullable_true"
    );

    filter_snapshot!(
        Filter::new("bool_list", FilterOperation::BooleanEquals(true)),
        TestColumn::bool_lists(false, true),
        "nullable_true"
    );

    filter_snapshot!(
        Filter::new("bool_list", FilterOperation::BooleanEquals(true)),
        TestColumn::bool_lists(false, false),
        "true"
    );
}
