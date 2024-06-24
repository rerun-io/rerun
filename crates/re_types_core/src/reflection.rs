//! Run-time reflection for reading meta-data about components and archetypes.

use crate::ComponentName;

/// Runtime reflection about components and archetypes.
#[derive(Default)]
pub struct Reflection {
    pub components: ComponentReflectionMap,
    // TODO(emilk): archetypes (ArchetypeFieldInfo etc).
}

/// Runtime reflection about components.
pub type ComponentReflectionMap = nohash_hasher::IntMap<ComponentName, ComponentReflection>;

/// Information about a Rerun [`component`](crate::Component), generated by codegen.
pub struct ComponentReflection {
    /// Markdown docstring for the component.
    pub docstring_md: &'static str,

    /// Placeholder value, used whenever no fallback was provided explicitly.
    ///
    /// This is usually the default value of the component, serialized.
    ///
    /// This is useful as a base fallback value when displaying UI.
    pub placeholder: Option<Box<dyn arrow2::array::Array>>,
}
