use arrow::datatypes::Field as ArrowField;

use crate::{MetadataExt as _, SorbetError};

/// The type of column in a sorbet batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColumnKind {
    RowId,
    Index,
    Component,
}

impl TryFrom<&ArrowField> for ColumnKind {
    type Error = SorbetError;

    fn try_from(fields: &ArrowField) -> Result<Self, Self::Error> {
        let kind = fields.get_opt("rerun.kind").unwrap_or("data");
        match kind {
            "control" | "row_id" => Ok(Self::RowId),
            "index" | "time" => Ok(Self::Index),
            "component" | "data" => Ok(Self::Component),

            _ => Err(SorbetError::custom(format!("Unknown column kind: {kind}"))),
        }
    }
}
