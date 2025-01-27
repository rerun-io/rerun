// TODO(#6889): At some point all these descriptors needs to be interned and have handles or
// something. And of course they need to be codegen. But we'll get there once we're back to
// natively tagged components.

use arrow::datatypes::{
    DataType as ArrowDatatype, Field as ArrowField, FieldRef as ArrowFieldRef,
    Schema as ArrowSchema,
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

    pub fn from_arrow_field(field: &ArrowField) -> Result<Self, ColumnError> {
        let kind = field.get_or_err("rerun.kind")?;
        match kind {
            "index" | "time" => Ok(Self::Time(TimeColumnDescriptor::try_from(field)?)),
            "data" => Ok(Self::Component(ComponentColumnDescriptor::try_from(field)?)),
            _ => Err(ColumnError::UnsupportedColumnKind {
                kind: kind.to_owned(),
            }),
        }
    }

    pub fn from_arrow_fields(fields: &[ArrowFieldRef]) -> Result<Vec<Self>, ColumnError> {
        fields
            .iter()
            .map(|field| Self::from_arrow_field(field))
            .collect()
    }

    pub fn from_arrow_schema(schema: &ArrowSchema) -> Result<Vec<Self>, ColumnError> {
        Self::from_arrow_fields(&schema.fields)
    }
}
