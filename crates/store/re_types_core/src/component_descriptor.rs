use crate::{ArchetypeName, ComponentName, SizeBytes};

/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
// TODO(#6889): Fully sorbetize this thing? `ArchetypeName` and such don't make sense in that
// context. And whatever `archetype_field_name` ends up being, it needs interning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub archetype_field_name: Option<String>,

    /// Semantic name associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_name: ComponentName,
}

impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            archetype_name,
            archetype_field_name,
            component_name,
        } = self;

        match (archetype_name, component_name, archetype_field_name) {
            (None, component_name, None) => f.write_str(component_name),
            (Some(archetype_name), component_name, None) => {
                f.write_fmt(format_args!("{archetype_name}:{component_name}"))
            }
            (None, component_name, Some(archetype_field_name)) => {
                f.write_fmt(format_args!("{component_name}#{archetype_field_name}"))
            }
            (Some(archetype_name), component_name, Some(archetype_field_name)) => f.write_fmt(
                format_args!("{archetype_name}:{component_name}#{archetype_field_name}"),
            ),
        }
    }
}

impl SizeBytes for ComponentDescriptor {
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
    pub fn new(component_name: ComponentName) -> Self {
        Self {
            archetype_name: None,
            archetype_field_name: None,
            component_name,
        }
    }

    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        self.archetype_name = Some(archetype_name);
        self
    }

    #[inline]
    pub fn with_archetype_field_name(mut self, archetype_field_name: String) -> Self {
        self.archetype_field_name = Some(archetype_field_name);
        self
    }
}
