//! Run-time reflection for reading meta-data about components and archetypes.

use crate::{ArchetypeName, ComponentName};

/// A trait for code-generated enums.
pub trait Enum:
    Sized + Copy + Clone + std::hash::Hash + PartialEq + Eq + std::fmt::Display + 'static
{
    /// All variants, in the order they appear in the enum.
    fn variants() -> &'static [Self];

    /// Markdown docstring for the given enum variant.
    fn docstring_md(self) -> &'static str;
}

/// Runtime reflection about components and archetypes.
#[derive(Clone, Debug, Default)]
pub struct Reflection {
    pub components: ComponentReflectionMap,
    pub archetypes: ArchetypeReflectionMap,
}

impl Reflection {
    /// Find an [`ArchetypeReflection`] based on its short name.
    ///
    /// Useful when the only information available is the short name, e.g. when inferring archetype
    /// names from an indicator component.
    //TODO( #6889): tagged component will contain a fully qualified archetype name, so this function
    // will be unnecessary.
    pub fn archetype_reflection_from_short_name(
        &self,
        short_name: &str,
    ) -> Option<&ArchetypeReflection> {
        // note: this mirrors `ArchetypeName::short_name`'s implementation
        self.archetypes
            .get(&ArchetypeName::from(short_name))
            .or_else(|| {
                self.archetypes.get(&ArchetypeName::from(format!(
                    "rerun.archetypes.{short_name}"
                )))
            })
            .or_else(|| {
                self.archetypes.get(&ArchetypeName::from(format!(
                    "rerun.blueprint.archetypes.{short_name}"
                )))
            })
            .or_else(|| {
                self.archetypes
                    .get(&ArchetypeName::from(format!("rerun.{short_name}")))
            })
    }
}

/// Runtime reflection about components.
pub type ComponentReflectionMap = nohash_hasher::IntMap<ComponentName, ComponentReflection>;

/// Information about a Rerun [`component`](crate::Component), generated by codegen.
#[derive(Clone, Debug)]
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

/// Runtime reflection about archetypes.
pub type ArchetypeReflectionMap = nohash_hasher::IntMap<ArchetypeName, ArchetypeReflection>;

/// Utility struct containing all archetype meta information.
#[derive(Clone, Debug)]
pub struct ArchetypeReflection {
    /// The name of the field in human case.
    pub display_name: &'static str,

    /// All the component fields of the archetype, in the order they appear in the archetype.
    pub fields: Vec<ArchetypeFieldReflection>,
}

impl ArchetypeReflection {
    /// Iterate over this archetype's required fields.
    #[inline]
    pub fn required_fields(&self) -> impl Iterator<Item = &ArchetypeFieldReflection> {
        self.fields.iter().filter(|field| field.is_required)
    }
}

/// Additional information about an archetype's field.
#[derive(Clone, Debug)]
pub struct ArchetypeFieldReflection {
    /// The type of the field (it's always a component).
    pub component_name: ComponentName,

    /// The name of the field in human case.
    pub display_name: &'static str,

    /// Markdown docstring for the field (not for the component type).
    pub docstring_md: &'static str,

    /// Is this a required component?
    pub is_required: bool,
}
