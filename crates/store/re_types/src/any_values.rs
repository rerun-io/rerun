//! Utilities to log arbitrary data to Rerun.

use nohash_hasher::IntMap;

use crate::{
    ArchetypeFieldName, ArchetypeName, Component, ComponentDescriptor, SerializedComponentBatch,
};
use re_types_core::{try_serialize_field, AsComponents, ComponentName, Loggable};

const DEFAULT_ARCHETYPE_NAME: &str = "rerun.AnyValues";
const DEFAULT_COMPONENT_NAME: &str = "rerun.component.AnyValue";

/// A helper for logging arbitrary data to Rerun.
pub struct AnyValues {
    archetype_name: ArchetypeName,
    batches: IntMap<ArchetypeFieldName, SerializedComponentBatch>,
}

impl Default for AnyValues {
    fn default() -> Self {
        Self::new(DEFAULT_ARCHETYPE_NAME)
    }
}

impl AnyValues {
    /// Assigns an (archetype) name to this set of any values.
    #[inline]
    pub fn new(archetype_name: impl Into<ArchetypeName>) -> Self {
        Self {
            archetype_name: archetype_name.into(),
            batches: Default::default(),
        }
    }

    /// Adds a field of arbitrary data to this archetype.
    ///
    /// The component name of this field will be set tp [`DEFAULT_COMPONENT_NAME`].
    ///
    /// In many cases, it might be more convenient to use [`Self::component`] to log an existing Rerun component instead.
    #[inline]
    pub fn field(
        self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        self.field_with_component_name(archetype_field_name, DEFAULT_COMPONENT_NAME, array)
    }

    /// Adds a field of arbitrary data to this archetype.
    ///
    /// In many cases, it might be more convenient to use [`Self::component`] to log an existing Rerun component instead.
    #[inline]
    pub fn field_with_component_name(
        mut self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        component_name: impl Into<ComponentName>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        let archetype_field_name = archetype_field_name.into();
        self.batches.insert(
            archetype_field_name,
            SerializedComponentBatch {
                array,
                descriptor: ComponentDescriptor {
                    archetype_name: Some(self.archetype_name),
                    archetype_field_name: Some(archetype_field_name),
                    component_name: component_name.into(),
                },
            },
        );
        self
    }

    /// Adds an existing Rerun [`Component`](crate::Component) to this archetype.
    #[inline]
    pub fn component<C: Component>(
        self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        component: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        self.loggable(archetype_field_name, C::name(), component)
    }

    /// Adds an existing Rerun [`Component`](crate::Component) to this archetype.
    ///
    /// This method can be used to override the component name.
    #[inline]
    pub fn loggable<L: Loggable>(
        mut self,
        archetype_field_name: impl Into<ArchetypeFieldName>,
        component_name: impl Into<ComponentName>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        let archetype_field_name = archetype_field_name.into();
        try_serialize_field(
            ComponentDescriptor {
                archetype_name: Some(self.archetype_name),
                archetype_field_name: Some(archetype_field_name),
                component_name: component_name.into(),
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
    use std::sync::Arc;

    use crate::components;

    use super::*;

    #[test]
    fn simple() {
        let _ = AnyValues::default()
            .component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
            .loggable::<components::Text>("homepage", "user.url", vec!["https://www.rerun.io"])
            .field(
                "description",
                Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
            )
            .field_with_component_name(
                "repository",
                "user.git",
                Arc::new(arrow::array::StringArray::from(vec![
                    "https://github.com/rerun-io/rerun",
                ])),
            );
    }
}
