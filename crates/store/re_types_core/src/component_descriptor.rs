use crate::{ArchetypeName, ComponentName, SizeBytes};

/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentDescriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    pub archetype_name: Option<ArchetypeName>,

    /// Semantic name associated with this data.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_name: ComponentName,

    /// Optional label to further qualify the data.
    ///
    /// Example: "postions".
    //
    // TODO: Maybe it's a dedicated type or an `InternedString` or w/e, doesn't matter.
    pub tag: Option<String>,
}

impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            archetype_name,
            component_name,
            tag,
        } = self;

        match (archetype_name, component_name, tag) {
            (None, component_name, None) => f.write_str(component_name),
            (Some(archetype_name), component_name, None) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}"))
            }
            (None, component_name, Some(tag)) => {
                f.write_fmt(format_args!("{component_name}#{tag}"))
            }
            (Some(archetype_name), component_name, Some(tag)) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}#{tag}"))
            }
        }
    }
}

impl SizeBytes for ComponentDescriptor {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            archetype_name,
            component_name,
            tag,
        } = self;
        archetype_name.heap_size_bytes() + component_name.heap_size_bytes() + tag.heap_size_bytes()
    }
}

impl ComponentDescriptor {
    #[inline]
    pub fn new(component_name: ComponentName) -> Self {
        Self {
            archetype_name: None,
            component_name,
            tag: None,
        }
    }

    #[inline]
    pub fn with_archetype_name(mut self, archetype_name: ArchetypeName) -> Self {
        self.archetype_name = Some(archetype_name);
        self
    }

    #[inline]
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tag = Some(tag);
        self
    }
}

// impl ToOwned for ComponentDescriptor {
//     type Owned = Self;
//
//     fn to_owned(&self) -> Self::Owned {
//         todo!()
//     }
// }

// TODO: as_metadata?

// TODO: logical steps:
// - generate descriptors
// - get rid of Archetype's component_name methods?
// - get descriptors all the way to the chunks (and thus store)
//   - log_archetype
//   - log_component
// - add pattern matched queries
// - replace indicators with pattern matched queries
// - modify mesh3d
