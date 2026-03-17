//! Incrementally maintained store schema.
//!
//! Tracks all column descriptors and per-entity component sets.
//!
//! Never affected by garbage collection.

use std::collections::BTreeMap;

use arrow::array::ListArray as ArrowListArray;
use arrow::datatypes::{DataType as ArrowDataType, Field as ArrowField};
use nohash_hasher::IntMap;

use re_byte_size::SizeBytes;
use re_chunk::ComponentIdentifier;
use re_log_types::{EntityPath, TimeType, Timeline, TimelineName};
use re_sdk_types::ComponentDescriptor;
use re_sorbet::{
    ChunkColumnDescriptors, ComponentColumnDescriptor, IndexColumnDescriptor, RowIdColumnDescriptor,
};
use re_types_core::{ArchetypeName, ComponentSet, ComponentType};

use crate::ColumnMetadataState;

/// Per-column metadata for a single component on a single entity.
#[derive(Debug, Clone)]
pub struct ColumnMetadataEntry {
    pub descriptor: ComponentDescriptor,
    pub metadata_state: ColumnMetadataState,
    pub datatype: ArrowDataType,
}

impl re_byte_size::SizeBytes for ColumnMetadataEntry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            descriptor,
            metadata_state,
            datatype,
        } = self;
        descriptor.heap_size_bytes() + metadata_state.heap_size_bytes() + datatype.heap_size_bytes()
    }
}

use crate::{ChunkComponentMeta, ChunkMeta, ChunkStoreEvent};

// ---

/// Key for looking up a [`ComponentColumnDescriptor`] in the schema.
///
/// Matches the fields used by the `Ord` implementation of [`ComponentColumnDescriptor`].
type SchemaComponentKey = (
    EntityPath,
    Option<ArchetypeName>,
    ComponentIdentifier,
    Option<ComponentType>,
);

fn schema_component_key(descr: &ComponentColumnDescriptor) -> SchemaComponentKey {
    (
        descr.entity_path.clone(),
        descr.archetype,
        descr.component,
        descr.component_type,
    )
}

// ---

/// Incrementally maintained store schema.
///
/// Contains [`ChunkColumnDescriptors`] and per-entity component sets.
/// Updated via [`Self::on_events`] when chunks are inserted or RRD manifests are ingested.
/// Never affected by garbage collection.
#[derive(Debug, Clone, Default)]
pub struct StoreSchema {
    /// The _latest_ [`TimeType`] for each timeline name.
    time_type_registry: IntMap<TimelineName, TimeType>,

    /// All component column descriptors ever seen, keyed for fast lookup/update.
    components: BTreeMap<SchemaComponentKey, ComponentColumnDescriptor>,

    /// Per-entity set of all components ever seen (sorted).
    components_per_entity: IntMap<EntityPath, ComponentSet>,

    // TODO(grtlr): Can we slim this map down by getting rid of `ComponentIdentifier`-level here?
    // Ideally, we'd even merge this with the above fields. We are currently storing a lot of
    // redundant information.
    per_column_metadata: IntMap<EntityPath, IntMap<ComponentIdentifier, ColumnMetadataEntry>>,
}

impl StoreSchema {
    /// Retrieve all timelines in the store.
    #[inline]
    pub fn timelines(&self) -> BTreeMap<TimelineName, Timeline> {
        self.time_type_registry
            .iter()
            .map(|(name, typ)| (*name, Timeline::new(*name, *typ)))
            .collect()
    }

    /// Lookup the _latest_ [`TimeType`] used by a specific [`TimelineName`].
    #[inline]
    pub fn time_column_type(&self, timeline_name: &TimelineName) -> Option<TimeType> {
        self.time_type_registry.get(timeline_name).copied()
    }

    /// Returns all [`ComponentIdentifier`]s that have ever been written to the given entity, sorted.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    #[inline]
    pub fn all_components_for_entity(&self, entity_path: &EntityPath) -> Option<&ComponentSet> {
        self.components_per_entity.get(entity_path)
    }

