use re_chunk::{Chunk, ComponentIdentifier};

use crate::{DynExpr, Lens, LensError, Lenses, OutputMode, PartialChunk, Selector};

/// Extension methods for applying lenses to a [`Chunk`].
pub trait ChunkExt {
    /// Apply one or more lenses to this chunk, returning transformed chunks.
    ///
    /// Each lens matches by input component. Columns not consumed by any
    /// matching lens are forwarded unchanged as a separate chunk
    /// ([`OutputMode::ForwardUnmatched`]).
    ///
    /// If no lens matches the chunk (including when an empty slice is passed),
    /// the original chunk is returned unchanged.
    fn apply_lenses(&self, lenses: &[Lens]) -> Result<Vec<Chunk>, PartialChunk>;

    /// Apply a selector to a single component, returning a new chunk with the
    /// component transformed in-place.
    ///
    /// All other columns (timelines, other components) are preserved unchanged.
    /// The source component's existing descriptor is preserved.
    fn apply_selector(
        &self,
        source: ComponentIdentifier,
        selector: &Selector<DynExpr>,
    ) -> Result<Chunk, LensError>;
}

impl ChunkExt for Chunk {
    fn apply_lenses(&self, lenses: &[Lens]) -> Result<Vec<Chunk>, PartialChunk> {
        let mut collection = Lenses::new(OutputMode::ForwardUnmatched);
        for lens in lenses {
            collection = collection.add_lens(lens.clone());
        }

        collection.apply(self).collect::<Result<Vec<_>, _>>()
    }

    fn apply_selector(
        &self,
        source: ComponentIdentifier,
        selector: &Selector<DynExpr>,
    ) -> Result<Chunk, LensError> {
        if !self.components().contains_component(source) {
            return Err(LensError::ComponentNotFound {
                entity_path: self.entity_path().clone(),
                component: source,
            });
        }

        let entity_path = self.entity_path().clone();
        let selector = selector.clone();

        self.with_mapped_component(source, None, |list_array| {
            let result = selector.execute_per_row(&list_array).map_err(|err| {
                LensError::ComponentOperationFailed {
                    target_entity: entity_path.clone(),
                    input_component: source,
                    component: source,
                    source: Box::new(err),
                }
            })?;

            result.ok_or_else(|| LensError::NoOutputColumnsProduced {
                input_component: source,
                target_entity: entity_path.clone(),
            })
        })
    }
}
