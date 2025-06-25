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
    pub fn with_field(mut self, field: impl AsRef<str>, array: arrow::array::ArrayRef) -> Self {
        let field = field.as_ref();
        let component = self
            .archetype_name
            .map(|archetype_name| archetype_name.with_field(field))
            .unwrap_or_else(|| field.into());

        self.batches.insert(
            component,
            SerializedComponentBatch {
                array,
                descriptor: ComponentDescriptor {
                    archetype: self.archetype_name,
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
        field: impl AsRef<str>,
        loggable: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        self.with_loggable(field, C::name(), loggable)
    }

    /// Adds an existing Rerun [`Component`] to this archetype.
    ///
    /// This method can be used to override the component type.
    #[inline]
    pub fn with_loggable<L: Loggable>(
        mut self,
        field: impl AsRef<str>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        let field = field.as_ref();
        let component = self
            .archetype_name
            .map(|archetype_name| archetype_name.with_field(field))
            .unwrap_or_else(|| field.into());

        try_serialize_field(
            ComponentDescriptor {
                archetype: self.archetype_name,
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

    use std::collections::BTreeSet;

    use crate::components;

    use super::*;

    #[test]
    fn without_archetype() {
        let values = AnyValues::default()
            .with_component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
            .with_loggable::<components::Text>("homepage", "user.url", vec!["https://www.rerun.io"])
            .with_field(
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
                    .with_component_type(components::Scalar::name()),
                ComponentDescriptor::partial("homepage").with_component_type("user.url".into()),
                ComponentDescriptor::partial("description"),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn with_archetype() {
        let values = AnyValues::new("MyExample")
            .with_component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
            .with_loggable::<components::Text>("homepage", "user.url", vec!["https://www.rerun.io"])
            .with_field(
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
                    .with_archetype("MyExample".into())
                    .with_component_type(components::Scalar::name()),
                ComponentDescriptor::partial("homepage")
                    .with_component_type("user.url".into())
                    .with_archetype("MyExample".into()),
                ComponentDescriptor::partial("description").with_archetype("MyExample".into()),
            ]
            .into_iter()
            .collect()
        );
    }
}
