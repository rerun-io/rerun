use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_types_core::{Component as _, Loggable as _, RowId};

use crate::MetadataExt as _;

#[derive(thiserror::Error, Debug)]
#[error("Wrong datatype. Expected {expected:?}, got {actual:?}")]
pub struct WrongDatatypeError {
    pub expected: ArrowDatatype,
    pub actual: ArrowDatatype,
}

impl WrongDatatypeError {
    pub fn compare_expected_actual(
        expected: &ArrowDatatype,
        actual: &ArrowDatatype,
    ) -> Result<(), Self> {
        if expected == actual {
            Ok(())
        } else {
            Err(Self {
                expected: expected.clone(),
                actual: actual.clone(),
            })
        }
    }
}

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
                "rerun.kind".to_owned(),
                crate::ColumnKind::RowId.to_string(),
            ),
            (
                "ARROW:extension:name".to_owned(),
                re_tuid::Tuid::ARROW_EXTENSION_NAME.to_owned(),
            ),
            (
                // Storing the metadata as JSON as is the convention…
                "ARROW:extension:metadata".to_owned(),
                r#"{ "namespace": "row" }"#.to_owned(), // for row_ prefix
            ),
        ]);
        if *is_sorted {
            metadata.insert("rerun.is_sorted".to_owned(), "true".to_owned());
        }

        let nullable = false; // All rows has an id
        ArrowField::new(
            RowId::descriptor().to_string(),
            RowId::arrow_datatype(),
            nullable,
        )
        .with_metadata(metadata)
    }

    #[allow(clippy::unused_self)]
    pub fn datatype(&self) -> ArrowDatatype {
        RowId::arrow_datatype()
    }
}

impl TryFrom<&ArrowField> for RowIdColumnDescriptor {
    type Error = WrongDatatypeError;

    fn try_from(field: &ArrowField) -> Result<Self, Self::Error> {
        WrongDatatypeError::compare_expected_actual(&RowId::arrow_datatype(), field.data_type())?;
        Ok(Self {
            is_sorted: field.metadata().get_bool("rerun.is_sorted"),
        })
    }
}
