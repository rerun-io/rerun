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
    /// Human-readable name for the column.
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            Self::RowId(descr) => descr.name(),
            Self::Time(descr) => descr.column_name(),
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
