use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};

use re_log_types::{ComponentPath, EntityPath};
use re_types_core::{ArchetypeFieldName, ArchetypeName, ComponentDescriptor, ComponentName};

use crate::{ArrowFieldMetadata, MetadataExt as _, MissingFieldMetadata};

/// Describes a data/component column, such as `Position3D`, in a dataframe.
///
/// This is an [`ArrowField`] that contains specific meta-data.
//
// TODO(#6889): Fully sorbetize this thing? `ArchetypeName` and such don't make sense in that
// context. And whatever `archetype_field_name` ends up being, it needs interning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentColumnDescriptor {
    /// The Arrow datatype of the stored column.
    ///
    /// This is the log-time datatype corresponding to how this data is encoded
    /// in a chunk. Currently this will always be an [`arrow::array::ListArray`], but as
    /// we introduce mono-type optimization, this might be a native type instead.
    pub store_datatype: ArrowDatatype,

    /// The path of the entity.
    ///
    /// If this column is part of a chunk batch,
    /// this is the same for all columns in the batch,
    /// and will also be set in the schema for the whole chunk.
    pub entity_path: EntityPath,

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

    /// Whether this column represents static data.
    pub is_static: bool,

    /// Whether this column represents an indicator component.
    pub is_indicator: bool,

    /// Whether this column represents a [`Clear`]-related components.
    ///
    /// [`Clear`]: re_types_core::archetypes::Clear
    pub is_tombstone: bool,

    /// Whether this column contains either no data or only contains null and/or empty values (`[]`).
    pub is_semantically_empty: bool,
}

impl PartialOrd for ComponentColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ComponentColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            store_datatype: _,
            is_static: _,
            is_indicator: _,
            is_tombstone: _,
            is_semantically_empty: _,
        } = self;

        entity_path
            .cmp(&other.entity_path)
            .then_with(|| archetype_name.cmp(&other.archetype_name))
            .then_with(|| archetype_field_name.cmp(&other.archetype_field_name))
            .then_with(|| component_name.cmp(&other.component_name))
    }
}

impl std::fmt::Display for ComponentColumnDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            store_datatype: _,
            is_static,
            is_indicator: _,
            is_tombstone: _,
            is_semantically_empty: _,
        } = self;

        let descriptor = ComponentDescriptor {
            archetype_name: *archetype_name,
            archetype_field_name: *archetype_field_name,
            component_name: *component_name,
        };

        let s = format!("{entity_path}@{}", descriptor.short_name());

        if *is_static {
            f.write_fmt(format_args!("|{s}|"))
        } else {
            f.write_str(&s)
        }
    }
}

impl From<ComponentColumnDescriptor> for re_types_core::ComponentDescriptor {
    #[inline]
    fn from(descr: ComponentColumnDescriptor) -> Self {
        Self {
            archetype_name: descr.archetype_name,
            archetype_field_name: descr.archetype_field_name,
            component_name: descr.component_name,
        }
    }
}

impl From<&ComponentColumnDescriptor> for re_types_core::ComponentDescriptor {
    #[inline]
    fn from(descr: &ComponentColumnDescriptor) -> Self {
        Self {
            archetype_name: descr.archetype_name,
            archetype_field_name: descr.archetype_field_name,
            component_name: descr.component_name,
        }
    }
}

impl ComponentColumnDescriptor {
    pub fn component_path(&self) -> ComponentPath {
        ComponentPath {
            entity_path: self.entity_path.clone(),
            component_name: self.component_name,
        }
    }

    #[inline]
    pub fn matches(&self, entity_path: &EntityPath, component_name: &str) -> bool {
        &self.entity_path == entity_path && self.component_name.matches(component_name)
    }

    fn metadata(&self) -> ArrowFieldMetadata {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            store_datatype: _,
            is_static,
            is_indicator,
            is_tombstone,
            is_semantically_empty,
        } = self;

        // TODO(#6889): This needs some proper sorbetization -- I just threw these names randomly.
        [
            Some(("rerun.kind".to_owned(), "data".to_owned())),
            Some(("rerun.entity_path".to_owned(), entity_path.to_string())),
            archetype_name.map(|name| ("rerun.archetype".to_owned(), name.short_name().to_owned())),
            archetype_field_name
                .as_ref()
                .map(|name| ("rerun.archetype_field".to_owned(), name.to_string())),
            Some((
                "rerun.component".to_owned(),
                component_name.short_name().to_owned(),
            )),
            (*is_static).then_some(("rerun.is_static".to_owned(), "true".to_owned())),
            (*is_indicator).then_some(("rerun.is_indicator".to_owned(), "true".to_owned())),
            (*is_tombstone).then_some(("rerun.is_tombstone".to_owned(), "true".to_owned())),
            (*is_semantically_empty)
                .then_some(("rerun.is_semantically_empty".to_owned(), "true".to_owned())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    pub fn returned_datatype(&self) -> ArrowDatatype {
        self.store_datatype.clone()
    }

    pub fn column_name(&self) -> String {
        let entity_path = &self.entity_path;
        let descriptor = ComponentDescriptor {
            archetype_name: self.archetype_name,
            archetype_field_name: self.archetype_field_name,
            component_name: self.component_name,
        };

        format!("{}:{}", entity_path, descriptor.component_name.short_name())

        // NOTE: Uncomment this to expose fully-qualified names in the Dataframe APIs!
        // I'm not doing that right now, to avoid breaking changes (and we need to talk about
        // what the syntax for these fully-qualified paths need to look like first).
        // format!("{entity_path}@{}", descriptor.short_name())
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let nullable = true;
        ArrowField::new(self.column_name(), self.returned_datatype(), nullable)
            .with_metadata(self.metadata())
    }
}

impl ComponentColumnDescriptor {
    /// `chunk_entity_path`: if this column is part of a chunk batch,
    /// what is its entity path (so we can set [`ComponentColumnDescriptor::entity_path`])?
    pub fn try_from_arrow_field(
        chunk_entity_path: Option<&EntityPath>,
        field: &ArrowField,
    ) -> Result<Self, MissingFieldMetadata> {
        let entity_path = if let Some(chunk_entity_path) = chunk_entity_path {
            chunk_entity_path.clone()
        } else {
            EntityPath::parse_forgiving(field.get_or_err("rerun.entity_path")?)
        };

        let component_name = if let Some(component_name) = field.get_opt("rerun.component") {
            ComponentName::from(component_name)
        } else {
            ComponentName::new(field.name()) // Legacy fallback
        };

        Ok(Self {
            store_datatype: field.data_type().clone(),
            entity_path,
            archetype_name: field.get_opt("rerun.archetype").map(|x| x.into()),
            archetype_field_name: field.get_opt("rerun.archetype_field").map(|x| x.into()),
            component_name,
            is_static: field.get_bool("rerun.is_static"),
            is_indicator: field.get_bool("rerun.is_indicator"),
            is_tombstone: field.get_bool("rerun.is_tombstone"),
            is_semantically_empty: field.get_bool("rerun.is_semantically_empty"),
        })
    }
}
