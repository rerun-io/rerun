use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField, Fields as ArrowFields};

use re_log_types::EntityPath;

use crate::{
    ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor, RowIdColumnDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnDescriptorRef<'a> {
    RowId(&'a RowIdColumnDescriptor),
    Time(&'a IndexColumnDescriptor),
    Component(&'a ComponentColumnDescriptor),
}

impl ColumnDescriptorRef<'_> {
    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::RowId(_) | Self::Time(_) => None,
            Self::Component(descr) => Some(&descr.entity_path),
        }
    }

    #[inline]
    pub fn short_name(&self) -> String {
        match self {
            Self::RowId(descr) => descr.name().to_owned(),
            Self::Time(descr) => descr.timeline.name().to_string(),
            Self::Component(descr) => descr.component_name.short_name().to_owned(),
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
            Self::Time(descr) => descr.datatype.clone(),
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
}

impl<'a> From<&'a ColumnDescriptor> for ColumnDescriptorRef<'a> {
    fn from(desc: &'a ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::Time(desc) => Self::Time(desc),
            ColumnDescriptor::Component(desc) => Self::Component(desc),
        }
    }
}

impl<'a> From<&'a RowIdColumnDescriptor> for ColumnDescriptorRef<'a> {
    fn from(desc: &'a RowIdColumnDescriptor) -> Self {
        Self::RowId(desc)
    }
}

impl<'a> From<&'a IndexColumnDescriptor> for ColumnDescriptorRef<'a> {
    fn from(desc: &'a IndexColumnDescriptor) -> Self {
        Self::Time(desc)
    }
}

impl<'a> From<&'a ComponentColumnDescriptor> for ColumnDescriptorRef<'a> {
    fn from(desc: &'a ComponentColumnDescriptor) -> Self {
        Self::Component(desc)
    }
}
