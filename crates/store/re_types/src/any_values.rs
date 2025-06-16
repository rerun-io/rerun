//! Utilities to log arbitrary data to Rerun.

use nohash_hasher::IntMap;

use crate::{
    ArchetypeFieldName, ArchetypeName, Component, ComponentDescriptor, SerializedComponentBatch,
};
use re_types_core::{try_serialize_field, AsComponents, ComponentType, Loggable};

/// A helper for logging arbitrary data to Rerun.
#[derive(Default)]
pub struct AnyValues {
    archetype_name: Option<ArchetypeName>,
    batches: IntMap<ArchetypeFieldName, SerializedComponentBatch>,
}

impl AnyValues {
    /// Assigns an (archetype) name to this set of any values.
    #[inline]
    pub fn new(archetype_name: impl Into<ArchetypeName>) -> Self {
        Self {
            archetype_name: Some(archetype_name.into()),
            batches: Default::default(),
        }
    }

    /// Adds a field of arbitrary data to this archetype.
    ///
    /// In many cases, it might be more convenient to use [`Self::with_component`] to log an existing Rerun component instead.
    #[inline]
    pub fn with_field(
        mut self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        let archetype_field_name = archetype_field_name.into();
        self.batches.insert(
            archetype_field_name,
            SerializedComponentBatch {
                array,
                descriptor: ComponentDescriptor {
                    archetype_name: self.archetype_name,
                    component_type: None,
                    archetype_field_name,
                },
            },
        );
        self
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    #[inline]
    pub fn with_component<C: Component>(
        self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        component: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        self.with_loggable(archetype_field_name, C::name(), component)
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    ///
    /// This method can be used to override the component name.
    #[inline]
    pub fn with_loggable<L: Loggable>(
        mut self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        let archetype_field_name = archetype_field_name.into();
        try_serialize_field(
            ComponentDescriptor {
                archetype_name: self.archetype_name,
                archetype_field_name,
                component_type: Some(component_type.into()),
            },
            loggable,
        )
        .and_then(|serialized| self.batches.insert(archetype_field_name, serialized));
        self
    }
}

impl AsComponents for AnyValues {
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        self.batches.values().cloned().collect()
    }
}

#[cfg(test)]
mod test {

    use crate::components;

    use super::*;

    #[test]
    fn simple() {
        let _ = AnyValues::default()
            .with_component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
            .with_loggable::<components::Text>("homepage", "user.url", vec!["https://www.rerun.io"])
            .with_field(
                "description",
                std::sync::Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
            );
    }
}
