use std::collections::HashSet;
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, ListArray, RecordBatchOptions, StringArray};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Fields};
use datafusion::common::DataFusionError;
use datafusion::physical_expr::expressions::col;
use datafusion::physical_expr::{LexOrdering, PhysicalSortExpr};
use itertools::Itertools as _;

use crate::{ArrowArrayDowncastRef as _, RecordBatchExt as _};

// --

pub trait RecordBatchTestExt {
    /// Formats a record batch in a snapshot-friendly way.
    fn format_snapshot(&self, transposed: bool) -> String;

    /// Formats a record batch's schema in a snapshot-friendly way.
    fn format_schema_snapshot(&self) -> String;

    /// Sort columns by field name.
    fn horizontally_sorted(&self) -> Self;

    /// Sort property columns lexicographically.
    ///
    /// This is useful because there is no guarantee on property ordering in partition tables and
    /// dataset manifest.
    ///
    /// Well, in practice there is no guarantee at all, but the base columns have a consistent,
    /// logical order, and it's nice to keep it in the snapshots while we can.
    fn sort_property_columns(&self) -> Self;

    fn sort_rows_by(&self, columns: &[&str]) -> Result<Self, DataFusionError>
    where
        Self: Sized;

    /// Sort the rows of the record batch in ascending order based on the column
    /// order in the schema. To make unit tests consistent when there are no
    /// guarantees on record batch ordering, this function is useful to ensure
    /// consistent results.
    fn auto_sort_rows(&self) -> Result<Self, DataFusionError>
    where
        Self: Sized;

    /// Returns a copy of `self` with only the specified columns, in the specified order.
    ///
    /// Returns `None` if any of the specified columns are missing.
    fn with_columns(&self, columns: &[&str]) -> Option<Self>
    where
        Self: Sized;

    /// Replaces the specified column containing strings (`StringArray`) with
    /// a new column containing the specified string. This will fail if column
    /// `column_name` is not a `StringArray`.
    fn replace_str(&self, column_name: &str, from: &str, to: &str) -> Self;

    /// Redacts values for the specified columns and replaces the redacted value
    /// ("redacted" for strings, 0 for ints, etc).
    /// This is useful when dealing with dynamic columns such as columns containing
    /// timestamp, where you still want to ensure that column has a non-null value.
    /// If existing value is null, then it will stay null.
    fn redact(&self, columns: &[&str]) -> Self;

    /// Returns a copy of `self` with only the specified columns, in the specified order.
    ///
    /// Missing columns are ignored.
    fn project_columns(&self, columns: &[&str]) -> Self;

    /// Returns a copy of `self` with only the columns that start with the specified prefix
    fn filter_columns_by_prefix(&self, prefix: &str) -> Self;

    /// Returns a copy of `self` with the specified columns removed.
    ///
    /// Missing columns are ignored.
    fn remove_columns(&self, columns: &[&str]) -> Self;
}

impl RecordBatchTestExt for arrow::array::RecordBatch {
    fn format_snapshot(&self, transposed: bool) -> String {
        crate::format_record_batch_opts(
            self,
            &crate::RecordBatchFormatOpts {
                transposed,
                width: Some(800),
                include_metadata: false,
                include_column_metadata: false,
                // 40 greatly improves readability while keeping all IDs fully visible (e.g. `chunk_NNN`).
                max_cell_content_width: 40,
                ..Default::default()
            },
        )
        .to_string()
    }

    #[inline]
    fn format_schema_snapshot(&self) -> String {
        self.schema().format_snapshot()
    }

    fn horizontally_sorted(&self) -> Self {
        self.clone()
            .sort_columns_by(|f1, f2| f1.name().cmp(f2.name()))
            .expect("should be able to sort")
    }