    /// Retrieves the [`ComponentDescriptor`] at a given [`EntityPath`] that has a certain [`ComponentIdentifier`].
    pub fn entity_component_descriptor(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<ComponentDescriptor> {
        self.per_column_metadata
            .get(entity_path)
            .and_then(|per_identifier| per_identifier.get(&component))
            .map(|entry| entry.descriptor.clone())
    }

    /// Get the [`re_types_core::ComponentType`] and [`ArrowDataType`] for a specific [`EntityPath`] and [`ComponentIdentifier`].
    pub fn lookup_component_type(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<(Option<ComponentType>, ArrowDataType)> {
        let entry = self
            .per_column_metadata
            .get(entity_path)
            .and_then(|per_identifier| per_identifier.get(&component))?;
        Some((entry.descriptor.component_type, entry.datatype.clone()))
    }

    /// Lookup the `ColumnMetadataState` for a specific [`EntityPath`] and [`ComponentIdentifier`].
    pub fn lookup_column_metadata_state(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<&ColumnMetadataState> {
        self.per_column_metadata
            .get(entity_path)
            .and_then(|per_identifier| per_identifier.get(&component))
            .map(|entry| &entry.metadata_state)
    }

    /// Checks whether any column in the store with the given [`re_types_core::ComponentType`] has a datatype
    /// that differs from `expected_datatype`.
    ///
    /// This iterates over all entities, so it should not be called in a hot path.
    pub fn has_mismatched_datatype_for_component_type(
        &self,
        component_type: &ComponentType,
        expected_datatype: &ArrowDataType,
    ) -> Option<&ArrowDataType> {
        re_tracing::profile_function!();
        for per_component in self.per_column_metadata.values() {
            for entry in per_component.values() {
                if entry.descriptor.component_type.as_ref() == Some(component_type)
                    && entry.datatype != *expected_datatype
                {
                    return Some(&entry.datatype);
                }
            }
        }
        None
    }

    /// Access the per-column metadata for a given entity.
    pub fn per_column_metadata_for_entity(
        &self,
        entity_path: &EntityPath,
    ) -> Option<&IntMap<ComponentIdentifier, ColumnMetadataEntry>> {
        self.per_column_metadata.get(entity_path)
    }

    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn chunk_column_descriptors(&self) -> ChunkColumnDescriptors {
        let mut indices: Vec<IndexColumnDescriptor> = self
            .time_type_registry
            .iter()
            .map(|(name, typ)| IndexColumnDescriptor::from(Timeline::new(*name, *typ)))
            .collect();
        indices.sort();

        ChunkColumnDescriptors {
            row_id: RowIdColumnDescriptor::from_sorted(false),
            indices,
            components: self.components.values().cloned().collect(),
        }
    }

    /// Update per-entity component set and per-column metadata for a single component.
    ///
    /// Returns `Some(ChunkComponentMeta)` when a schema event should be emitted,
    /// i.e. when the column is genuinely new or `is_static` transitions from `false` to `true`.
    fn update_column_metadata(
        &mut self,
        col_descr: &ComponentColumnDescriptor,
    ) -> Option<ChunkComponentMeta> {
        let ComponentColumnDescriptor {
            entity_path,
            component,
            is_static,
            is_semantically_empty,
            store_datatype: _,
            component_type: _,
            archetype: _,
            is_tombstone: _,
        } = col_descr;
        let descriptor = col_descr.component_descriptor();
        let inner_datatype = col_descr.inner_datatype();
        let metadata_state = ColumnMetadataState {
            is_semantically_empty: *is_semantically_empty,
            is_static: *is_static,
        };

        let key = schema_component_key(col_descr);
        self.components
            .entry(key)
            .and_modify(|existing| {
                existing.is_static |= is_static;
                existing.is_semantically_empty &= is_semantically_empty;
            })
            .or_insert_with(|| col_descr.clone());

        let is_new = self
            .components_per_entity
            .entry(entity_path.clone())
            .or_default()
            .insert(*component);

        let prev_is_static = self
            .per_column_metadata
            .get(entity_path)
            .and_then(|per_id| per_id.get(component))
            .map(|e| e.metadata_state.is_static);

        let entry = self
            .per_column_metadata
            .entry(entity_path.clone())
            .or_default()
            .entry(*component)
            .and_modify(|e| {
                if e.datatype != inner_datatype {
                    // TODO(grtlr): If we encounter two different data types, we should split the chunk.
                    // More information: https://github.com/rerun-io/rerun/pull/10082#discussion_r2140549340
                    re_log::warn_once!(
                        "Datatype of column {} in {entity_path} has changed from {} to {inner_datatype}",
                        e.descriptor,
                        e.datatype,
                    );
                    e.datatype = inner_datatype.clone();
                }
                e.metadata_state.is_static |= is_static;
                e.metadata_state.is_semantically_empty &= is_semantically_empty;
            })
            .or_insert_with(|| ColumnMetadataEntry {
                descriptor: descriptor.clone(),
                metadata_state,
                datatype: inner_datatype.clone(),
            });

        let new_is_static = entry.metadata_state.is_static;
        let static_changed = prev_is_static.is_some_and(|prev| !prev && new_is_static);

        if is_new || static_changed {
            Some(ChunkComponentMeta {
                descriptor: descriptor.clone(),
                inner_arrow_datatype: Some(inner_datatype.clone()),
                has_data: !entry.metadata_state.is_semantically_empty,
                is_static: new_is_static,
            })
        } else {
            None
        }
    }

    // --- Updating via events ---

    /// Update the schema from store events.
    ///
    /// This processes addition events (both physical chunk additions and virtual
    /// manifest additions). Deletion events and schema column addition events are
    /// ignored since the schema is purely additive and schema events are output, not input.
    ///
    /// Returns newly discovered entity/component pairs grouped by entity.
    pub fn on_events(&mut self, events: &[ChunkStoreEvent]) -> Vec<ChunkMeta> {
        re_tracing::profile_function!();

        let mut all_new: nohash_hasher::IntMap<EntityPath, Vec<ChunkComponentMeta>> =
            Default::default();

        for event in events {
            match &event.diff {
                crate::ChunkStoreDiff::Addition(add) => {
                    for new_col in self.on_chunk_addition(&add.chunk_after_processing) {
                        all_new
                            .entry(add.chunk_after_processing.entity_path().clone())
                            .or_default()
                            .push(new_col);
                    }
                }
                crate::ChunkStoreDiff::VirtualAddition(vadd) => {
                    for (entity_path, new_cols) in self.on_rrd_manifest(&vadd.rrd_manifest) {
                        all_new.entry(entity_path).or_default().extend(new_cols);
                    }
                }
                crate::ChunkStoreDiff::Deletion(_) | crate::ChunkStoreDiff::SchemaAddition(_) => {
                    // Schema is purely additive — deletions and schema column addition events are ignored.
                }
            }
        }

        all_new
            .into_iter()
            .map(|(entity_path, components)| ChunkMeta {
                entity_path,
                components,
            })
            .collect()
    }

    /// Returns [`ChunkComponentMeta`] for each genuinely new component column.
    fn on_chunk_addition(&mut self, chunk: &re_chunk::Chunk) -> Vec<ChunkComponentMeta> {
        let is_static = chunk.is_static();

        // Update time type registry
        for (name, time_column) in chunk.timelines() {
            let new_typ = time_column.timeline().typ();
            if let Some(old_typ) = self.time_type_registry.insert(*name, new_typ)
                && old_typ != new_typ
            {
                re_log::warn_once!(
                    "Timeline '{name}' changed type from {old_typ:?} to {new_typ:?}. \
                        Rerun does not support using different types for the same timeline.",
                );
            }
        }

        let entity_path = chunk.entity_path();

        let mut new_columns = Vec::new();

        // Update component columns and per-entity component sets
        for column in chunk.components().values() {
            let descriptor = &column.descriptor;
            let component = descriptor.component;

            let is_semantically_empty =
                re_arrow_util::is_list_array_semantically_empty(&column.list_array);

            use re_types_core::Archetype as _;
            let is_tombstone = re_types_core::archetypes::Clear::all_components()
                .iter()
                .any(|descr| descr.component == component);

            let col_descr = ComponentColumnDescriptor {
                store_datatype: ArrowListArray::DATA_TYPE_CONSTRUCTOR(
                    ArrowField::new("item", column.list_array.value_type().clone(), true).into(),
                ),
                entity_path: entity_path.clone(),
                archetype: descriptor.archetype,
                component: descriptor.component,
                component_type: descriptor.component_type,
                is_static,
                is_tombstone,
                is_semantically_empty,
            };

            if let Some(meta) = self.update_column_metadata(&col_descr) {
                new_columns.push(meta);
            }
        }

        new_columns
    }

    /// Returns newly inserted columns grouped by entity path.
    fn on_rrd_manifest(
        &mut self,
        rrd_manifest: &re_log_encoding::RrdManifest,
    ) -> Vec<(EntityPath, Vec<ChunkComponentMeta>)> {
        let sorbet_schema = rrd_manifest.recording_schema();

        // Update time type registry
        for descr in sorbet_schema.columns.index_columns() {
            self.time_type_registry
                .insert(descr.timeline_name(), descr.timeline().typ());
        }

        let mut new_per_entity: nohash_hasher::IntMap<EntityPath, Vec<ChunkComponentMeta>> =
            Default::default();

        // Update component columns and per-entity component sets
        for descr in sorbet_schema.columns.component_columns() {
            if let Some(meta) = self.update_column_metadata(descr) {
                new_per_entity
                    .entry(descr.entity_path.clone())
                    .or_default()
                    .push(meta);
            }
        }

        new_per_entity.into_iter().collect()
    }

    /// Remove all data for a given entity path.
    ///
    /// Called from `ChunkStore::drop_entity_path`.
    pub fn drop_entity(&mut self, entity_path: &EntityPath) {
        self.components.retain(|key, _| key.0 != *entity_path);
        self.components_per_entity.remove(entity_path);
        self.per_column_metadata.remove(entity_path);
    }
}

impl SizeBytes for StoreSchema {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            time_type_registry,
            components,
            components_per_entity,
            per_column_metadata,
        } = self;

        time_type_registry.heap_size_bytes()
            + components.heap_size_bytes()
            + components_per_entity.heap_size_bytes()
            + per_column_metadata.heap_size_bytes()
    }
}
