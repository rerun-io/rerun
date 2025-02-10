use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_types_core::{Component as _, Loggable as _, RowId};

#[derive(thiserror::Error, Debug)]
#[error("Wrong datatype. Expected {expected:?}, got {actual:?}")]
pub struct WrongDatatypeError {
    pub expected: ArrowDatatype,
    pub actual: ArrowDatatype,
}

/// Describes the [`RowId`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowIdColumnDescriptor {}

impl RowIdColumnDescriptor {
    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self {} = self;

        let nullable = false; // All rows has an id

        let metadata = [
            Some(("rerun.kind".to_owned(), "control".to_owned())),
            // This ensures the RowId/Tuid is formatted correctly:
            Some((
                "ARROW:extension:name".to_owned(),
                re_tuid::Tuid::ARROW_EXTENSION_NAME.to_owned(),
            )),
        ]
        .into_iter()
        .flatten()
        .collect();

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
        if field.data_type() == &RowId::arrow_datatype() {
            Ok(Self {})
        } else {
            Err(WrongDatatypeError {
                expected: RowId::arrow_datatype(),
                actual: field.data_type().clone(),
            })
        }
    }
}
