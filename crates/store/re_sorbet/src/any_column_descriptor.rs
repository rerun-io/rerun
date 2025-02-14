use crate::{
    ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor, RowIdColumnDescriptor,
};
use re_log_types::EntityPath;

//TODO: turn that into a ColumnDescriptorRef???

//TODO(#9034): we should use `crate::ColumnDescriptor`, but it currently doesn't support `RowId`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnyColumnDescriptor {
    RowId(RowIdColumnDescriptor),
    Time(IndexColumnDescriptor),
    Component(ComponentColumnDescriptor),
}

impl AnyColumnDescriptor {
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
}

impl From<ColumnDescriptor> for AnyColumnDescriptor {
    fn from(desc: ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::Time(desc) => Self::Time(desc),
            ColumnDescriptor::Component(desc) => Self::Component(desc),
        }
    }
}

impl From<RowIdColumnDescriptor> for AnyColumnDescriptor {
    fn from(desc: RowIdColumnDescriptor) -> Self {
        Self::RowId(desc)
    }
}

impl From<IndexColumnDescriptor> for AnyColumnDescriptor {
    fn from(desc: IndexColumnDescriptor) -> Self {
        Self::Time(desc)
    }
}

impl From<ComponentColumnDescriptor> for AnyColumnDescriptor {
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self::Component(desc)
    }
}
