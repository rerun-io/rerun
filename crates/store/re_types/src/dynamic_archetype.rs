//! Utilities to generate an arbitrary archetype to log Rerun.

use nohash_hasher::IntMap;

use crate::{
    reflection::ComponentDescriptorExt as _, ArchetypeName, Component, ComponentDescriptor,
    ComponentIdentifier, SerializedComponentBatch,
};
use re_types_core::{try_serialize_field, AsComponents, ComponentType, Loggable};

/// A helper for logging a dynamically defined archetype to Rerun.
/// component names will be modified in a way similar to Rerun
/// internal types to avoid name collisions.
pub struct DynamicArchetype {
    archetype_name: Option<ArchetypeName>,
    batches: IntMap<ComponentIdentifier, SerializedComponentBatch>,
}

impl DynamicArchetype {
    /// Specifies an archetype name for this dynamically generated archetype.
    #[inline]
    pub fn new(archetype_name: impl Into<ArchetypeName>) -> Self {
        Self {
            archetype_name: Some(archetype_name.into()),
            batches: Default::default(),
        }
    }

    // Only crate public to reduce code duplication.
    pub(crate) fn new_without_archetype() -> Self {
        Self {
            archetype_name: None,
            batches: Default::default(),
        }
    }

    /// Adds a field of arbitrary data to this archetype.
    ///
    /// In many cases, it might be more convenient to use [`Self::with_component`] to log an existing Rerun component instead.
    #[inline]
    pub fn with_component_from_data(
        mut self,
        field: impl AsRef<str>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        let field = field.as_ref();
        let component = field.into();

        self.batches.insert(
            component,
            SerializedComponentBatch {
                array,
                descriptor: {
                    let mut desc = ComponentDescriptor::partial(component);
                    if let Some(archetype_name) = self.archetype_name {
                        desc = desc.with_builtin_archetype(archetype_name);
                    }
                    desc
                },
            },
        );
        self
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    #[inline]
    pub fn with_component<C: Component>(
        self,
        field: impl AsRef<str>,
        loggable: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        self.with_component_override(field, C::name(), loggable)
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    ///
    /// This method can be used to override the component type.
    #[inline]
    pub fn with_component_override<L: Loggable>(
        mut self,
        field: impl AsRef<str>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        let field = field.as_ref();
        let component = field.into();
        let mut desc =
            ComponentDescriptor::partial(component).with_component_type(component_type.into());
        if let Some(archetype_name) = self.archetype_name {
            desc = desc.with_builtin_archetype(archetype_name);
        }

        try_serialize_field(desc, loggable)
            .and_then(|serialized| self.batches.insert(component, serialized));
        self
    }
}

impl AsComponents for DynamicArchetype {
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        self.batches.values().cloned().collect()
    }
}

#[cfg(test)]
mod test {

    use std::collections::BTreeSet;

    use crate::components;
    use re_types_core::datatypes::Utf8;

    use super::*;

    #[test]
    fn with_archetype() {
        let values = DynamicArchetype::new("MyExample")
            .with_component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
            .with_component_override::<Utf8>("homepage", "user.url", vec!["https://www.rerun.io"])
            .with_component_from_data(
                "description",
                std::sync::Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
            );

        let actual = values
            .as_serialized_batches()
            .into_iter()
            .map(|batch| batch.descriptor)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual,
            [
                ComponentDescriptor::partial("confidence")
                    .with_builtin_archetype("MyExample")
                    .with_component_type(components::Scalar::name()),
                ComponentDescriptor::partial("homepage")
                    .with_component_type("user.url".into())
                    .with_builtin_archetype("MyExample"),
                ComponentDescriptor::partial("description").with_builtin_archetype("MyExample"),
            ]
            .into_iter()
            .collect()
        );
    }
}