    fn sort_property_columns(&self) -> Self {
        self.clone()
            .sort_columns_by(|f1, f2| {
                if f1.name().starts_with("property:") && f2.name().starts_with("property:") {
                    f1.name().cmp(f2.name())
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .expect("should be able to sort")
    }

    fn sort_rows_by(&self, columns: &[&str]) -> Result<Self, DataFusionError> {
        let sort_exprs = columns
            .iter()
            .map(|column| {
                Ok(PhysicalSortExpr::new(
                    col(column, self.schema_ref())?,
                    SortOptions::default(),
                ))
            })
            .collect::<Result<Vec<_>, DataFusionError>>()?;

        let Some(ordering) = LexOrdering::new(sort_exprs) else {
            return Ok(self.clone());
        };

        datafusion::physical_plan::sorts::sort::sort_batch(self, &ordering, None)
    }

    fn auto_sort_rows(&self) -> Result<Self, DataFusionError> {
        let sort_exprs = self
            .schema()
            .fields()
            .iter()
            .map(|column| {
                Ok(PhysicalSortExpr::new(
                    col(column.name(), self.schema_ref())?,
                    SortOptions::default(),
                ))
            })
            .collect::<Result<Vec<_>, DataFusionError>>()?;

        let Some(ordering) = LexOrdering::new(sort_exprs) else {
            return Ok(self.clone());
        };

        datafusion::physical_plan::sorts::sort::sort_batch(self, &ordering, None)
    }

    fn with_columns(&self, columns: &[&str]) -> Option<Self>
    where
        Self: Sized,
    {
        let mut fields = Vec::new();
        let mut arrays = Vec::new();

        let schema = self.schema();
        for column in columns {
            let (_, field) = schema.column_with_name(column)?;
            fields.push(field.clone());

            let array = self.column_by_name(column)?;
            arrays.push(array.clone());
        }

        let schema = arrow::datatypes::Schema::new_with_metadata(fields, schema.metadata().clone());
        Some(
            Self::try_new_with_options(Arc::new(schema), arrays, &RecordBatchOptions::default())
                .expect("creating record batch"),
        )
    }

    fn replace_str(&self, column_name: &str, from: &str, to: &str) -> Self {
        let schema = self.schema();
        schema
            .field_with_name(column_name)
            .expect("Column not found in schema");

        let mut arrays: Vec<ArrayRef> = Vec::new();
        for column in schema.fields() {
            let array = self.column_by_name(column.name()).expect("no such column");

            if column.name() == column_name {
                // Only transform the specified column
                let string_array = array
                    .try_downcast_array_ref::<StringArray>()
                    .expect("expected column to be StringArray");

                let new_values = string_array
                    .iter()
                    .map(|opt| opt.map(|s| s.replace(from, to)))
                    .collect_vec();

                arrays.push(Arc::new(StringArray::from(new_values)) as ArrayRef);
            } else {
                // Keep other columns as-is
                arrays.push(array.clone());
            }
        }

        if schema.fields().is_empty() {
            Self::new_empty(schema)
        } else {
            Self::try_new_with_options(schema, arrays, &RecordBatchOptions::default())
                .expect("creation should succeed")
        }
    }

    fn redact(&self, columns: &[&str]) -> Self {
        let mut arrays = Vec::new();

        let schema = self.schema();
        for column in schema.fields() {
            let array = self.column_by_name(column.name()).expect("no such column");

            if !columns.contains(&column.name().as_str()) {
                arrays.push(array.clone());
                continue;
            }

            macro_rules! redact_array {
                ($array:expr, $array_type:ty, $redact_fn:expr) => {{
                    let typed_array = $array
                        .try_downcast_array_ref::<$array_type>()
                        .expect(concat!("expected column to be ", stringify!($array_type)));

                    let redacted_values = typed_array.iter().map($redact_fn).collect_vec();

                    Arc::new(<$array_type>::from(redacted_values)) as ArrayRef
                }};
            }

            match column.data_type() {
                arrow::datatypes::DataType::Utf8 => {
                    arrays.push(redact_array!(array, StringArray, |opt| opt.map(|_| "redacted")));
                }
                arrow::datatypes::DataType::Int64 => {
                    arrays
                        .push(redact_array!(array, arrow::array::Int64Array, |opt| opt.map(|_| 0)));
                }
                arrow::datatypes::DataType::UInt64 => {
                    arrays
                        .push(redact_array!(array, arrow::array::UInt64Array, |opt| opt.map(|_| 0)));
                }
                arrow::datatypes::DataType::List(field) => {
                    let list_array = array
                        .try_downcast_array_ref::<arrow::array::ListArray>()
                        .expect("expected column to be ListArray");

                    let (redacted_values, inner_field) = match field.data_type() {
                        arrow::datatypes::DataType::Utf8 => {
                            let redacted = redact_array!(
                                list_array.values(),
                                arrow::array::StringArray,
                                |opt| opt.map(|_| "redacted")
                            );

                            let field = Arc::new(Field::new("item", DataType::Utf8, true));

                            (redacted, field)
                        }
                        arrow::datatypes::DataType::Int64 => {
                            let redacted = redact_array!(
                                list_array.values(),
                                arrow::array::Int64Array,
                                |opt| opt.map(|_| 0)
                            );

                            let field = Arc::new(Field::new("item", DataType::Int64, true));

                            (redacted, field)
                        }
                        _ => {
                            panic!(
                                "Redaction not implemented for type {} inside a List",
                                field.data_type()
                            );
                        }
                    };

                    let offsets = list_array.offsets();
                    let list_nulls = list_array.nulls().cloned();

                    let redacted_list = ListArray::try_new(
                        inner_field,
                        offsets.clone(),
                        Arc::new(redacted_values),
                        list_nulls,
                    )
                    .expect("Failed to create ListArray");

                    arrays.push(Arc::new(redacted_list) as ArrayRef);
                }
                arrow::datatypes::DataType::Binary => {
                    arrays.push(redact_array!(array, arrow::array::BinaryArray, |opt| opt
                        .map(|_| [0u8; 8].as_slice())));
                }
                // TODO(zehiko) add support for other types as needed
                _ => {
                    panic!("Redaction not implemented for type {}", column.data_type());
                }
            }
        }

        if schema.fields().is_empty() {
            Self::new_empty(schema.clone())
        } else {
            Self::try_new_with_options(schema.clone(), arrays, &RecordBatchOptions::default())
                .expect("creation should succeed")
        }
    }

    /// Remove the named columns.
    fn remove_columns(&self, columns: &[&str]) -> Self {
        self.clone()
            .filter_columns_by(|field| !columns.contains(&field.name().as_str()))
            .expect("should be able to filter")
    }

    /// Only keep the named columns.
    fn project_columns(&self, columns: &[&str]) -> Self {
        let col_idx = |field: &Field| columns.iter().position(|c| c == field.name());

        self.clone()
            .filter_columns_by(|field| columns.contains(&field.name().as_str()))
            .expect("should be able to filter")
            .sort_columns_by(|f1, f2| col_idx(f1).cmp(&col_idx(f2)))
            .expect("should be able to sort")
    }

    fn filter_columns_by_prefix(&self, prefix: &str) -> Self {
        self.clone()
            .filter_columns_by(|field| field.name().starts_with(prefix))
            .expect("should be able to filter")
    }
}

pub trait SchemaTestExt {
    /// Formats a record batch in a snapshot-friendly way.
    fn format_snapshot(&self) -> String;
}

impl SchemaTestExt for arrow::datatypes::Schema {
    fn format_snapshot(&self) -> String {
        let metadata = (!self.metadata().is_empty()).then(|| {
            format!(
                "top-level metadata: [\n    {}\n]",
                self.metadata()
                    .iter()
                    .map(|(k, v)| format!("{k}: {v:?}"))
                    .sorted()
                    .join("\n    ")
            )
        });

        let mut fields = self.fields.iter().collect_vec();
        fields.sort_by(|a, b| a.name().cmp(b.name()));
        let fields = fields.into_iter().map(|field| {
            if field.metadata().is_empty() {
                format!(
                    "{}: {}{}",
                    field.name(),
                    if field.is_nullable() { "nullable " } else { "" },
                    crate::format_data_type(field.data_type())
                )
            } else {
                format!(
                    "{}: {}{} [\n    {}\n]",
                    field.name(),
                    if field.is_nullable() { "nullable " } else { "" },
                    crate::format_data_type(field.data_type()),
                    field
                        .metadata()
                        .iter()
                        .map(|(k, v)| format!("{k}: {v:?}"))
                        .sorted()
                        .join("\n    ")
                )
            }
        });

        metadata.into_iter().chain(fields).join("\n")
    }
}

pub trait FieldsTestExt {
    /// Returns true if all the required fields are present, regardless of the order.
    fn contains_unordered(
        &self,
        required_fields: impl IntoIterator<Item = impl AsRef<Field>>,
    ) -> bool;
}

impl FieldsTestExt for Fields {
    fn contains_unordered(
        &self,
        required_fields: impl IntoIterator<Item = impl AsRef<Field>>,
    ) -> bool {
        let fields = self.iter().map(|f| f.as_ref()).collect::<HashSet<_>>();

        required_fields
            .into_iter()
            .all(|f| fields.contains(f.as_ref()))
    }
}
