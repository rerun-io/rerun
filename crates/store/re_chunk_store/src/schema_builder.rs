use std::collections::HashMap;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::Chunk;
use re_sorbet::{ColumnDescriptor, ComponentColumnDescriptor, SorbetColumnDescriptors};
use re_types_core::Archetype as _;

/// Helper to track static-ness ("any" semantics) and emptiness ("all" semantics) of columns across
/// a collection of chunks.
#[derive(Debug, Clone, Copy)]
struct ColumnMetadata {
    is_static: bool,
    is_semantically_empty: bool,
}

impl ColumnMetadata {
    fn merge_with(&mut self, other: Self) {
        self.is_static |= other.is_static;
        self.is_semantically_empty &= other.is_semantically_empty;
    }
}

/// Helper to build a schema from a collection of chunks belonging to the same logical store.
///
/// This keeps track of store-wide metadata for each column, such as whether the column is static or
/// semantically empty.
#[derive(Debug, Clone, Default)]
pub struct SchemaBuilder {
    columns: HashMap<ColumnDescriptor, ColumnMetadata>,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a chunk to the builder.
    #[tracing::instrument(level = "trace")]
    pub fn add_chunk(&mut self, chunk: &Chunk) -> Result<(), re_chunk::ChunkError> {
        let chunk_batch = chunk.to_chunk_batch()?;
        let chunk_schema = chunk_batch.chunk_schema();

        for (column_descriptor, array_ref) in
            (*chunk_schema.columns).iter().zip(chunk_batch.columns())
        {
            let this_metadata = match column_descriptor {
                ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => ColumnMetadata {
                    is_static: false,
                    is_semantically_empty: false,
                },

                ColumnDescriptor::Component(_) => ColumnMetadata {
                    is_static: chunk.is_static(),
                    is_semantically_empty: {
                        array_ref.downcast_array_ref().is_some_and(|list_array| {
                            re_arrow_util::is_list_array_semantically_empty(list_array)
                        })
                    },
                },
            };

            self.columns
                .entry(column_descriptor.clone())
                .and_modify(|metadata: &mut ColumnMetadata| metadata.merge_with(this_metadata))
                .or_insert(this_metadata);
        }

        Ok(())
    }

    /// Return the completed schema.
    pub fn build(self) -> SorbetColumnDescriptors {
        SorbetColumnDescriptors {
            columns: self
                .columns
                .into_iter()
                .map(|(mut column_descriptor, metadata)| {
                    match &mut column_descriptor {
                        ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => {}

                        ColumnDescriptor::Component(component_column_descriptor) => {
                            let component_descriptor =
                                component_column_descriptor.component_descriptor();

                            //TODO(#10315): we need a type safe way to do this gymnastics
                            let ComponentColumnDescriptor {
                                store_datatype: _,
                                component_name: _,
                                entity_path: _,
                                archetype_name: _,
                                archetype_field_name: _,

                                is_static,
                                is_indicator,
                                is_tombstone,
                                is_semantically_empty,
                            } = component_column_descriptor;

                            *is_static = metadata.is_static;
                            *is_semantically_empty = metadata.is_semantically_empty;
                            *is_indicator = component_descriptor.is_indicator_component();
                            *is_tombstone = re_types_core::archetypes::Clear::all_components()
                                .iter()
                                .any(|descr| descr == &component_descriptor);
                        }
                    };

                    column_descriptor
                })
                .collect(),
        }
    }
}
