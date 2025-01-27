// TODO(#6889): At some point all these descriptors needs to be interned and have handles or
// something. And of course they need to be codegen. But we'll get there once we're back to
// natively tagged components.

use arrow::datatypes::{
    DataType as ArrowDatatype, Field as ArrowField, FieldRef as ArrowFieldRef,
    Fields as ArrowFields,
};

use re_log_types::EntityPath;

use crate::{ComponentColumnDescriptor, MetadataExt as _, TimeColumnDescriptor};

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
// * [`TimeColumnDescriptor`]
// * [`ComponentColumnDescriptor`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColumnDescriptor {
    Time(TimeColumnDescriptor),
    Component(ComponentColumnDescriptor),
}

impl ColumnDescriptor {
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
    pub fn to_arrow_field(&self) -> ArrowField {
        match self {
            Self::Time(descr) => descr.to_arrow_field(),
            Self::Component(descr) => descr.to_arrow_field(),
        }
    }

    #[inline]
    pub fn to_arrow_fields(columns: &[Self]) -> ArrowFields {
        columns.iter().map(|c| c.to_arrow_field()).collect()
    }

    pub fn from_arrow_fields(fields: &[ArrowFieldRef]) -> Result<Vec<Self>, ColumnError> {
        fields
            .iter()
            .map(|field| Self::try_from(field.as_ref()))
            .collect()
    }
}

impl TryFrom<&ArrowField> for ColumnDescriptor {
    type Error = ColumnError;

    fn try_from(field: &ArrowField) -> Result<Self, Self::Error> {
        let kind = field.get_or_err("rerun.kind")?;
        match kind {
            "index" | "time" => Ok(Self::Time(TimeColumnDescriptor::try_from(field)?)),
            "data" => Ok(Self::Component(ComponentColumnDescriptor::try_from(field)?)),
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
        ColumnDescriptor::Time(TimeColumnDescriptor {
            timeline: re_log_types::Timeline::log_time(),
            datatype: arrow::datatypes::DataType::Timestamp(
                arrow::datatypes::TimeUnit::Nanosecond,
                None,
            ),
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

    let original_schema =
        arrow::datatypes::Schema::new(ColumnDescriptor::to_arrow_fields(&original_columns));
    let ipc_bytes = crate::ipc_from_schema(&original_schema).unwrap();

    let recovered_schema = crate::schema_from_ipc(&ipc_bytes).unwrap();
    assert_eq!(recovered_schema.as_ref(), &original_schema);

    let recovered_columns = ColumnDescriptor::from_arrow_fields(&recovered_schema.fields).unwrap();
    assert_eq!(recovered_columns, original_columns);
}
