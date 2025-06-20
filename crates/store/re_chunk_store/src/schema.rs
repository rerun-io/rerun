use futures::{Stream, StreamExt as _};
use std::collections::HashMap;
use std::error::Error;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::Chunk;
use re_sorbet::{ColumnDescriptor, SorbetColumnDescriptors};
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

#[derive(thiserror::Error, Debug)]
pub enum SchemaFromChunkStreamError<E> {
    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    External(E),
}

pub async fn store_schema_from_chunk_stream<E: Error>(
    mut chunks: impl Stream<Item = Result<Chunk, E>> + Unpin,
) -> Result<SorbetColumnDescriptors, SchemaFromChunkStreamError<E>> {
    let mut columns = HashMap::new();

    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(SchemaFromChunkStreamError::External)?;
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

            columns
                .entry(column_descriptor.clone())
                .and_modify(|metadata: &mut ColumnMetadata| metadata.merge_with(this_metadata))
                .or_insert(this_metadata);
        }
    }

    let columns = columns
        .into_iter()
        .map(|(mut column_descriptor, metadata)| {
            match &mut column_descriptor {
                ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => {}

                ColumnDescriptor::Component(component_column_descriptor) => {
                    let component_descriptor = component_column_descriptor.component_descriptor();

                    component_column_descriptor.is_static = metadata.is_static;
                    component_column_descriptor.is_semantically_empty =
                        metadata.is_semantically_empty;

                    component_column_descriptor.is_indicator =
                        component_descriptor.is_indicator_component();
                    component_column_descriptor.is_tombstone =
                        re_types_core::archetypes::Clear::all_components()
                            .iter()
                            .any(|descr| descr == &component_descriptor);
                }
            };

            column_descriptor
        })
        .collect();

    Ok(SorbetColumnDescriptors { columns })
}
