use std::borrow::Cow;

use crate::{ArchetypeName, ComponentIdentifier, ComponentType};

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

    /// Name of the component associated with this data.
    ///
    /// If `archetype_name` is `None`, this will be a simple field name.
    /// [`ArchetypeName::with_field`] is a convenient method to create a [`ComponentIdentifier`].
    ///
    /// Example: `Points3D:positions`. Warning: Never parse this string to retrieve an archetype!
    pub component: ComponentIdentifier,

    /// Optional, semantic type associated with this data.
    ///
    /// This is fully implied by the `component`, but included for semantic convenience.
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
            None => self.display_name().to_owned(),
        }
    }

    /// Returns the archetype field name.
    ///
    /// This is the result of stripping the [`ArchetypeName`] from [`Self::component`].
    #[inline]
    pub fn archetype_field_name(&self) -> &str {
        self.archetype_name
            .and_then(|archetype_name| {
                self.component
                    .strip_prefix(&format!("{}:", archetype_name.short_name()))
            })
            .unwrap_or_else(|| self.component.as_str())
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
    pub fn partial(component: impl Into<ComponentIdentifier>) -> Self {
        Self {
            archetype_name: None,
            component: component.into(),
            component_type: None,
        }
    }

    /// Unconditionally sets [`Self::archetype_name`] to the given one.
    ///
    /// This also changes the archetype part of [`Self::component`].
    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        {
            let field_name = self.archetype_field_name();
            self.component = archetype_name.with_field(field_name);
        }
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
    ///
    /// This also changes the archetype part of [`Self::component`].
    #[inline]
    pub fn or_with_archetype_name(mut self, archetype_name: impl Fn() -> ArchetypeName) -> Self {
        if self.archetype_name.is_none() {
            let archetype_name = archetype_name();
            self.component = archetype_name.with_field(self.component);
            self.archetype_name = Some(archetype_name);
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
const FIELD_METADATA_KEY_ARCHETYPE: &str = "rerun.archetype";

/// The key used to identify the [`crate::ComponentIdentifier`] in field-level metadata.
const FIELD_METADATA_KEY_COMPONENT: &str = "rerun.component";

/// The key used to identify the [`crate::ComponentType`] in field-level metadata.
const FIELD_METADATA_KEY_COMPONENT_TYPE: &str = "rerun.component_type";

impl From<arrow::datatypes::Field> for ComponentDescriptor {
    #[inline]
    fn from(field: arrow::datatypes::Field) -> Self {
        let md = field.metadata();

        let descr = Self {
            archetype_name: md
                .get(FIELD_METADATA_KEY_ARCHETYPE)
                .cloned()
                .map(Into::into),
            component: md.get(FIELD_METADATA_KEY_COMPONENT).cloned().unwrap_or_else(|| {
                re_log::debug!("Missing metadata field {FIELD_METADATA_KEY_COMPONENT}, resorting to field name: {}", field.name());
                field.name().to_string()
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

#[cfg(test)]
mod test {
    use crate::ArchetypeName;

    use super::ComponentDescriptor;

    #[test]
    fn component_descriptor_manipulation() {
        let archetype_name: ArchetypeName = "rerun.archetypes.MyExample".into();
        let descr = ComponentDescriptor {
            archetype_name: Some(archetype_name),
            component: archetype_name.with_field("test"),
            component_type: Some("user.Whatever".into()),
        };
        assert_eq!(descr.archetype_field_name(), "test");
        assert_eq!(descr.display_name(), "MyExample:test");

        let archetype_name: ArchetypeName = "rerun.archetypes.MyOtherExample".into();
        let descr = descr.with_archetype_name(archetype_name);
        assert_eq!(descr.archetype_field_name(), "test");
        assert_eq!(descr.display_name(), "MyOtherExample:test");
    }
}
