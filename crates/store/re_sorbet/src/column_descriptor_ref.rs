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
    #[inline]
    pub fn column_name(&self, batch_type: BatchType) -> String {
        match self {
            Self::RowId(descr) => descr.name().to_owned(),
            Self::Time(descr) => descr.column_name().to_owned(),
            Self::Component(descr) => descr.column_name(batch_type),
        }
    }

    pub fn to_owned(&self) -> ColumnDescriptor {
        match self {
            Self::RowId(descr) => ColumnDescriptor::RowId((*descr).clone()),
            Self::Time(descr) => ColumnDescriptor::Time((*descr).clone()),
            Self::Component(descr) => ColumnDescriptor::Component((*descr).clone()),
        }
    }

    /// Short human-readable name for the column.
    #[inline]
    pub fn display_name(&self) -> String {
        self.to_owned().display_name()
    }
}

impl<'a> From<&'a ColumnDescriptor> for ColumnDescriptorRef<'a> {
    fn from(desc: &'a ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::RowId(desc) => Self::RowId(desc),
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
