use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_types_core::{Component as _, Loggable as _, RowId};

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

/// Describes the [`RowId`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowIdColumnDescriptor {}

impl RowIdColumnDescriptor {
    pub fn new() -> Self {
        Self {}
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self {} = self;

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
        // TODO: check `rerun.kind`
        Self::try_from(field.data_type())
    }
}

impl TryFrom<&ArrowDatatype> for RowIdColumnDescriptor {
    type Error = WrongDatatypeError;

    fn try_from(data_type: &ArrowDatatype) -> Result<Self, Self::Error> {
        WrongDatatypeError::compare_expected_actual(&RowId::arrow_datatype(), data_type)?;
        Ok(Self {})
    }
}

impl TryFrom<ArrowDatatype> for RowIdColumnDescriptor {
    type Error = WrongDatatypeError;

    fn try_from(data_type: ArrowDatatype) -> Result<Self, Self::Error> {
        WrongDatatypeError::compare_expected_actual(&RowId::arrow_datatype(), &data_type)?;
        Ok(Self {})
    }
}
