use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};

use re_log_types::{ComponentPath, EntityPath};
use re_types_core::{ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType};

use crate::{ArrowFieldMetadata, BatchType, ColumnKind, ComponentColumnSelector, MetadataExt as _};

/// This is an [`ArrowField`] that contains specific meta-data.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentColumnDescriptor {
    /// The Arrow datatype of the stored column.
    ///
    /// This is the log-time datatype corresponding to how this data is encoded
    /// in a chunk. Currently this will always be an [`arrow::array::ListArray`], but as
    /// we introduce mono-type optimization, this might be a native type instead.
    pub store_datatype: ArrowDatatype,

    /// Optional semantic name associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_type: Option<ComponentType>,

    /// The path of the entity.
    ///
    /// If this column is part of a chunk batch,
    /// this is the same for all columns in the batch,
    /// and will also be set in the schema for the whole chunk.
    ///
    /// If this is missing from the metadata, it will be set to `/`.
    pub entity_path: EntityPath, // TODO(#8744): make optional for general sorbet batches

    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    pub archetype_name: Option<ArchetypeName>,

    /// Identifier of the field associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `positions`.
    pub component: ComponentIdentifier,

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
            component,
            component_type,
            store_datatype: _,
            is_static: _,
            is_indicator: _,
            is_tombstone: _,
            is_semantically_empty: _,
        } = self;

        entity_path
            .cmp(&other.entity_path)
            .then_with(|| archetype_name.cmp(&other.archetype_name))
            .then_with(|| component.cmp(&other.component))
            .then_with(|| component_type.cmp(&other.component_type))
    }
}

impl std::fmt::Display for ComponentColumnDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let descriptor = self.component_descriptor();

        let Self {
            entity_path,
            archetype_name: _,
            component: _,
            component_type: _,
            store_datatype: _,
            is_static,
            is_indicator: _,
            is_tombstone: _,
            is_semantically_empty: _,
        } = self;

        let s = format!("{entity_path}@{}", descriptor.display_name());

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
        descr.sanity_check();
        Self {
            archetype_name: descr.archetype_name,
            component: descr.component,
            component_type: descr.component_type,
        }
    }
}

impl From<&ComponentColumnDescriptor> for re_types_core::ComponentDescriptor {
    #[inline]
    fn from(descr: &ComponentColumnDescriptor) -> Self {
        descr.sanity_check();
        Self {
            archetype_name: descr.archetype_name,
            component: descr.component,
            component_type: descr.component_type,
        }
    }
}

