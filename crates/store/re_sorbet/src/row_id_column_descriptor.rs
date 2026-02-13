use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_arrow_util::WrongDatatypeError;
use re_types_core::{Loggable as _, RowId};

use crate::MetadataExt as _;

/// Describes the schema of the primary [`RowId`] column.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowIdColumnDescriptor {
    /// Are the values in this column sorted?
    ///
    /// `false` means either "unsorted" or "unknown".
    pub is_sorted: bool,
}

impl RowIdColumnDescriptor {
    #[inline]
    pub fn from_sorted(is_sorted: bool) -> Self {
        Self { is_sorted }
    }

    /// Column name, used in Arrow record batches and schemas.
    #[expect(clippy::unused_self)]
    pub fn column_name(&self) -> String {
        RowId::partial_descriptor().to_string()
    }

    /// Short field/column name
    #[inline]
    #[expect(clippy::unused_self)]
    pub fn short_name(&self) -> String {
        RowId::partial_descriptor().display_name().to_owned()
    }

    /// Human-readable name for this column.
    #[inline]
    #[expect(clippy::unused_self)]
    pub fn name(&self) -> &'static str {
        "Row ID"
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self { is_sorted } = self;

        let mut metadata = std::collections::HashMap::from([
            (
                crate::metadata::RERUN_KIND.to_owned(),
                crate::ColumnKind::RowId.to_string(),
            ),
            (
                "ARROW:extension:name".to_owned(),
                re_tuid::Tuid::ARROW_EXTENSION_NAME.to_owned(),
            ),
            (
                // Storing the metadata as JSON as is the convention…
                "ARROW:extension:metadata".to_owned(),
                r#"{"namespace":"row"}"#.to_owned(), // for row_ prefix
            ),
        ]);
        if *is_sorted {
            metadata.insert("rerun:is_sorted".to_owned(), "true".to_owned());
        }

        let nullable = false; // All rows has an id
        ArrowField::new(self.column_name(), RowId::arrow_datatype(), nullable)
            .with_metadata(metadata)
    }

    #[expect(clippy::unused_self)]
    pub fn datatype(&self) -> ArrowDatatype {
        RowId::arrow_datatype()
    }
}

impl TryFrom<&ArrowField> for RowIdColumnDescriptor {
    type Error = WrongDatatypeError;

    fn try_from(field: &ArrowField) -> Result<Self, Self::Error> {
        WrongDatatypeError::ensure_datatype(field, &RowId::arrow_datatype())?;
        Ok(Self {
            is_sorted: field.metadata().get_bool("rerun:is_sorted"),
        })
    }
}
