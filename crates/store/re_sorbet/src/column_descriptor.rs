use arrow::datatypes::{
    DataType as ArrowDatatype, Field as ArrowField, FieldRef as ArrowFieldRef,
    Fields as ArrowFields,
};
use re_log_types::EntityPath;
use re_types_core::{ArchetypeName, ComponentType};

use crate::{ColumnKind, ComponentColumnDescriptor, IndexColumnDescriptor, RowIdColumnDescriptor};

#[derive(thiserror::Error, Debug)]
pub enum ColumnError {
    #[error(transparent)]
    MissingFieldMetadata(#[from] crate::MissingFieldMetadata),

    #[error(transparent)]
    UnknownColumnKind(#[from] crate::UnknownColumnKind),

    #[error("Unsupported column rerun:kind: {kind:?}. Expected one of: index, data")]
    UnsupportedColumnKind { kind: ColumnKind },

    #[error(transparent)]
    UnsupportedTimeType(#[from] crate::UnsupportedTimeType),
}

/// Describes any kind of column.
///
/// See:
/// * [`RowIdColumnDescriptor`]
/// * [`IndexColumnDescriptor`]
/// * [`ComponentColumnDescriptor`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColumnDescriptor {
    /// The primary row id column.
    ///
    /// There should usually only be one of these.
    RowId(RowIdColumnDescriptor),

    /// Index columns (timelines).
    Time(IndexColumnDescriptor),

    /// The actual component data
    Component(ComponentColumnDescriptor),
}

impl ColumnDescriptor {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        match self {
            Self::RowId(_) | Self::Time(_) => {}
            Self::Component(descr) => descr.sanity_check(),
        }
    }

    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::RowId(_) | Self::Time(_) => None,
            Self::Component(descr) => Some(&descr.entity_path),
        }
    }

    #[inline]
    pub fn component_type(&self) -> Option<&ComponentType> {
        match self {
            Self::RowId(_) | Self::Time(_) => None,
            Self::Component(descr) => descr.component_type.as_ref(),
        }
    }

    /// Column name, used in record batches.
    #[inline]
    pub fn column_name(&self, batch_type: crate::BatchType) -> String {
        match self {
            Self::RowId(descr) => descr.column_name(),
            Self::Time(descr) => descr.column_name().to_owned(),
            Self::Component(descr) => descr.column_name(batch_type),
        }
    }

    /// Short and usually unique, used in UI.
    #[inline]
    pub fn display_name(&self) -> String {
        match self {
            Self::RowId(descr) => descr.short_name(),
            Self::Time(descr) => descr.column_name().to_owned(),
            Self::Component(descr) => descr.display_name().to_owned(),
        }
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        match self {
            Self::RowId(_) | Self::Time(_) => false,
            Self::Component(descr) => descr.is_static,
        }
    }

    #[inline]
    pub fn arrow_datatype(&self) -> ArrowDatatype {
        match self {
            Self::RowId(descr) => descr.datatype(),
            Self::Time(descr) => descr.datatype().clone(),
            Self::Component(descr) => descr.returned_datatype(),
        }
    }

    #[inline]
    pub fn to_arrow_field(&self, batch_type: crate::BatchType) -> ArrowField {
        match self {
            Self::RowId(descr) => descr.to_arrow_field(),
            Self::Time(descr) => descr.to_arrow_field(),
            Self::Component(descr) => descr.to_arrow_field(batch_type),
        }
    }

    #[inline]
    pub fn to_arrow_fields(columns: &[Self], batch_type: crate::BatchType) -> ArrowFields {
        columns
            .iter()
            .map(|c| c.to_arrow_field(batch_type))
            .collect()
    }

    /// `chunk_entity_path`: if this column is part of a chunk batch,
    /// what is its entity path (so we can set [`ComponentColumnDescriptor::entity_path`])?
    pub fn from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &[ArrowFieldRef],
    ) -> Result<Vec<Self>, ColumnError> {
        fields
            .iter()
            .map(|field| Self::try_from_arrow_field(chunk_entity_path, field.as_ref()))
            .collect()
    }

    pub fn archetype_name(&self) -> Option<ArchetypeName> {
        match self {
            Self::Component(component) => component.archetype,
            _ => None,
        }
    }
}

impl ColumnDescriptor {
    /// `chunk_entity_path`: if this column is part of a chunk batch,
    /// what is its entity path (so we can set [`ComponentColumnDescriptor::entity_path`])?
    pub fn try_from_arrow_field(
        chunk_entity_path: Option<&EntityPath>,
        field: &ArrowField,
    ) -> Result<Self, ColumnError> {
        match ColumnKind::try_from(field)? {
            ColumnKind::RowId => Err(ColumnError::UnsupportedColumnKind {
                kind: ColumnKind::RowId,
            }),

            ColumnKind::Index => Ok(Self::Time(IndexColumnDescriptor::try_from(field)?)),

            ColumnKind::Component => Ok(Self::Component(
                ComponentColumnDescriptor::from_arrow_field(chunk_entity_path, field),
            )),
        }
    }
}

#[test]
fn test_schema_over_ipc() {
    #![expect(clippy::disallowed_methods)] // Schema::new

    let original_columns = [
        ColumnDescriptor::Time(IndexColumnDescriptor::from_timeline(
            re_log_types::Timeline::log_time(),
            true,
        )),
        ColumnDescriptor::Component(ComponentColumnDescriptor {
            entity_path: re_log_types::EntityPath::from("/some/path"),
            archetype: Some("archetype".to_owned().into()),
            component: "component".to_owned().into(),
            component_type: Some(re_types_core::ComponentType::new("component_type")),
            store_datatype: arrow::datatypes::DataType::Int64,
            is_static: true,
            is_tombstone: false,
            is_semantically_empty: false,
        }),
    ];

    let original_schema = arrow::datatypes::Schema::new(ColumnDescriptor::to_arrow_fields(
        &original_columns,
        crate::BatchType::Dataframe,
    ));
    let ipc_bytes = crate::ipc_from_schema(&original_schema).unwrap();

    let recovered_schema = crate::raw_schema_from_ipc(&ipc_bytes).unwrap();
    assert_eq!(recovered_schema.as_ref(), &original_schema);

    let recovered_columns =
        ColumnDescriptor::from_arrow_fields(None, &recovered_schema.fields).unwrap();
    assert_eq!(recovered_columns, original_columns);
}
