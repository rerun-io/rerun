use std::borrow::Borrow;

use arrow::datatypes::{Schema, SchemaBuilder};
use arrow::error::ArrowError;

/// Merges schemas field by field, widening fields as [`arrow::datatypes::Field::try_merge`] allows.
///
/// Fields keep the order in which they are first seen, so the order of `schemas` decides the order
/// of the result.
///
/// This is how the schemas of a dataset's layers combine into a dataset or segment schema, and
/// servers and clients must all merge this way to agree on its fields and their types.
///
/// Unlike [`Schema::try_merge`], top-level metadata is dropped instead of merged, so schemas whose
/// fields are compatible always merge, whatever metadata they carry.
pub fn merge_schema_fields<S: Borrow<Schema>>(
    schemas: impl IntoIterator<Item = S>,
) -> Result<Schema, ArrowError> {
    let mut merger = SchemaFieldsMerger::default();
    for schema in schemas {
        merger = merger.try_merge(schema.borrow())?;
    }
    Ok(merger.finish())
}

/// Streaming version of [`merge_schema_fields`].
#[derive(Default)]
#[must_use]
pub struct SchemaFieldsMerger {
    builder: SchemaBuilder,
}

impl SchemaFieldsMerger {
    /// Merges the fields of `schema` into the fields merged so far.
    ///
    /// Consumes the merger because a failed merge may leave some fields of `schema` merged.
    pub fn try_merge(mut self, schema: &Schema) -> Result<Self, ArrowError> {
        for field in schema.fields() {
            self.builder.try_merge(field)?;
        }
        Ok(self)
    }

    /// The merged schema, without top-level metadata.
    pub fn finish(self) -> Schema {
        self.builder.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::datatypes::{DataType, Field};

    use super::*;

    fn schema(fields: Vec<Field>, metadata: &[(&str, &str)]) -> Schema {
        Schema::new_with_metadata(
            fields,
            metadata
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        )
    }

    #[test]
    fn matches_try_merge_without_top_level_metadata() {
        let lhs = schema(
            vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Utf8, true),
            ],
            &[],
        );
        let rhs = schema(
            vec![
                Field::new("c", DataType::Float64, true),
                Field::new("a", DataType::Int32, true),
            ],
            &[],
        );

        assert_eq!(
            merge_schema_fields([&lhs, &rhs]).unwrap(),
            Schema::try_merge([lhs, rhs]).unwrap()
        );
    }

    #[test]
    fn drops_top_level_metadata() {
        let lhs = schema(vec![Field::new("a", DataType::Int32, true)], &[("k", "1")]);
        let rhs = schema(vec![Field::new("a", DataType::Int32, true)], &[("k", "2")]);

        let merged = merge_schema_fields([lhs, rhs]).unwrap();
        assert_eq!(merged.metadata, HashMap::new());
        assert_eq!(merged.fields().len(), 1);
    }

    #[test]
    fn rejects_incompatible_fields() {
        let lhs = schema(vec![Field::new("a", DataType::Int32, true)], &[]);
        let rhs = schema(vec![Field::new("a", DataType::Utf8, true)], &[]);

        assert!(merge_schema_fields([lhs, rhs]).is_err());
    }

    #[test]
    fn no_schemas_merge_to_empty() {
        assert_eq!(
            merge_schema_fields(std::iter::empty::<Schema>()).unwrap(),
            Schema::empty()
        );
    }
}
