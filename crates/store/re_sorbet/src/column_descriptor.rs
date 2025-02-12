// TODO(#6889): At some point all these descriptors needs to be interned and have handles or
// something. And of course they need to be codegen. But we'll get there once we're back to
// natively tagged components.

use arrow::datatypes::{
    DataType as ArrowDatatype, Field as ArrowField, FieldRef as ArrowFieldRef,
    Fields as ArrowFields,
};

use re_log_types::EntityPath;

use crate::{ComponentColumnDescriptor, IndexColumnDescriptor, MetadataExt as _};

#[derive(thiserror::Error, Debug)]
pub enum ColumnError {
    #[error(transparent)]
    MissingFieldMetadata(#[from] crate::MissingFieldMetadata),

    #[error("Unsupported column rerun.kind: {kind:?}. Expected one of: index, data")]
    UnsupportedColumnKind { kind: String },

    #[error(transparent)]
    UnsupportedTimeType(#[from] crate::UnsupportedTimeType),
}

// Describes any kind of column.
//
// See:
// * [`IndexColumnDescriptor`]
// * [`ComponentColumnDescriptor`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColumnDescriptor {
    Time(IndexColumnDescriptor),
    Component(ComponentColumnDescriptor),
}

impl ColumnDescriptor {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        match self {
            Self::Time(_) => {}
            Self::Component(descr) => descr.sanity_check(),
        }
    }

    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::Time(_) => None,
            Self::Component(descr) => Some(&descr.entity_path),
        }
    }

    #[inline]
    pub fn short_name(&self) -> String {
        match self {
            Self::Time(descr) => descr.timeline.name().to_string(),
            Self::Component(descr) => descr.component_name.short_name().to_owned(),
        }
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        match self {
            Self::Time(_) => false,
            Self::Component(descr) => descr.is_static,
        }
    }

    #[inline]
    pub fn arrow_datatype(&self) -> ArrowDatatype {
        match self {
            Self::Time(descr) => descr.datatype.clone(),
            Self::Component(descr) => descr.returned_datatype(),
        }
    }

    #[inline]
    pub fn to_arrow_field(&self, batch_type: crate::BatchType) -> ArrowField {
        match self {
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
}

impl ColumnDescriptor {
    /// `chunk_entity_path`: if this column is part of a chunk batch,
    /// what is its entity path (so we can set [`ComponentColumnDescriptor::entity_path`])?
    pub fn try_from_arrow_field(
        chunk_entity_path: Option<&EntityPath>,
        field: &ArrowField,
    ) -> Result<Self, ColumnError> {
        let kind = field.get_or_err("rerun.kind")?;
        match kind {
            "index" | "time" => Ok(Self::Time(IndexColumnDescriptor::try_from(field)?)),

            "data" => Ok(Self::Component(
                ComponentColumnDescriptor::from_arrow_field(chunk_entity_path, field),
            )),

            _ => Err(ColumnError::UnsupportedColumnKind {
                kind: kind.to_owned(),
            }),
        }
    }
}

#[test]
fn test_schema_over_ipc() {
    #![expect(clippy::disallowed_methods)] // Schema::new

    let original_columns = [
        ColumnDescriptor::Time(IndexColumnDescriptor {
            timeline: re_log_types::Timeline::log_time(),
            datatype: arrow::datatypes::DataType::Timestamp(
                arrow::datatypes::TimeUnit::Nanosecond,
                None,
            ),
            is_sorted: true,
        }),
        ColumnDescriptor::Component(ComponentColumnDescriptor {
            entity_path: re_log_types::EntityPath::from("/some/path"),
            archetype_name: Some("archetype".to_owned().into()),
            archetype_field_name: Some("field".to_owned().into()),
            component_name: re_types_core::ComponentName::new("component"),
            store_datatype: arrow::datatypes::DataType::Int64,
            is_static: true,
            is_tombstone: false,
            is_semantically_empty: false,
            is_indicator: true,
        }),
    ];

    let original_schema = arrow::datatypes::Schema::new(ColumnDescriptor::to_arrow_fields(
        &original_columns,
        crate::BatchType::Dataframe,
    ));
    let ipc_bytes = crate::ipc_from_schema(&original_schema).unwrap();

    let recovered_schema = crate::schema_from_ipc(&ipc_bytes).unwrap();
    assert_eq!(recovered_schema.as_ref(), &original_schema);

    let recovered_columns =
        ColumnDescriptor::from_arrow_fields(None, &recovered_schema.fields).unwrap();
    assert_eq!(recovered_columns, original_columns);
}
