use std::collections::HashSet;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, UInt64Array};
use arrow::datatypes::{Field, Schema, SchemaBuilder};
use itertools::Itertools as _;

use crate::MissingColumnError;

/// Takes rows from a [`RecordBatch`] at the specified indices.
///
/// This is a convenience wrapper around [`arrow::compute::take_record_batch`]
/// that accepts `usize` indices instead of requiring a specific Arrow array type.
pub fn take_record_batch(
    batch: &RecordBatch,
    indices: &[usize],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let indices: UInt64Array = indices.iter().map(|&i| i as u64).collect();
    arrow::compute::take_record_batch(batch, &indices)
}

// ---

/// Concatenates the given [`RecordBatch`]es, regardless of their respective schema.
///
/// The final schema will be the merge of all the input schemas.
///
/// This will fail if the concatenation requires backfilling null values into non-nullable column.
/// You probably want to call [`RecordBatchExt::make_nullable`] first.
pub fn concat_polymorphic_batches(batches: &[RecordBatch]) -> arrow::error::Result<RecordBatch> {
    if batches.is_empty() {
        return Ok(RecordBatch::new_empty(Arc::new(Schema::empty())));
    }

    let schema_merged = {
        let mut schema_builder = SchemaBuilder::new();
        for batch in batches {
            for field in &batch.schema().fields {
                schema_builder.try_merge(field)?;
            }

            let md_merged = schema_builder.metadata_mut();
            for (k, v) in batch.schema_ref().metadata() {
                if let Some(previous) = md_merged.insert(k.clone(), v.clone())
                    && previous != *v
                {
                    return Err(arrow::error::ArrowError::SchemaError(format!(
                        "incompatible schemas cannot be merged (conflicting metadata for {k:?})"
                    )));
                }
            }
        }

        Arc::new(schema_builder.finish())
    };

    let batches_patched = {
        let batches_patched: arrow::error::Result<Vec<RecordBatch>> = batches
            .iter()
            .map(|batch| {
                // TODO(cmc): I'm doing this manually because `RecordBatch::with_schema` just
                // doesn't seem to work? It will fail with "not a superset" for schemas that are
                // very clearly a superset, so I don't know, whatever.
                let columns = schema_merged
                    .fields
                    .iter()
                    .map(|field| {
                        if let Some(col) = batch.column_by_name(field.name()) {
                            col.clone()
                        } else {
                            Arc::new(arrow::array::new_null_array(
                                field.data_type(),
                                batch.num_rows(),
                            ))
                        }
                    })
                    .collect_vec();
                RecordBatch::try_new_with_options(
                    schema_merged.clone(),
                    columns,
                    &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
                )
            })
            .collect();

        batches_patched?
    };

    arrow::compute::concat_batches(&schema_merged, &batches_patched)
}

pub trait RecordBatchExt {
    /// Helper for [`RecordBatchExt`].
    fn inner(&self) -> &RecordBatch;

    /// Returns a new [`RecordBatch`] where all *top-level* fields are nullable.
    ///
    /// ⚠️ This is *not* recursive! E.g. for a `StructArray` containing 2 fields, only the field
    /// corresponding to the `StructArray` itself will be made nullable.
    fn make_nullable(&self) -> RecordBatch;

    /// Concatenate the given [`RecordBatch`]es horizontally.
    ///
    /// Both batches must have the same number of rows, and a non-overlapping schema.
    fn concat_horizontally_with(
        &self,
        right_batch: &RecordBatch,
    ) -> arrow::error::Result<RecordBatch>;

    /// Reorders the columns of a [`RecordBatch`] based on a comparison function.
    fn sort_columns_by(
        self,
        cmp_fn: impl Fn(&Field, &Field) -> std::cmp::Ordering,
    ) -> arrow::error::Result<RecordBatch>;

    /// Retain columns based on the provided predicate.
    fn filter_columns_by(
        self,
        predicate: impl Fn(&Field) -> bool,
    ) -> arrow::error::Result<RecordBatch>;

    /// Project columns based on the provided column names.
    ///
    /// If a column is not found, or if a column is duplicated, returns an
    /// [`arrow::error::ArrowError::InvalidArgumentError`] error.
    fn project_columns<'a, I>(self, projected_columns: I) -> arrow::error::Result<RecordBatch>
    where
        I: Iterator<Item = &'a str>;

    /// Rename columns based on the provided (original, new) pairs.
    fn rename_columns(self, renames: &[(&str, &str)]) -> arrow::error::Result<RecordBatch>;

    /// Get a column by name, with a nice error message otherwise
    fn try_get_column(&self, name: &str) -> Result<&ArrayRef, MissingColumnError> {
        self.inner()
            .column_by_name(name)
            .ok_or_else(|| MissingColumnError {
                missing: name.to_owned(),
                available: self
                    .inner()
                    .schema()
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect(),
            })
    }
}

