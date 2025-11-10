use arrow::datatypes::Field as ArrowField;

use crate::MetadataExt as _;

#[derive(thiserror::Error, Debug)]
#[error(
    "Unknown `rerun:kind` {kind:?} in column {column_name:?}. Expect one of `row_id`, `index`, or `component`."
)]
pub struct UnknownColumnKind {
    pub kind: String,
    pub column_name: String,
}

/// The type of column in a sorbet batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColumnKind {
    /// Row ID
    RowId,

    /// Timeline
    Index,

    /// Data (also the default when unknown)
    #[default]
    Component,
}

impl std::fmt::Display for ColumnKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RowId => write!(f, "control"),
            Self::Index => write!(f, "index"),
            Self::Component => write!(f, "data"),
        }
    }
}

impl TryFrom<&ArrowField> for ColumnKind {
    type Error = UnknownColumnKind;

    fn try_from(field: &ArrowField) -> Result<Self, Self::Error> {
        debug_assert!(
            field.metadata().get("rerun.kind").is_none(),
            "We should have migrated to 'rerun:kind'"
        );

        let Some(kind) = field.get_opt(crate::metadata::RERUN_KIND) else {
            return Ok(Self::default());
        };
        match kind {
            "control" | "row_id" => Ok(Self::RowId),
            "index" | "time" => Ok(Self::Index),
            "component" | "data" => Ok(Self::Component),

            _ => Err(UnknownColumnKind {
                kind: kind.to_owned(),
                column_name: field.name().to_owned(),
            }),
        }
    }
}
