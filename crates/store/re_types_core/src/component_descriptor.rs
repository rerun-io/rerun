use std::borrow::Cow;

use crate::{ArchetypeName, ComponentIdentifier, ComponentType};

/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
///
/// Every component at a given entity path is uniquely identified by the
/// `component` field of the descriptor. The `archetype` and `component_type`
/// fields provide additional information about the semantics of the data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComponentDescriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    pub archetype: Option<ArchetypeName>,

    /// Uniquely identifies of the component associated with this data.
    ///
    /// Example: `Points3D:positions`.
    pub component: ComponentIdentifier,

    /// Optional type information for this component.
    ///
    /// Can be used to inform applications on how to interpret the data.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_type: Option<ComponentType>,
}

impl std::hash::Hash for ComponentDescriptor {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            archetype: archetype_name,
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
        f.write_str(self.display_name())
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
    pub fn display_name(&self) -> &str {
        self.sanity_check();
        self.component.as_str()
    }
}

impl re_byte_size::SizeBytes for ComponentDescriptor {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            archetype: archetype_name,
            component,
            component_type,
        } = self;
        archetype_name.heap_size_bytes()
            + component_type.heap_size_bytes()
            + component.heap_size_bytes()
    }
}

impl ComponentDescriptor {
    /// Creates a new component descriptor that only has the `component` set.
    ///
    /// Both `archetype` and `component_type` will be missing.
    pub fn partial(component: impl Into<ComponentIdentifier>) -> Self {
        Self {
            archetype: None,
            component: component.into(),
            component_type: None,
        }
    }

    /// Unconditionally sets [`Self::archetype`] to the given one.
    #[inline]
    pub fn with_archetype(mut self, archetype_name: ArchetypeName) -> Self {
        self.archetype = Some(archetype_name);
        self
    }

    /// Unconditionally sets [`Self::component`] to the given one.
    #[inline]
    pub fn with_component_type(mut self, component_type: ComponentType) -> Self {
        self.component_type = Some(component_type);
        self
    }

    /// Sets [`Self::archetype`] to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype(mut self, archetype_name: impl Fn() -> ArchetypeName) -> Self {
        if self.archetype.is_none() {
            self.archetype = Some(archetype_name());
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
pub const FIELD_METADATA_KEY_ARCHETYPE: &str = "rerun:archetype";

/// The key used to identify the [`crate::ComponentIdentifier`] in field-level metadata.
pub const FIELD_METADATA_KEY_COMPONENT: &str = "rerun:component";

/// The key used to identify the [`crate::ComponentType`] in field-level metadata.
pub const FIELD_METADATA_KEY_COMPONENT_TYPE: &str = "rerun:component_type";

impl From<arrow::datatypes::Field> for ComponentDescriptor {
    #[inline]
    fn from(field: arrow::datatypes::Field) -> Self {
        let md = field.metadata();

        let descr = Self {
            archetype: md
                .get(FIELD_METADATA_KEY_ARCHETYPE)
                .cloned()
                .map(Into::into),
            component: md.get(FIELD_METADATA_KEY_COMPONENT).cloned().unwrap_or_else(|| {
                re_log::debug!("Missing metadata field {FIELD_METADATA_KEY_COMPONENT}, resorting to field name: {}", field.name());
                field.name().clone()
            }).into(),
            component_type: md
                .get(FIELD_METADATA_KEY_COMPONENT_TYPE)
                .cloned()
                .map(Into::into),
        };
        descr.sanity_check();
        descr
    }
}
