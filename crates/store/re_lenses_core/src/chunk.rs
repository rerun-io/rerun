use std::sync::Arc;

use itertools::Itertools as _;
use re_chunk::{Chunk, ComponentIdentifier};

use crate::{DynExpr, Lens, LensError, LensRuntimeError, Lenses, OutputMode, Runtime, Selector};

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
    fn apply_lenses(&self, lenses: &[Lens], runtime: &Runtime) -> Result<Vec<Chunk>, LensError>;

    /// Apply a selector to a single component, returning a new chunk with the
    /// component transformed in-place.
    ///
    /// All other columns (timelines, other components) are preserved unchanged.
    /// The source component's existing descriptor is preserved.
    ///
    /// For better performance, prefer [`Lens::mutate`] with [`apply_lenses`](Self::apply_lenses)
    /// which processes multiple transformations in a single pass.
    fn apply_selector(
        &self,
        source: ComponentIdentifier,
        selector: &Selector<DynExpr>,
        runtime: &Runtime,
    ) -> Result<Chunk, LensRuntimeError>;
}

impl ChunkExt for Chunk {
    fn apply_lenses(&self, lenses: &[Lens], runtime: &Runtime) -> Result<Vec<Chunk>, LensError> {
        apply_lenses_shared(Arc::new(self.clone()), lenses, runtime)
    }

    fn apply_selector(
        &self,
        source: ComponentIdentifier,
        selector: &Selector<DynExpr>,
        runtime: &Runtime,
    ) -> Result<Chunk, LensRuntimeError> {
        if !self.components().contains_component(source) {
            return Err(LensRuntimeError::ComponentNotFound {
                entity_path: self.entity_path().clone(),
                component: source,
            });
        }

        let entity_path = self.entity_path().clone();
        let selector = selector.clone();

        self.with_mapped_component(source, None, |list_array| {
            let result = runtime
                .execute_per_row(&selector, &list_array)
                .map_err(|err| LensRuntimeError::ComponentOperationFailed {
                    target_entity: entity_path.clone(),
                    input_component: source,
                    component: source,
                    source: Box::new(err),
                })?;

            result.ok_or_else(|| LensRuntimeError::NoOutputColumnsProduced {
                input_component: source,
                target_entity: entity_path.clone(),
            })
        })
    }
}

impl ChunkExt for Arc<Chunk> {
    fn apply_lenses(&self, lenses: &[Lens], runtime: &Runtime) -> Result<Vec<Chunk>, LensError> {
        apply_lenses_shared(Self::clone(self), lenses, runtime)
    }

    fn apply_selector(
        &self,
        source: ComponentIdentifier,
        selector: &Selector<DynExpr>,
        runtime: &Runtime,
    ) -> Result<Chunk, LensRuntimeError> {
        self.as_ref().apply_selector(source, selector, runtime)
    }
}

// TODO(RR-5787): return the iterator from `Lenses::apply` instead of collecting.
fn apply_lenses_shared(
    chunk: Arc<Chunk>,
    lenses: &[Lens],
    runtime: &Runtime,
) -> Result<Vec<Chunk>, LensError> {
    let mut collection = Lenses::new(OutputMode::ForwardUnmatched);
    for lens in lenses {
        collection = collection.add_lens(lens.clone());
    }

    collection.apply(chunk, runtime).try_collect()
}
