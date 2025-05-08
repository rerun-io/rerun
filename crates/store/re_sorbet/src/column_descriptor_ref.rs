use crate::{
    BatchType, ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnDescriptorRef<'a> {
    RowId(&'a RowIdColumnDescriptor),
    //TODO(ab): this should be renamed Index!
    Time(&'a IndexColumnDescriptor),
    Component(&'a ComponentColumnDescriptor),
}

impl ColumnDescriptorRef<'_> {
    /// Human-readable name for the column.
    #[inline]
    pub fn name(&self, batch_type: BatchType) -> String {
        match self {
            Self::RowId(descr) => descr.name().to_owned(),
            Self::Time(descr) => descr.column_name().to_owned(),
            Self::Component(descr) => descr.column_name(batch_type),
        }
    }

    /// Short human-readable name for the column.
    #[inline]
    pub fn short_name(&self) -> &str {
        match self {
            Self::RowId(descr) => descr.name(),
            Self::Time(descr) => descr.timeline_name().as_str(),
            Self::Component(descr) => descr.component_name.short_name(),
        }
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