impl RecordBatchExt for RecordBatch {
    fn inner(&self) -> &RecordBatch {
        self
    }

    fn make_nullable(&self) -> RecordBatch {
        let schema = Schema::new_with_metadata(
            self.schema()
                .fields
                .iter()
                .map(|field| (**field).clone().with_nullable(true))
                .collect_vec(),
            self.schema().metadata.clone(),
        );

        #[expect(clippy::unwrap_used)] // cannot fail, we just made things more permissible
        self.clone().with_schema(Arc::new(schema)).unwrap()
    }

    /// Concatenate the given [`RecordBatch`]es horizontally.
    ///
    /// `other_batch` is added to the right of `self`. Both batches must have the same number of
    /// rows, and a non-overlapping schema.
    fn concat_horizontally_with(
        &self,
        other_batch: &RecordBatch,
    ) -> arrow::error::Result<RecordBatch> {
        if self.num_rows() != other_batch.num_rows() {
            return Err(arrow::error::ArrowError::InvalidArgumentError(
                "RecordBatches must have the same number of rows".to_owned(),
            ));
        }

        let merged_schema = Schema::try_merge([
            Arc::unwrap_or_clone(self.schema()),
            Arc::unwrap_or_clone(other_batch.schema()),
        ])?;

        if merged_schema.fields().len()
            != self.schema().fields().len() + other_batch.schema().fields().len()
        {
            return Err(arrow::error::ArrowError::InvalidArgumentError(
                "RecordBatches must have a non-overlapping schema".to_owned(),
            ));
        }

        let mut columns: Vec<ArrayRef> = Vec::new();
        columns.extend_from_slice(self.columns());
        columns.extend_from_slice(other_batch.columns());

        Self::try_new_with_options(
            Arc::new(merged_schema),
            columns,
            &RecordBatchOptions::default().with_row_count(Some(self.num_rows())),
        )
    }

