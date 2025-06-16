use std::borrow::Cow;

use crate::{ArchetypeFieldName, ArchetypeName, ComponentType};

/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
///
/// Every component is uniquely identified by its [`ComponentDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComponentDescriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    pub archetype_name: Option<ArchetypeName>,

    /// Optional name of the field within `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `positions`.
    pub component: ArchetypeFieldName,

    /// Semantic type associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_type: Option<ComponentType>,
}

impl std::hash::Hash for ComponentDescriptor {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            archetype_name,
            component,
            component_type,
        } = self;

        let archetype_name = archetype_name.map_or(0, |v| v.hash());
        let component = component.hash();
        let component_type = component_type.map_or(0, |v| v.hash());

        // NOTE: This is a NoHash type, so we must respect the invariant that `write_XX` is only
        // called once, see <https://docs.rs/nohash-hasher/0.2.0/nohash_hasher/trait.IsEnabled.html>.
        state.write_u64(archetype_name ^ component ^ component_type);
    }
}

impl nohash_hasher::IsEnabled for ComponentDescriptor {}

impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name())
    }
}

impl From<ComponentDescriptor> for Cow<'static, ComponentDescriptor> {
    #[inline]
    fn from(descr: ComponentDescriptor) -> Self {
        Cow::Owned(descr)
    }
}

impl<'d> From<&'d ComponentDescriptor> for Cow<'d, ComponentDescriptor> {
    #[inline]
    fn from(descr: &'d ComponentDescriptor) -> Self {
        Cow::Borrowed(descr)
    }
}

impl ComponentDescriptor {
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        if let Some(component_type) = self.component_type {
            component_type.sanity_check();
        }
    }

    /// Short and usually unique, used in UI.
    pub fn display_name(&self) -> String {
        self.sanity_check();
        let Self {
            archetype_name,
            component,
            ..
        } = self;

        if let Some(archetype_name) = &archetype_name {
            format!("{}:{component}", archetype_name.short_name())
        } else {
            component.to_string()
        }
    }

    /// Is this an indicator component for an archetype?
    // TODO(#8129): Remove when we remove tagging.
    pub fn is_indicator_component(&self) -> bool {
        self.component.ends_with("Indicator")
    }

    /// If this is an indicator component, for which archetype?
    // TODO(#8129): Remove
    pub fn indicator_component_archetype_short_name(&self) -> Option<String> {
        ComponentType::new(&self.component)
            .short_name()
            .strip_suffix("Indicator")
            .map(|name| name.to_owned())
    }

    /// Returns the fully-qualified name, e.g. `rerun.archetypes.Points3D:positions#rerun.components.Position3D`.
    ///
    /// The result explicitly contains the [`ComponentType`], so in most cases [`ComponentDescriptor::display_name`] should be used instead.
    #[inline]
    pub fn full_name(&self) -> String {
        match self.component_type {
            Some(component_type) => {
                format!("{}#{component_type}", self.display_name())
            }
            None => self.display_name(),
        }
    }
}

impl re_byte_size::SizeBytes for ComponentDescriptor {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            archetype_name,
            component,
            component_type,
        } = self;
        archetype_name.heap_size_bytes()
            + component_type.heap_size_bytes()
            + component.heap_size_bytes()
    }
}

impl ComponentDescriptor {
    pub fn partial(component: impl Into<ArchetypeFieldName>) -> Self {
        Self {
            archetype_name: None,
            component: component.into(),
            component_type: None,
        }
    }

    /// Unconditionally sets [`Self::archetype_name`] to the given one.
    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        self.archetype_name = Some(archetype_name);
        self
    }

    /// Unconditionally sets [`Self::component`] to the given one.
    #[inline]
    pub fn with_component_type(mut self, component_type: ComponentType) -> Self {
        self.component_type = Some(component_type);
        self
    }

    /// Sets [`Self::archetype_name`] to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype_name(mut self, archetype_name: impl Fn() -> ArchetypeName) -> Self {
        if self.archetype_name.is_none() {
            self.archetype_name = Some(archetype_name());
        }
        self
    }

    /// Sets [`Self::component_type`] to the given one iff it's not already set.
    #[inline]
    pub fn or_with_component_type(
        mut self,
        component_type: impl FnOnce() -> ComponentType,
    ) -> Self {
        if self.component_type.is_none() {
            self.component_type = Some(component_type());
        }
        self
    }
}

// ---

// TODO(cmc): This is far from ideal and feels very hackish, but for now the priority is getting
// all things related to tags up and running so we can gather learnings.
// This is only used on the archetype deserialization path, which isn't ever used outside of tests anyway.

// TODO(cmc): we really shouldn't be duplicating these.

/// The key used to identify the [`crate::ArchetypeName`] in field-level metadata.
const FIELD_METADATA_KEY_ARCHETYPE_NAME: &str = "rerun.archetype_name";

/// The key used to identify the [`crate::ArchetypeFieldName`] in field-level metadata.
const FIELD_METADATA_KEY_COMPONENT_TYPE: &str = "rerun.component_type";

impl From<arrow::datatypes::Field> for ComponentDescriptor {
    #[inline]
    fn from(field: arrow::datatypes::Field) -> Self {
        let md = field.metadata();

        let descr = Self {
            archetype_name: md
                .get(FIELD_METADATA_KEY_ARCHETYPE_NAME)
                .cloned()
                .map(Into::into),
            component: field.name().to_string().into(),
            component_type: md
                .get(FIELD_METADATA_KEY_COMPONENT_TYPE)
                .cloned()
                .map(Into::into),
        };
        descr.sanity_check();
        descr
    }
}
