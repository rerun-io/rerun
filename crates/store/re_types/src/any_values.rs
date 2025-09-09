//! Utilities to log arbitrary data to Rerun.

use crate::{
    dynamic_archetype::DynamicArchetype, ArchetypeName, Component, SerializedComponentBatch,
};
use re_types_core::{AsComponents, ComponentType, Loggable};

/// A helper for logging arbitrary data to Rerun.
pub struct AnyValues {
    builder: DynamicArchetype,
}

impl Default for AnyValues {
    /// Creates an empty `AnyValues` container.
    fn default() -> Self {
        Self {
            builder: DynamicArchetype::new_without_archetype(),
        }
    }
}

impl AnyValues {
    //TODO(#10908): Prune this method in 0.26.0
    /// Assigns an (archetype) name to this set of any values.
    #[deprecated(
        since = "0.25.0",
        note = "Use of archetype leads to ambiguity whether provided field names should be prefixed to be unique. Refer to dynamic_archetype instead for an unambiguous usage."
    )]
    #[inline]
    pub fn new(archetype_name: impl Into<ArchetypeName>) -> Self {
        Self {
            builder: DynamicArchetype::new(archetype_name),
        }
    }

    /// Adds a component generated from arbitrary data to this collection.
    ///
    /// In many cases, it might be more convenient to use [`Self::with_component`] to log an existing Rerun component instead.
    #[inline]
    pub fn with_component_from_data(
        self,
        field: impl AsRef<str>,
        array: arrow::array::ArrayRef,
    ) -> Self {
        Self {
            builder: self.builder.with_component_from_data(field, array),
        }
    }

    //TODO(#10908): Prune this method in 0.26.0
    /// Adds a field of arbitrary data to this collection.
    ///
    /// In many cases, it might be more convenient to use [`Self::with_component`] to log an existing Rerun component instead.
    #[deprecated(since = "0.25.0", note = "Use with_component_from_data instead.")]
    #[inline]
    pub fn with_field(self, field: impl AsRef<str>, array: arrow::array::ArrayRef) -> Self {
        Self {
            builder: self.builder.with_component_from_data(field, array),
        }
    }

    /// Adds an existing Rerun [`Component`] to this collection.
    #[inline]
    pub fn with_component<C: Component>(
        self,
        field: impl AsRef<str>,
        loggable: impl IntoIterator<Item = impl Into<C>>,
    ) -> Self {
        Self {
            builder: self.builder.with_component(field, loggable),
        }
    }

    /// Adds an existing Rerun [`Component`] to this collection.
    ///
    /// This method can be used to override the component type.
    #[inline]
    pub fn with_component_override<L: Loggable>(
        self,
        field: impl AsRef<str>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        Self {
            builder: self
                .builder
                .with_component_override(field, component_type, loggable),
        }
    }

    //TODO(#10908): Prune this method in 0.26.0
    /// Adds an existing Rerun [`Component`] to this collection.
    ///
    /// This method can be used to override the component type.
    #[deprecated(since = "0.25.0", note = "Use with_component_override instead.")]
    #[inline]
    pub fn with_loggable<L: Loggable>(
        self,
        field: impl AsRef<str>,
        component_type: impl Into<ComponentType>,
        loggable: impl IntoIterator<Item = impl Into<L>>,
    ) -> Self {
        Self {
            builder: self
                .builder
                .with_component_override(field, component_type, loggable),
        }
    }
}

impl AsComponents for AnyValues {
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        self.builder.as_serialized_batches()
    }
}

#[cfg(test)]
mod test {

    use std::collections::BTreeSet;

    use crate::components;

    use crate::ComponentDescriptor;
    use re_types_core::datatypes::Utf8;

    use super::*;

    #[test]
    fn without_archetype() {
        let values = AnyValues::default()
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
                    .with_component_type(components::Scalar::name()),
                ComponentDescriptor::partial("homepage").with_component_type("user.url".into()),
                ComponentDescriptor::partial("description"),
            ]
            .into_iter()
            .collect()
        );
    }
}
