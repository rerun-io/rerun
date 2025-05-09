use std::borrow::Cow;

use crate::{ArchetypeFieldName, ArchetypeName, ComponentName};

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
    pub archetype_field_name: Option<ArchetypeFieldName>,

    /// Semantic name associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_name: ComponentName,
}

impl std::hash::Hash for ComponentDescriptor {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            archetype_name,
            archetype_field_name,
            component_name,
        } = self;

        let archetype_name = archetype_name.map_or(0, |v| v.hash());
        let archetype_field_name = archetype_field_name.map_or(0, |v| v.hash());
        let component_name = component_name.hash();

        // NOTE: This is a NoHash type, so we must respect the invariant that `write_XX` is only
        // called one, see <https://docs.rs/nohash-hasher/0.2.0/nohash_hasher/trait.IsEnabled.html>.
        state.write_u64(archetype_name ^ archetype_field_name ^ component_name);
    }
}

impl nohash_hasher::IsEnabled for ComponentDescriptor {}

impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_any_string(false))
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
    fn to_any_string(&self, use_short_names: bool) -> String {
        let Self {
            archetype_name,
            archetype_field_name,
            component_name,
        } = self;

        let (archetype_name, component_name) = if use_short_names {
            (
                archetype_name.map(|s| s.short_name()),
                component_name.short_name(),
            )
        } else {
            (archetype_name.map(|s| s.as_str()), component_name.as_str())
        };

        match (archetype_name, component_name, archetype_field_name) {
            (None, component_name, None) => component_name.to_owned(),
            (Some(archetype_name), component_name, None) => {
                format!("{archetype_name}:{component_name}")
            }
            (None, component_name, Some(archetype_field_name)) => {
                format!("{component_name}#{archetype_field_name}")
            }
            (Some(archetype_name), component_name, Some(archetype_field_name)) => {
                format!("{archetype_name}:{component_name}#{archetype_field_name}")
            }
        }
    }

    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        self.component_name.sanity_check();
    }

    /// Returns the fully-qualified name, e.g. `rerun.archetypes.Points3D:rerun.components.Position3D#positions`.
    ///
    /// This is the default `Display` implementation for [`ComponentDescriptor`].
    #[inline]
    pub fn full_name(&self) -> String {
        self.sanity_check();
        self.to_any_string(false)
    }

    /// Returns the unqualified name, e.g. `Points3D:Position3D#positions`.
    #[inline]
    pub fn short_name(&self) -> String {
        self.sanity_check();
        self.to_any_string(true)
    }
}

impl re_byte_size::SizeBytes for ComponentDescriptor {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            archetype_name,
            archetype_field_name,
            component_name,
        } = self;
        archetype_name.heap_size_bytes()
            + component_name.heap_size_bytes()
            + archetype_field_name.heap_size_bytes()
    }
}

impl ComponentDescriptor {
    #[inline]
    pub fn new(component_name: impl Into<ComponentName>) -> Self {
        let component_name = component_name.into();
        component_name.sanity_check();
        Self {
            archetype_name: None,
            archetype_field_name: None,
            component_name,
        }
    }

    /// Unconditionally sets [`Self::archetype_name`] to the given one.
    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        self.archetype_name = Some(archetype_name);
        self
    }

    /// Unconditionally sets [`Self::archetype_field_name`] to the given one.
    #[inline]
    pub fn with_archetype_field_name(mut self, archetype_field_name: ArchetypeFieldName) -> Self {
        self.archetype_field_name = Some(archetype_field_name);
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

    /// Sets [`Self::archetype_field_name`] to the given one iff it's not already set.
    #[inline]
    pub fn or_with_archetype_field_name(
        mut self,
        archetype_field_name: impl FnOnce() -> ArchetypeFieldName,
    ) -> Self {
        if self.archetype_field_name.is_none() {
            self.archetype_field_name = Some(archetype_field_name());
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
const FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME: &str = "rerun.archetype_field_name";

impl From<arrow::datatypes::Field> for ComponentDescriptor {
    #[inline]
    fn from(field: arrow::datatypes::Field) -> Self {
        let md = field.metadata();

        let descr = Self {
            archetype_name: md
                .get(FIELD_METADATA_KEY_ARCHETYPE_NAME)
                .cloned()
                .map(Into::into),
            archetype_field_name: md
                .get(FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME)
                .cloned()
                .map(Into::into),
            component_name: field.name().to_string().into(),
        };
        descr.sanity_check();
        descr
    }
}
