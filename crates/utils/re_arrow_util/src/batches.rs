use std::sync::Arc;

use arrow::{
    array::RecordBatch,
    datatypes::{Schema, SchemaBuilder},
};
use itertools::Itertools as _;

// ---

/// Returns a new [`RecordBatch`] where all *top-level* fields are nullable.
///
/// ⚠️ This is *not* recursive! E.g. for a `StructArray` containing 2 fields, only the field
/// corresponding to the `StructArray` itself will be made nullable.
pub fn make_batch_nullable(batch: &RecordBatch) -> RecordBatch {
    let schema = Schema::new_with_metadata(
        batch
            .schema()
            .fields
            .iter()
            .map(|field| (**field).clone().with_nullable(true))
            .collect_vec(),
        batch.schema().metadata.clone(),
    );

    #[allow(clippy::unwrap_used)] // cannot fail, we just made things more permissible
    batch.clone().with_schema(Arc::new(schema)).unwrap()
}

/// Concatenates the given [`RecordBatch`]es, regardless of their respective schema.
///
/// The final schema will be the merge of all the input schemas.
///
/// This will fail if the concatenation requires backfilling null values into non-nullable column.
/// You probably want to call [`make_batch_nullable`] first.
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
                RecordBatch::try_new(schema_merged.clone(), columns)
            })
            .collect();
        batches_patched?
    };

    arrow::compute::concat_batches(&schema_merged, &batches_patched)
}

/// Add a new key/value pair to the metadata of a [`RecordBatch`],
/// replacing any existing value for that key.
// TODO(apache/arrow-rs#7628): this should be built into Arrow, but it isn't yet.
#[must_use]
pub fn insert_metadata(
    record_batch: RecordBatch,
    key: impl Into<String>,
    value: impl Into<String>,
) -> RecordBatch {
    let mut new_schema = std::sync::Arc::unwrap_or_clone(record_batch.schema());
    new_schema.metadata.insert(key.into(), value.into());

    // cannot fail because the new schema is always a superset of the old
    #[allow(clippy::unwrap_used)]
    record_batch.with_schema(Arc::new(new_schema)).unwrap()
}

#[cfg(test)]
mod tests {
    #![expect(clippy::disallowed_methods)]

    use std::sync::Arc;

    use arrow::{
        array::{BooleanArray, Int32Array, RecordBatch, StringArray, StructArray, UInt64Array},
        datatypes::{DataType, Field, Schema},
    };

    use super::*;

    #[test]
    fn make_batch_nullable_basics() {
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
            let schema = Schema::new(vec![
                col1_schema.clone(),
                col2_schema.clone(),
                col3_schema.clone(),
            ]);

            let col1 = Int32Array::from_iter_values([1]);
            let col2 = StringArray::from_iter_values(["col".to_owned()]);
            let col3_1 = BooleanArray::from(vec![true]);
            let col3_2 = UInt64Array::from_iter_values([42]);
            let col3 = StructArray::new(
                vec![col3_1_schema, col3_2_schema].into(),
                vec![Arc::new(col3_1), Arc::new(col3_2)],
                None,
            );

            RecordBatch::try_new(
                Arc::new(schema),
                vec![Arc::new(col1), Arc::new(col2), Arc::new(col3)],
            )
            .unwrap()
        };

        let expected = Schema::new(vec![
            col1_schema.clone(),
            col2_schema.clone(),
            col3_schema.clone(),
        ]);
        assert_eq!(expected, *batch.schema());

        let batch_patched = make_batch_nullable(&batch);

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

            Schema::new(vec![
                col1_schema.clone(),
                col2_schema.clone(),
                col3_schema.clone(),
            ])
        };
        assert_eq!(expected, *batch_patched.schema());
    }

    #[test]
    fn concat_polymorphic_batches_basics() {
        let col1_schema = Field::new("col1", DataType::Int32, false);
        let col2_schema = Field::new("col2", DataType::Utf8, false);
        let col3_schema = Field::new("col3", DataType::Boolean, false);
        let col4_schema = Field::new("col4", DataType::UInt64, false);

        let batch1 = {
            let schema = Schema::new(vec![col1_schema, col2_schema.clone()]);

            let col1 = Int32Array::from_iter_values([1]);
            let col2 = StringArray::from_iter_values(["col".to_owned()]);

            RecordBatch::try_new(Arc::new(schema), vec![Arc::new(col1), Arc::new(col2)]).unwrap()
        };
        let batch2 = {
            let schema = Schema::new(vec![col3_schema, col4_schema.clone()]);

            let col3 = BooleanArray::from(vec![true]);
            let col4 = UInt64Array::from_iter_values([42]);

            RecordBatch::try_new(Arc::new(schema), vec![Arc::new(col3), Arc::new(col4)]).unwrap()
        };
        let batch3 = {
            let schema = Schema::new(vec![col2_schema, col4_schema]);

            let col2 = StringArray::from_iter_values(["super-col".to_owned()]);
            let col4 = UInt64Array::from_iter_values([43]);

            RecordBatch::try_new(Arc::new(schema), vec![Arc::new(col2), Arc::new(col4)]).unwrap()
        };

        // This will fail, because we have to insert null values to do the concatenation, and our
        // columns don't allow for that right now.
        let batches = &[batch1.clone(), batch2.clone(), batch3.clone()];
        assert!(concat_polymorphic_batches(batches).is_err());

        let batches = &[
            make_batch_nullable(&batch1),
            make_batch_nullable(&batch2),
            make_batch_nullable(&batch3),
        ];
        let batch_concat = concat_polymorphic_batches(batches).unwrap();

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
}
