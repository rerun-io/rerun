//! Utilities to log arbitrary data to Rerun.

use nohash_hasher::IntMap;

use crate::{
    ArchetypeName, Component, ComponentDescriptor, ComponentIdentifier, SerializedComponentBatch,
};
use re_types_core::{try_serialize_field, AsComponents, ComponentType, Loggable};

/// A helper for logging arbitrary data to Rerun.
#[derive(Default)]
pub struct AnyValues {
    archetype_name: Option<ArchetypeName>,
    batches: IntMap<ComponentIdentifier, SerializedComponentBatch>,
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
        component: impl Into<ComponentIdentifier>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        let component = component.into();
        self.batches.insert(
            component,
            SerializedComponentBatch {
                array,
                descriptor: ComponentDescriptor {
                    archetype_name: self.archetype_name,
                    component_type: None,
                    component,
                },
            },
        );
        self
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    #[inline]
    pub fn with_component<C: Component>(
        self,
        component: impl Into<ComponentIdentifier>,
        loggable: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        self.with_loggable(component, C::name(), loggable)
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    ///
    /// This method can be used to override the component type.
    #[inline]
    pub fn with_loggable<L: Loggable>(
        mut self,
        component: impl Into<ComponentIdentifier>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        let component = component.into();
        try_serialize_field(
            ComponentDescriptor {
                archetype_name: self.archetype_name,
                component,
                component_type: Some(component_type.into()),
            },
            loggable,
        )
        .and_then(|serialized| self.batches.insert(component, serialized));
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
