//! Utilities to generate an arbitrary archetype to log Rerun.

use nohash_hasher::IntMap;

use crate::reflection::ComponentDescriptorExt as _;
use crate::{
    ArchetypeName, AsComponents, Component, ComponentDescriptor, ComponentIdentifier,
    ComponentType, Loggable, SerializedComponentBatch, try_serialize_field,
};

/// A helper for logging a dynamically defined archetype to Rerun.
///
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

    // Only used internally; kept public so helper crates can avoid code duplication.
    #[doc(hidden)]
    pub fn new_without_archetype() -> Self {
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