    fn sort_columns_by(
        self,
        cmp_fn: impl Fn(&Field, &Field) -> std::cmp::Ordering,
    ) -> arrow::error::Result<RecordBatch> {
        let (schema_ref, columns, row_count) = self.into_parts();
        let Schema { fields, metadata } = Arc::unwrap_or_clone(schema_ref);

        let (fields, columns): (Vec<_>, Vec<_>) = fields
            .iter()
            .map(Arc::clone)
            .zip(columns)
            .sorted_by(|(left_field, _), (right_field, _)| {
                cmp_fn(left_field.as_ref(), right_field.as_ref())
            })
            .unzip();

        Self::try_new_with_options(
            Arc::new(Schema::new_with_metadata(fields, metadata)),
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    fn filter_columns_by(
        self,
        predicate: impl Fn(&Field) -> bool,
    ) -> arrow::error::Result<RecordBatch> {
        let (schema_ref, columns, row_count) = self.into_parts();
        let Schema { fields, metadata } = Arc::unwrap_or_clone(schema_ref);

        let (new_fields, new_columns): (Vec<_>, Vec<_>) = fields
            .iter()
            .map(Arc::clone)
            .zip(columns)
            .filter(|(field, _)| predicate(field))
            .unzip();

        Self::try_new_with_options(
            Arc::new(Schema::new_with_metadata(new_fields, metadata)),
            new_columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    fn project_columns<'a, I>(self, projected_columns: I) -> arrow::error::Result<RecordBatch>
    where
        I: Iterator<Item = &'a str>,
    {
        let (schema_ref, columns, row_count) = self.into_parts();
        let Schema { fields, metadata } = Arc::unwrap_or_clone(schema_ref);

        let mut seen_columns = HashSet::with_capacity(projected_columns.size_hint().0);
        let (new_columns, new_fields): (Vec<_>, Vec<_>) = projected_columns
            .map(|col_name| {
                let (col_index, field) =
                    fields
                        .find(col_name)
                        .ok_or(arrow::error::ArrowError::InvalidArgumentError(format!(
                            "projected column '{col_name}' not found in schema"
                        )))?;

                // Check for duplicate projected column names and return an error if found.
                if seen_columns.contains(col_name) {
                    return Err(arrow::error::ArrowError::InvalidArgumentError(format!(
                        "projected column '{col_name}' was requested twice"
                    )));
                } else {
                    seen_columns.insert(col_name);
                }

                columns
                    .get(col_index)
                    .map(|col| (Arc::clone(col), Arc::clone(field)))
                    .ok_or_else(|| {
                        arrow::error::ArrowError::InvalidArgumentError(format!(
                            "internal error: column index '{col_index}' out of bounds"
                        ))
                    })
            })
            .process_results(|iter| iter.unzip())?;

        Self::try_new_with_options(
            Arc::new(Schema::new_with_metadata(new_fields, metadata)),
            new_columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    fn rename_columns(self, renames: &[(&str, &str)]) -> arrow::error::Result<RecordBatch> {
        let (schema_ref, columns, row_count) = self.into_parts();
        let Schema { fields, metadata } = Arc::unwrap_or_clone(schema_ref);

        let new_fields: Vec<_> = fields
            .iter()
            .map(|f| {
                for (original_name, new_name) in renames {
                    if f.name() == *original_name {
                        return Arc::new(f.as_ref().clone().with_name(*new_name));
                    }
                }
                Arc::clone(f)
            })
            .collect();

        Self::try_new_with_options(
            Arc::new(Schema::new_with_metadata(new_fields, metadata)),
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::disallowed_methods)]

    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow::array::{
        BooleanArray, Float64Array, Int32Array, RecordBatch, StringArray, StructArray, UInt64Array,
    };
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::error::ArrowError;

    use super::*;

    #[test]
    fn make_nullable_basics() {
        let col1_schema = Field::new("col1", DataType::Int32, true);
        let col2_schema = Field::new("col2", DataType::Utf8, false);
        let col3_1_schema = Field::new("col3", DataType::Boolean, false);
        let col3_2_schema = Field::new("col4", DataType::UInt64, true);
        let col3_schema = Field::new(
            "col4",
            DataType::Struct(vec![col3_1_schema.clone(), col3_2_schema.clone()].into()),
            false,
        );

        let batch = {
            let schema = Schema::new_with_metadata(
                vec![
                    col1_schema.clone(),
                    col2_schema.clone(),
                    col3_schema.clone(),
                ],
                HashMap::default(),
            );

            let col1 = Int32Array::from_iter_values([1]);
            let col2 = StringArray::from_iter_values(["col".to_owned()]);
            let col3_1 = BooleanArray::from(vec![true]);
            let col3_2 = UInt64Array::from_iter_values([42]);
            let col3 = StructArray::new(
                vec![col3_1_schema, col3_2_schema].into(),
                vec![Arc::new(col3_1), Arc::new(col3_2)],
                None,
            );

            RecordBatch::try_new_with_options(
                Arc::new(schema),
                vec![Arc::new(col1), Arc::new(col2), Arc::new(col3)],
                &RecordBatchOptions::default().with_row_count(Some(1)),
            )
            .unwrap()
        };

        let expected = Schema::new_with_metadata(
            vec![
                col1_schema.clone(),
                col2_schema.clone(),
                col3_schema.clone(),
            ],
            HashMap::default(),
        );
        assert_eq!(expected, *batch.schema());

        let batch_patched = batch.make_nullable();

        let expected = {
            let col1_schema = Field::new("col1", DataType::Int32, true);
            let col2_schema = Field::new("col2", DataType::Utf8, true);
            let col3_1_schema = Field::new("col3", DataType::Boolean, false); // still false
            let col3_2_schema = Field::new("col4", DataType::UInt64, true);
            let col3_schema = Field::new(
                "col4",
                DataType::Struct(vec![col3_1_schema.clone(), col3_2_schema.clone()].into()),
                true,
            );

            Schema::new_with_metadata(
                vec![
                    col1_schema.clone(),
                    col2_schema.clone(),
                    col3_schema.clone(),
                ],
                HashMap::default(),
            )
        };
        assert_eq!(expected, *batch_patched.schema());
    }

    #[test]
    fn concat_polymorphic_batches_basics() {
        let col1_schema = Field::new("col1", DataType::Int32, false);
        let col2_schema = Field::new("col2", DataType::Utf8, false);
        let col3_schema = Field::new("col3", DataType::Boolean, false);
        let col4_schema = Field::new("col4", DataType::UInt64, false);

        let options = RecordBatchOptions::default().with_row_count(Some(1));
        let batch1 = {
            let schema = Schema::new_with_metadata(
                vec![col1_schema, col2_schema.clone()],
                HashMap::default(),
            )
            .with_metadata(std::iter::once(("batch1".to_owned(), "yes".to_owned())).collect());

            let col1 = Int32Array::from_iter_values([1]);
            let col2 = StringArray::from_iter_values(["col".to_owned()]);

            RecordBatch::try_new_with_options(
                Arc::new(schema),
                vec![Arc::new(col1), Arc::new(col2)],
                &options,
            )
            .unwrap()
        };
        let batch2 = {
            let schema = Schema::new_with_metadata(
                vec![col3_schema, col4_schema.clone()],
                HashMap::default(),
            )
            .with_metadata(std::iter::once(("batch2".to_owned(), "no".to_owned())).collect());

            let col3 = BooleanArray::from(vec![true]);
            let col4 = UInt64Array::from_iter_values([42]);

            RecordBatch::try_new_with_options(
                Arc::new(schema),
                vec![Arc::new(col3), Arc::new(col4)],
                &options,
            )
            .unwrap()
        };
        let batch3 = {
            let schema =
                Schema::new_with_metadata(vec![col2_schema, col4_schema], HashMap::default())
                    .with_metadata(
                        [
                            ("batch1".to_owned(), "yes".to_owned()),
                            ("batch2".to_owned(), "no".to_owned()),
                            ("batch3".to_owned(), "maybe".to_owned()),
                        ]
                        .into_iter()
                        .collect(),
                    );

            let col2 = StringArray::from_iter_values(["super-col".to_owned()]);
            let col4 = UInt64Array::from_iter_values([43]);

            RecordBatch::try_new_with_options(
                Arc::new(schema),
                vec![Arc::new(col2), Arc::new(col4)],
                &options,
            )
            .unwrap()
        };

        // This will fail, because we have to insert null values to do the concatenation, and our
        // columns don't allow for that right now.
        let batches = &[batch1.clone(), batch2.clone(), batch3.clone()];
        assert!(concat_polymorphic_batches(batches).is_err());

        let batches = &[
            batch1.make_nullable(),
            batch2.make_nullable(),
            batch3.make_nullable(),
        ];
        let mut batch_concat = concat_polymorphic_batches(batches).unwrap();

        // We must compare metadata on its own, because it's a vanilla HashMap: snapshots
        // have undefined order.
        assert_eq!(
            *batch_concat.schema_ref().metadata(),
            [
                ("batch1".to_owned(), "yes".to_owned()),
                ("batch2".to_owned(), "no".to_owned()),
                ("batch3".to_owned(), "maybe".to_owned()),
            ]
            .into_iter()
            .collect::<HashMap<String, String>>(),
        );
        batch_concat.schema_metadata_mut().clear();

        insta::assert_debug_snapshot!(batch_concat, @r###"
        RecordBatch {
            schema: Schema {
                fields: [
                    Field {
                        name: "col1",
                        data_type: Int32,
                        nullable: true,
                        dict_id: 0,
                        dict_is_ordered: false,
                        metadata: {},
                    },
                    Field {
                        name: "col2",
                        data_type: Utf8,
                        nullable: true,
                        dict_id: 0,
                        dict_is_ordered: false,
                        metadata: {},
                    },
                    Field {
                        name: "col3",
                        data_type: Boolean,
                        nullable: true,
                        dict_id: 0,
                        dict_is_ordered: false,
                        metadata: {},
                    },
                    Field {
                        name: "col4",
                        data_type: UInt64,
                        nullable: true,
                        dict_id: 0,
                        dict_is_ordered: false,
                        metadata: {},
                    },
                ],
                metadata: {},
            },
            columns: [
                PrimitiveArray<Int32>
                [
                  1,
                  null,
                  null,
                ],
                StringArray
                [
                  "col",
                  null,
                  "super-col",
                ],
                BooleanArray
                [
                  null,
                  true,
                  null,
                ],
                PrimitiveArray<UInt64>
                [
                  null,
                  42,
                  43,
                ],
            ],
            row_count: 3,
        }
        "###);
    }

    #[test]
    fn concat_polymorphic_batches_incompatible() {
        let options = RecordBatchOptions::default().with_row_count(Some(1));
        let batch1 = {
            let schema = Schema::empty()
                .with_metadata(std::iter::once(("is_true".to_owned(), "yes".to_owned())).collect());
            RecordBatch::try_new_with_options(Arc::new(schema), vec![], &options).unwrap()
        };
        let mut batch2 = {
            let schema = Schema::empty()
                .with_metadata(std::iter::once(("is_true".to_owned(), "no".to_owned())).collect());
            RecordBatch::try_new_with_options(Arc::new(schema), vec![], &options).unwrap()
        };

        let err = concat_polymorphic_batches(&[batch1.clone(), batch2.clone()]).unwrap_err();
        assert!(matches!(err, ArrowError::SchemaError(_)));

        batch2
            .schema_metadata_mut()
            .insert("is_true".to_owned(), "yes".to_owned());
        assert!(concat_polymorphic_batches(&[batch1, batch2]).is_ok());
    }

    #[test]
    fn test_concat_horizontally_basic() {
        // Create first batch with two columns
        let schema1 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Utf8, false),
            ],
            HashMap::default(),
        ));
        let batch1 = RecordBatch::try_new(
            schema1,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["foo", "bar", "baz"])),
            ],
        )
        .unwrap();

        // Create second batch with two columns
        let schema2 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("c", DataType::Float64, false),
                Field::new("d", DataType::Int32, false),
            ],
            HashMap::default(),
        ));
        let batch2 = RecordBatch::try_new(
            schema2,
            vec![
                Arc::new(Float64Array::from(vec![1.1, 2.2, 3.3])),
                Arc::new(Int32Array::from(vec![10, 20, 30])),
            ],
        )
        .unwrap();

        // Concatenate
        let result = batch1.concat_horizontally_with(&batch2).unwrap();

        // Verify schema
        assert_eq!(result.num_columns(), 4);
        assert_eq!(result.num_rows(), 3);
        assert_eq!(result.schema().field(0).name(), "a");
        assert_eq!(result.schema().field(1).name(), "b");
        assert_eq!(result.schema().field(2).name(), "c");
        assert_eq!(result.schema().field(3).name(), "d");

        // Verify data
        let col_a = result
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(col_a.value(0), 1);
        assert_eq!(col_a.value(1), 2);
        assert_eq!(col_a.value(2), 3);

        let col_d = result
            .column(3)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(col_d.value(0), 10);
        assert_eq!(col_d.value(1), 20);
        assert_eq!(col_d.value(2), 30);
    }

    #[test]
    fn test_concat_horizontally_different_row_counts_fails() {
        let schema1 = Arc::new(Schema::new_with_metadata(
            vec![Field::new("a", DataType::Int32, false)],
            HashMap::default(),
        ));
        let batch1 =
            RecordBatch::try_new(schema1, vec![Arc::new(Int32Array::from(vec![1, 2, 3]))]).unwrap();

        let schema2 = Arc::new(Schema::new_with_metadata(
            vec![Field::new("b", DataType::Int32, false)],
            HashMap::default(),
        ));
        let batch2 = RecordBatch::try_new(
            schema2,
            vec![Arc::new(Int32Array::from(vec![10, 20]))], // Only 2 rows
        )
        .unwrap();

        let result = batch1.concat_horizontally_with(&batch2);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must have the same number of rows")
        );
    }

    #[test]
    fn test_concat_horizontally_empty_batches() {
        let schema1 = Arc::new(Schema::new_with_metadata(
            vec![Field::new("a", DataType::Int32, false)],
            HashMap::default(),
        ));
        let batch1 =
            RecordBatch::try_new(schema1, vec![Arc::new(Int32Array::from(Vec::<i32>::new()))])
                .unwrap();

        let schema2 = Arc::new(Schema::new_with_metadata(
            vec![Field::new("b", DataType::Utf8, false)],
            HashMap::default(),
        ));
        let batch2 = RecordBatch::try_new(
            schema2,
            vec![Arc::new(StringArray::from(Vec::<String>::new()))],
        )
        .unwrap();

        let result = batch1.concat_horizontally_with(&batch2).unwrap();
        assert_eq!(result.num_rows(), 0);
        assert_eq!(result.num_columns(), 2);
    }

    #[test]
    fn test_concat_horizontally_preserves_column_order() {
        let schema1 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("col1", DataType::Int32, false),
                Field::new("col2", DataType::Int32, false),
            ],
            HashMap::default(),
        ));
        let batch1 = RecordBatch::try_new(
            schema1,
            vec![
                Arc::new(Int32Array::from(vec![1])),
                Arc::new(Int32Array::from(vec![2])),
            ],
        )
        .unwrap();

        let schema2 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("col3", DataType::Int32, false),
                Field::new("col4", DataType::Int32, false),
            ],
            HashMap::default(),
        ));
        let batch2 = RecordBatch::try_new(
            schema2,
            vec![
                Arc::new(Int32Array::from(vec![3])),
                Arc::new(Int32Array::from(vec![4])),
            ],
        )
        .unwrap();

        let result = batch1.concat_horizontally_with(&batch2).unwrap();

        // Verify columns appear in order: col1, col2, col3, col4
        assert_eq!(result.schema().field(0).name(), "col1");
        assert_eq!(result.schema().field(1).name(), "col2");
        assert_eq!(result.schema().field(2).name(), "col3");
        assert_eq!(result.schema().field(3).name(), "col4");
    }

    #[test]
    fn test_concat_duplicate_field_names() {
        // Create first batch with column "id"
        let schema1 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("id", DataType::Int32, false),
                Field::new("name", DataType::Utf8, false),
            ],
            HashMap::default(),
        ));
        let batch1 = RecordBatch::try_new(
            schema1,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["foo", "bar", "baz"])),
            ],
        )
        .unwrap();

        // Create second batch that ALSO has column "id"
        let schema2 = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("id", DataType::Int32, false), // Duplicate!
                Field::new("value", DataType::Float64, false),
            ],
            HashMap::default(),
        ));
        let batch2 = RecordBatch::try_new(
            schema2,
            vec![
                Arc::new(Int32Array::from(vec![10, 20, 30])),
                Arc::new(Float64Array::from(vec![1.1, 2.2, 3.3])),
            ],
        )
        .unwrap();

        let result = batch1.concat_horizontally_with(&batch2);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("RecordBatches must have a non-overlapping schema")
        );
    }

    #[test]
    fn test_concat_preserves_metadata() {
        use std::collections::HashMap;

        // Create schema with schema-level metadata
        let mut schema1_metadata = HashMap::new();
        schema1_metadata.insert("source".to_owned(), "batch_data".to_owned());
        schema1_metadata.insert("left_version".to_owned(), "1.0".to_owned());

        let schema1 = Arc::new(
            Schema::new_with_metadata(
                vec![
                    Field::new("id", DataType::Int32, false).with_metadata(HashMap::from([(
                        "field_meta".to_owned(),
                        "id_info".to_owned(),
                    )])),
                    Field::new("name", DataType::Utf8, false),
                ],
                HashMap::default(),
            )
            .with_metadata(schema1_metadata),
        );

        let batch1 = RecordBatch::try_new(
            schema1,
            vec![
                Arc::new(Int32Array::from(vec![1, 2])),
                Arc::new(StringArray::from(vec!["a", "b"])),
            ],
        )
        .unwrap();

        // Create schema with NON-conflicting metadata
        let mut schema2_metadata = HashMap::new();
        schema2_metadata.insert("source".to_owned(), "batch_data".to_owned()); // Same value!
        schema2_metadata.insert("right_timestamp".to_owned(), "2025-10-20".to_owned()); // Different key

        let schema2 = Arc::new(
            Schema::new_with_metadata(
                vec![
                    Field::new("value", DataType::Float64, false)
                        .with_metadata(HashMap::from([("unit".to_owned(), "meters".to_owned())])),
                ],
                HashMap::default(),
            )
            .with_metadata(schema2_metadata),
        );

        let batch2 =
            RecordBatch::try_new(schema2, vec![Arc::new(Float64Array::from(vec![1.5, 2.5]))])
                .unwrap();

        let result = batch1.concat_horizontally_with(&batch2).unwrap();

        // Verify schema-level metadata is merged
        let result_metadata = result.schema_ref().metadata();
        assert_eq!(
            result_metadata.get("source"),
            Some(&"batch_data".to_owned())
        );
        assert_eq!(result_metadata.get("left_version"), Some(&"1.0".to_owned()));
        assert_eq!(
            result_metadata.get("right_timestamp"),
            Some(&"2025-10-20".to_owned())
        );

        // Verify field-level metadata is preserved
        let id_field = result.schema_ref().field(0);
        assert_eq!(id_field.name(), "id");
        assert_eq!(
            id_field.metadata().get("field_meta"),
            Some(&"id_info".to_owned())
        );

        let value_field = result.schema_ref().field(2);
        assert_eq!(value_field.name(), "value");
        assert_eq!(
            value_field.metadata().get("unit"),
            Some(&"meters".to_owned())
        );
    }

    #[test]
    fn test_concat_conflicting_schema_metadata_fails() {
        use std::collections::HashMap;

        // When both schemas have the same metadata key with different values,
        // try_merge REJECTS the merge
        let mut metadata1 = HashMap::new();
        metadata1.insert("owner".to_owned(), "alice".to_owned());

        let schema1 = Arc::new(
            Schema::new_with_metadata(
                vec![Field::new("a", DataType::Int32, false)],
                HashMap::default(),
            )
            .with_metadata(metadata1),
        );

        let mut metadata2 = HashMap::new();
        metadata2.insert("owner".to_owned(), "bob".to_owned()); // Conflict!

        let schema2 = Arc::new(
            Schema::new_with_metadata(
                vec![Field::new("b", DataType::Int32, false)],
                HashMap::default(),
            )
            .with_metadata(metadata2),
        );

        let batch1 =
            RecordBatch::try_new(schema1, vec![Arc::new(Int32Array::from(vec![1, 2]))]).unwrap();

        let batch2 =
            RecordBatch::try_new(schema2, vec![Arc::new(Int32Array::from(vec![3, 4]))]).unwrap();

        // This should fail due to conflicting metadata
        let result = batch1.concat_horizontally_with(&batch2);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("conflicting metadata")
        );
    }

    #[test]
    fn test_sort_columns_by() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("zebra", DataType::Int32, false),
                Field::new("apple", DataType::Utf8, false),
                Field::new("mango", DataType::Int32, false),
            ],
            HashMap::default(),
        ));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef,
                Arc::new(StringArray::from(vec!["a", "b", "c"])) as ArrayRef,
                Arc::new(Int32Array::from(vec![10, 20, 30])) as ArrayRef,
            ],
        )
        .unwrap();

        let sorted = batch
            .sort_columns_by(|a, b| a.name().cmp(b.name()))
            .unwrap();

        let names: Vec<_> = sorted
            .schema_ref()
            .fields()
            .iter()
            .map(|f| f.name().to_owned())
            .collect();
        assert_eq!(names, vec!["apple", "mango", "zebra"]);

        // Verify data moved with columns
        let apple_col = sorted
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(apple_col.value(0), "a");

        let mango_col = sorted
            .column(1)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(mango_col.value(0), 10);
    }

    #[test]
    fn test_sort_columns_by_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_owned(), "value".to_owned());

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("b", DataType::Int32, false),
                Field::new("a", DataType::Int32, false),
            ],
            metadata.clone(),
        ));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1])) as ArrayRef,
                Arc::new(Int32Array::from(vec![2])) as ArrayRef,
            ],
        )
        .unwrap();

        let sorted = batch
            .sort_columns_by(|a, b| a.name().cmp(b.name()))
            .unwrap();

        assert_eq!(sorted.schema_ref().metadata(), &metadata);
    }

    #[test]
    fn test_sort_columns_by_empty_batch() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("a", DataType::Int32, false)],
            HashMap::default(),
        ));
        let batch = RecordBatch::try_new(
            schema,
            vec![Arc::new(Int32Array::from(Vec::<i32>::new())) as ArrayRef],
        )
        .unwrap();

        let sorted = batch
            .sort_columns_by(|a, b| a.name().cmp(b.name()))
            .unwrap();

        assert_eq!(sorted.num_rows(), 0);
        assert_eq!(sorted.num_columns(), 1);
    }

    #[test]
    fn test_filter_columns_basic() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("id", DataType::Int32, false),
                Field::new("name", DataType::Utf8, false),
                Field::new("age", DataType::Int32, false),
            ],
            HashMap::default(),
        ));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
                Arc::new(Int32Array::from(vec![10, 20, 30])),
            ],
        )
        .unwrap();

        // Keep only Int32 columns
        let filtered = batch
            .filter_columns_by(|f| matches!(f.data_type(), DataType::Int32))
            .unwrap();

        assert_eq!(filtered.num_columns(), 2);
        assert_eq!(filtered.schema_ref().field(0).name(), "id");
        assert_eq!(filtered.schema_ref().field(1).name(), "age");
        assert_eq!(filtered.num_rows(), 3);
    }

    #[test]
    fn test_filter_columns_preserves_metadata() {
        let mut metadata = HashMap::default();
        metadata.insert("key".to_owned(), "value".to_owned());

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Utf8, false),
            ],
            metadata.clone(),
        ));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1])),
                Arc::new(StringArray::from(vec!["x"])),
            ],
        )
        .unwrap();

        let filtered = batch
            .filter_columns_by(|f| matches!(f.data_type(), DataType::Int32))
            .unwrap();

        assert_eq!(filtered.schema_ref().metadata(), &metadata);
    }

    #[test]
    fn test_filter_columns_empty_schema() {
        let schema = Arc::new(Schema::empty());

        let batch = RecordBatch::try_new_with_options(
            schema,
            vec![],
            &RecordBatchOptions::default().with_row_count(Some(3)),
        )
        .unwrap();

        assert_eq!(batch.num_columns(), 0);
        assert_eq!(batch.num_rows(), 3);

        let filtered = batch.filter_columns_by(|_| true).unwrap();

        assert_eq!(filtered.num_columns(), 0);
        assert_eq!(filtered.num_rows(), 3);
    }

    fn sample_batch() -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("id", DataType::Int32, false),
                    Field::new("name", DataType::Utf8, false),
                    Field::new("age", DataType::Int32, false),
                ],
                HashMap::default(),
            )),
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["Alice", "Bob", "Charlie"])),
                Arc::new(Int32Array::from(vec![30, 25, 35])),
            ],
        )
        .unwrap()
    }

    #[test]
    fn test_project_basic() {
        let batch = sample_batch();
        let projected = batch
            .project_columns(["name", "id"].iter().copied())
            .unwrap();

        assert_eq!(projected.num_columns(), 2);
        assert_eq!(projected.schema_ref().field(0).name(), "name");
        assert_eq!(projected.schema_ref().field(1).name(), "id");
        assert_eq!(projected.num_rows(), 3);
    }

    #[test]
    fn test_project_preserves_order() {
        let batch = sample_batch();
        let projected = batch
            .project_columns(["age", "name", "id"].iter().copied())
            .unwrap();

        assert_eq!(projected.num_columns(), 3);
        assert_eq!(projected.schema_ref().field(0).name(), "age");
        assert_eq!(projected.schema_ref().field(1).name(), "name");
        assert_eq!(projected.schema_ref().field(2).name(), "id");

        // Verify data matches the reordered columns
        let age_col = projected
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(age_col.value(0), 30);
    }

    #[test]
    fn test_project_column_not_found() {
        let batch = sample_batch();
        let result = batch.project_columns(["name", "missing"].iter().copied());

        match result {
            Err(ArrowError::InvalidArgumentError(msg)) => {
                assert!(msg.contains("column 'missing' not found"));
            }
            _ => panic!("expected InvalidArgumentError"),
        }
    }

    #[test]
    fn test_project_duplicate_column() {
        let batch = sample_batch();
        let result = batch.project_columns(["name", "age", "name"].iter().copied());

        match result {
            Err(ArrowError::InvalidArgumentError(msg)) => {
                assert!(msg.contains("name"));
                assert!(msg.contains("twice") || msg.contains("duplicate"));
            }
            _ => panic!("expected InvalidArgumentError"),
        }
    }

    #[test]
    fn test_project_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_owned(), "value".to_owned());

        let batch = RecordBatch::try_new(
            Arc::new(Schema::new_with_metadata(
                vec![Field::new("id", DataType::Int32, false)],
                metadata.clone(),
            )),
            vec![Arc::new(Int32Array::from(vec![1]))],
        )
        .unwrap();

        let projected = batch.project_columns(std::iter::once("id")).unwrap();
        assert_eq!(projected.schema_ref().metadata(), &metadata);
    }

    #[test]
    fn test_rename_columns_basic() {
        let batch = sample_batch();
        let renamed = batch
            .rename_columns(&[("name", "full_name"), ("age", "years")])
            .unwrap();

        assert_eq!(renamed.num_columns(), 3);
        assert_eq!(renamed.schema_ref().field(0).name(), "id");
        assert_eq!(renamed.schema_ref().field(1).name(), "full_name");
        assert_eq!(renamed.schema_ref().field(2).name(), "years");
        assert_eq!(renamed.num_rows(), 3);

        // Verify data is preserved
        let name_col = renamed
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(name_col.value(0), "Alice");
    }

    #[test]
    fn test_rename_columns_no_renames() {
        let batch = sample_batch();
        let renamed = batch.rename_columns(&[]).unwrap();

        assert_eq!(renamed.schema_ref().field(0).name(), "id");
        assert_eq!(renamed.schema_ref().field(1).name(), "name");
        assert_eq!(renamed.schema_ref().field(2).name(), "age");
    }

    #[test]
    fn test_rename_columns_nonexistent_column() {
        let batch = sample_batch();
        // Renaming a nonexistent column should silently do nothing
        let renamed = batch
            .rename_columns(&[("nonexistent", "something")])
            .unwrap();

        assert_eq!(renamed.schema_ref().field(0).name(), "id");
        assert_eq!(renamed.schema_ref().field(1).name(), "name");
        assert_eq!(renamed.schema_ref().field(2).name(), "age");
    }

    #[test]
    fn test_rename_columns_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_owned(), "value".to_owned());

        let batch = RecordBatch::try_new(
            Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("a", DataType::Int32, false),
                    Field::new("b", DataType::Utf8, false),
                ],
                metadata.clone(),
            )),
            vec![
                Arc::new(Int32Array::from(vec![1])),
                Arc::new(StringArray::from(vec!["x"])),
            ],
        )
        .unwrap();

        let renamed = batch.rename_columns(&[("a", "alpha")]).unwrap();

        assert_eq!(renamed.schema_ref().metadata(), &metadata);
        assert_eq!(renamed.schema_ref().field(0).name(), "alpha");
    }

    #[test]
    fn test_rename_columns_preserves_field_properties() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("col", DataType::Int32, true).with_metadata(
                std::collections::HashMap::from([(
                    "description".to_owned(),
                    "A test column".to_owned(),
                )]),
            )],
            HashMap::default(),
        ));

        let batch =
            RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![1, 2, 3]))]).unwrap();

        let renamed = batch.rename_columns(&[("col", "renamed_col")]).unwrap();

        let field = renamed.schema_ref().field(0);
        assert_eq!(field.name(), "renamed_col");
        assert!(field.is_nullable());
        assert_eq!(field.data_type(), &DataType::Int32);
        assert_eq!(
            field.metadata().get("description"),
            Some(&"A test column".to_owned())
        );
    }
}