impl ComponentColumnDescriptor {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        if let Some(c) = self.component_type {
            c.sanity_check();
        }
        if let Some(archetype_name) = &self.archetype_name {
            archetype_name.sanity_check();
        }
    }

    pub fn component_path(&self) -> ComponentPath {
        ComponentPath {
            entity_path: self.entity_path.clone(),
            component_descriptor: self.component_descriptor(),
        }
    }

    pub fn component_descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: self.archetype_name,
            component: self.component,
            component_type: self.component_type,
        }
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.is_static
    }

    #[inline]
    /// Checks if the current column descriptor matches a given [`ComponentColumnSelector`].
    ///
    /// The matching accepts selectors with full _and_ short versions of their [`ArchetypeName`].
    pub fn matches_weak(&self, selector: &ComponentColumnSelector) -> bool {
        let matches_archetype_name_weak = || match (self.archetype_name, selector.archetype_name) {
            (Some(this), Some(sel)) if this != sel => this.short_name() == sel,
            (lhs, rhs) => lhs == rhs,
        };

        // We convert down to `str` for comparison to avoid interning new fields.
        self.entity_path == selector.entity_path
            && matches_archetype_name_weak()
            && self.component.as_str() == selector.component
    }

    fn metadata(&self, batch_type: BatchType) -> ArrowFieldMetadata {
        self.sanity_check();

        let Self {
            entity_path,
            archetype_name,
            component,
            component_type,
            store_datatype: _,
            is_static,
            is_indicator,
            is_tombstone,
            is_semantically_empty,
        } = self;

        // TODO(#6889): This needs some proper sorbetization -- I just threw these names randomly.
        // We use the long names for the archetype and component types so that they roundtrip properly!
        let mut metadata = std::collections::HashMap::from([
            ("rerun.kind".to_owned(), ColumnKind::Component.to_string()),
            ("rerun.archetype_field".to_owned(), component.to_string()),
        ]);

        match batch_type {
            BatchType::Dataframe => {
                metadata.insert("rerun.entity_path".to_owned(), entity_path.to_string());
            }
            BatchType::Chunk => {
                // The whole chhunk is for the same entity, which is set in the record batch metadata.
                // No need to repeat it here.
            }
        }

        if let Some(archetype_name) = archetype_name {
            metadata.insert(
                "rerun.archetype".to_owned(),
                archetype_name.full_name().to_owned(),
            );
        }

        if let Some(component_type) = component_type {
            metadata.insert(
                "rerun.component".to_owned(),
                component_type.full_name().to_owned(),
            );
        }

        if *is_static {
            metadata.insert("rerun.is_static".to_owned(), "true".to_owned());
        }
        if *is_indicator {
            metadata.insert("rerun.is_indicator".to_owned(), "true".to_owned());
        }
        if *is_tombstone {
            metadata.insert("rerun.is_tombstone".to_owned(), "true".to_owned());
        }
        if *is_semantically_empty {
            metadata.insert("rerun.is_semantically_empty".to_owned(), "true".to_owned());
        }

        metadata
    }

    #[inline]
    pub fn returned_datatype(&self) -> ArrowDatatype {
        self.store_datatype.clone()
    }

    /// What we show in the UI
    pub fn display_name(&self) -> String {
        self.component_descriptor().display_name()
    }

    fn column_name_impl(&self, batch_type: BatchType, short_archetype: bool) -> String {
        self.sanity_check();

        match batch_type {
            BatchType::Chunk => {
                // All columns are of the same entity
                self.component_descriptor().display_name()
            }
            BatchType::Dataframe => {
                // Each column can be of a different entity
                match self.archetype_name {
                    Some(archetype_name) => format!(
                        "{}:{}:{}",
                        self.entity_path,
                        if short_archetype {
                            archetype_name.short_name()
                        } else {
                            archetype_name.full_name()
                        },
                        self.component
                    ),
                    None => format!("{}:{}", self.entity_path, self.component),
                }
            }
        }
    }

    /// Uses short [`ArchetypeName`]s, if present.
    pub fn column_name(&self, batch_type: BatchType) -> String {
        self.column_name_impl(batch_type, true)
    }

    /// Uses fully-qualified [`ArchetypeName`]s, if present.
    pub fn column_name_qualified(&self, batch_type: BatchType) -> String {
        self.column_name_impl(batch_type, false)
    }

    #[inline]
    pub fn to_arrow_field(&self, batch_type: BatchType) -> ArrowField {
        let nullable = true;
        ArrowField::new(
            self.column_name(batch_type),
            self.returned_datatype(),
            nullable,
        )
        .with_metadata(self.metadata(batch_type))
    }
}

impl ComponentColumnDescriptor {
    /// `chunk_entity_path`: if this column is part of a chunk batch,
    /// what is its entity path (so we can set [`ComponentColumnDescriptor::entity_path`])?
    pub fn from_arrow_field(chunk_entity_path: Option<&EntityPath>, field: &ArrowField) -> Self {
        let entity_path = if let Some(entity_path) = field.get_opt("rerun.entity_path") {
            EntityPath::parse_forgiving(entity_path)
        } else if let Some(chunk_entity_path) = chunk_entity_path {
            chunk_entity_path.clone()
        } else {
            EntityPath::root() // TODO(#8744): make entity_path optional for general sorbet batches
        };

        let component = if let Some(component) = field.get_opt("rerun.archetype_field") {
            ComponentIdentifier::from(component)
        } else {
            ComponentIdentifier::new(field.name()) // fallback
        };

        let schema = Self {
            store_datatype: field.data_type().clone(),
            entity_path,
            archetype_name: field.get_opt("rerun.archetype").map(Into::into),
            component,
            component_type: field.get_opt("rerun.component").map(Into::into),
            is_static: field.get_bool("rerun.is_static"),
            is_indicator: field.get_bool("rerun.is_indicator"),
            is_tombstone: field.get_bool("rerun.is_tombstone"),
            is_semantically_empty: field.get_bool("rerun.is_semantically_empty"),
        };

        schema.sanity_check();

        schema
    }
}
