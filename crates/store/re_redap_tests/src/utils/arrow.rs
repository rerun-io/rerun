use std::sync::Arc;

use arrow::compute::SortOptions;
use arrow::{
    array::{ArrayRef, ListArray, StringArray},
    datatypes::{DataType, Field, Schema},
};
use datafusion::common::DataFusionError;
use datafusion::physical_expr::expressions::col;
use datafusion::physical_expr::{LexOrdering, PhysicalSortExpr};
use itertools::Itertools as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::ArrowArray as _;

// --

pub trait RecordBatchExt {
    /// Formats a record batch in a snapshot-friendly way.
    fn format_snapshot(&self, transposed: bool) -> String;

    /// Formats a record batch's schema in a snapshot-friendly way.
    fn format_schema_snapshot(&self) -> String;

    fn horizontally_sorted(&self) -> Self;

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
    fn filtered_columns(&self, columns: &[&str]) -> Self;

    /// Returns copy of self with only the columns that start with the specified prefix
    fn filtered_columns_by_prefix(&self, prefix: &str) -> Self;

    /// Returns a copy of `self` with the specified columns removed.
    ///
    /// Missing columns are ignored.
    fn unfiltered_columns(&self, columns: &[&str]) -> Self;
}

impl RecordBatchExt for arrow::array::RecordBatch {
    fn format_snapshot(&self, transposed: bool) -> String {
        re_format_arrow::format_record_batch_opts(
            self,
            &re_format_arrow::RecordBatchFormatOpts {
                transposed,
                width: Some(800),
                include_metadata: false,
                include_column_metadata: false,
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
        let schema = self.schema();

        let mut fields_and_columns =
            itertools::izip!(schema.fields.iter(), self.columns()).collect_vec();
        fields_and_columns.sort_by_key(|(field, _column)| field.name());

        let (fields, columns): (Vec<_>, Vec<_>) = fields_and_columns.into_iter().unzip();

        Self::try_new(
            Arc::new(Schema::new_with_metadata(
                fields.into_iter().cloned().collect_vec(),
                schema.metadata.clone(),
            )),
            columns.into_iter().cloned().collect_vec(),
        )
        .unwrap()
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

        let schema = arrow::datatypes::Schema::new(fields);
        Some(Self::try_new(Arc::new(schema), arrays).expect("creating record batch"))
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
            Self::try_new(schema, arrays).expect("creation should succeed")
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
            Self::try_new(schema.clone(), arrays).expect("creation should succeed")
        }
    }

    fn unfiltered_columns(&self, columns: &[&str]) -> Self {
        let schema = self.schema();
        let columns = schema
            .fields()
            .iter()
            .filter_map(|field| {
                let name = field.name().as_str();
                (!columns.contains(&name)).then_some(name)
            })
            .collect_vec();
        self.filtered_columns(&columns)
    }

    fn filtered_columns(&self, columns: &[&str]) -> Self {
        let mut fields = Vec::new();
        let mut arrays = Vec::new();

        let schema = self.schema();
        for column in columns {
            let Some((_, field)) = schema.column_with_name(column) else {
                continue;
            };
            fields.push(field.clone());

            let Some(array) = self.column_by_name(column) else {
                continue;
            };
            arrays.push(array.clone());
        }

        let schema = arrow::datatypes::Schema::new(fields);
        if schema.fields().is_empty() {
            Self::new_empty(Arc::new(schema))
        } else {
            Self::try_new(Arc::new(schema), arrays).expect("creation should succeed")
        }
    }

    fn filtered_columns_by_prefix(&self, prefix: &str) -> Self {
        let mut fields = Vec::new();
        let mut arrays = Vec::new();

        let schema = self.schema();
        for column in schema.fields() {
            if column.name().starts_with(prefix) {
                fields.push(column.clone());

                let Some(array) = self.column_by_name(column.name()) else {
                    continue;
                };
                arrays.push(array.clone());
            }
        }

        let schema = arrow::datatypes::Schema::new(fields);
        if schema.fields().is_empty() {
            Self::new_empty(Arc::new(schema))
        } else {
            Self::try_new(Arc::new(schema), arrays).expect("creation should succeed")
        }
    }
}

pub trait SchemaExt {
    /// Formats a record batch in a snapshot-friendly way.
    fn format_snapshot(&self) -> String;
}

impl SchemaExt for arrow::datatypes::Schema {
    fn format_snapshot(&self) -> String {
        let mut fields = self.fields.iter().collect_vec();
        fields.sort_by(|a, b| a.name().cmp(b.name()));
        fields
            .into_iter()
            .map(|field| {
                if field.metadata().is_empty() {
                    format!(
                        "{}: {}",
                        field.name(),
                        re_arrow_util::format_data_type(field.data_type())
                    )
                } else {
                    format!(
                        "{}: {} [\n    {}\n]",
                        field.name(),
                        re_arrow_util::format_data_type(field.data_type()),
                        field
                            .metadata()
                            .iter()
                            .map(|(k, v)| format!("{k}:{v}"))
                            .sorted()
                            .join("\n    ")
                    )
                }
            })
            .join("\n")
    }
}
