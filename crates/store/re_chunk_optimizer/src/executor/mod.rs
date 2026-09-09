//! The executor: drives a plan against a [`ChunkProvider`].

pub mod merge_split;

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use itertools::Itertools as _;

use re_chunk::{Chunk, ChunkId, ComponentIdentifier};
use re_log_encoding::ChunkProvider;

use crate::Error;
use crate::plan::{ChunkSlice, ColumnSelection, PlanUnit};
use crate::view::ChunkIndexView;

use merge_split::MergeSplitRunState;

/// Pull-based execution of a plan: chunks are loaded on demand as [`Self::next_chunk`] is driven.
///
/// # Implementation notes
///
/// For now, the executor is a simple state machine that executes one plan unit at a time, with a
/// FIFO queue to hold generated chunks until they are pulled from the stream.
pub struct Executor {
    provider: Arc<dyn ChunkProvider>,
    view: ChunkIndexView,

    /// Previously produced output chunk ready for streaming.
    ready: VecDeque<Arc<Chunk>>,

    /// The remaining plan units to execute, consumed one at a time.
    units: std::vec::IntoIter<PlanUnit>,

    /// State of the in-flight merge/split run, if any.
    run: Option<MergeSplitRunState>,
}

impl Executor {
    pub fn new(
        provider: Arc<dyn ChunkProvider>,
        view: ChunkIndexView,
        units: Vec<PlanUnit>,
    ) -> Self {
        Self {
            provider,
            view,
            units: units.into_iter(),
            run: None,
            ready: VecDeque::new(),
        }
    }

    /// The next optimized chunk, or `None` when done.
    pub async fn next_chunk(&mut self) -> Result<Option<Arc<Chunk>>, Error> {
        loop {
            if let Some(chunk) = self.ready.pop_front() {
                return Ok(Some(chunk));
            }

            if let Some(run) = &mut self.run {
                let flow = run
                    .step(self.provider.as_ref(), &self.view, &mut self.ready)
                    .await?;
                if flow.is_break() {
                    self.run = None;
                }
                continue;
            }

            let Some(output) = self.units.next() else {
                return Ok(None);
            };

            match output {
                PlanUnit::Passthrough(slice) => {
                    let chunks =
                        load_in_order(self.provider.as_ref(), &self.view, &[slice]).await?;
                    self.ready.extend(chunks);
                }

                PlanUnit::MergeSplitRun { inputs, target } => {
                    self.run = Some(MergeSplitRunState::new(inputs, target));
                }
            }
        }
    }
}

/// Load the chunks the slices name and return one input chunk per slice, in slice order.
pub async fn load_in_order(
    provider: &dyn ChunkProvider,
    view: &ChunkIndexView,
    slices: &[ChunkSlice],
) -> Result<Vec<Arc<Chunk>>, Error> {
    let Some(first) = slices.first() else {
        return Ok(Vec::new());
    };

    let mut ids: Vec<ChunkId> = slices
        .iter()
        .map(|slice| view.chunk(slice.chunk).chunk_id)
        .collect();
    ids.sort_unstable();
    ids.dedup();

    // Every planned node stays within one entity, so the first chunk names them all.
    let loaded = provider
        .load_chunks(&ids)
        .await
        .map_err(|err| Error::load_chunks(&view.chunk(first.chunk).entity_path, ids.len(), err))?;

    let by_id: HashMap<ChunkId, Arc<Chunk>> = loaded
        .into_iter()
        .map(|chunk| (chunk.id(), chunk))
        .collect();

    let mut chunks = Vec::with_capacity(slices.len());
    for slice in slices {
        let meta = view.chunk(slice.chunk);
        let chunk = by_id
            .get(&meta.chunk_id)
            .ok_or_else(|| Error::missing_chunk(meta.chunk_id, &meta.entity_path))?;
        chunks.push(select_columns(Arc::clone(chunk), &slice.columns)?);
    }
    Ok(chunks)
}

/// The input chunk a column selection keeps of a decoded chunk.
///
/// A selection that keeps every column of the chunk returns the same `Arc`. A selection naming a
/// column the chunk lacks, or keeping no column at all, means the chunk index and the chunk
/// disagree and is an error.
pub fn select_columns(chunk: Arc<Chunk>, columns: &ColumnSelection) -> Result<Arc<Chunk>, Error> {
    let ensure_columns_are_present =
        |chunk: &Chunk, columns: &BTreeSet<ComponentIdentifier>| -> Result<(), Error> {
            let components = chunk.components();
            let missing = columns
                .iter()
                .filter(|col| !components.contains_component(**col))
                .copied()
                .collect_vec();
            if !missing.is_empty() {
                return Err(Error::index_mismatch(
                    chunk.id(),
                    chunk.entity_path(),
                    missing,
                ));
            }
            Ok(())
        };

    match columns {
        ColumnSelection::All => Ok(chunk),

        ColumnSelection::Only(columns) => {
            ensure_columns_are_present(&chunk, columns)?;
            if columns.len() == chunk.components().len() {
                return Ok(chunk);
            }
            Ok(Arc::new(
                chunk.components_sliced(&columns.iter().copied().collect_vec()),
            ))
        }

        ColumnSelection::Except(columns) => {
            ensure_columns_are_present(&chunk, columns)?;

            if columns.len() == chunk.components().len() {
                return Err(Error::empty_selection(chunk.id(), chunk.entity_path()));
            }

            Ok(Arc::new(chunk.components_dropped(
                &columns.iter().copied().collect_vec(),
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use re_chunk::{Chunk, ChunkId, RowId};
    use re_log_types::Timeline;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_types_core::ComponentBatch as _;

    use super::select_columns;
    use crate::Error;
    use crate::plan::ColumnSelection;

    /// Covering selections return the same `Arc`; a column the chunk lacks, or a selection keeping
    /// nothing, is an error; a partial selection is a new chunk with the selected columns.
    #[test]
    fn test_select_columns() -> Result<(), Error> {
        let frame = Timeline::new_sequence("frame");
        let points = MyPoints::descriptor_points().component;
        let colors = MyPoints::descriptor_colors().component;
        let labels = MyPoints::descriptor_labels().component;

        let mixed = Arc::new(
            Chunk::builder_with_id(ChunkId::from_u128(1), "entity")
                .with_serialized_batches(
                    RowId::from_u128(1),
                    [(frame, 0_i64)],
                    [
                        MyPoint::from_iter(0..4)
                            .try_serialized(MyPoints::descriptor_points())
                            .unwrap(),
                        MyColor::from_iter(0..4)
                            .try_serialized(MyPoints::descriptor_colors())
                            .unwrap(),
                    ],
                )
                .build()
                .unwrap(),
        );
        let colors_only = Arc::new(mixed.components_sliced(&[colors]));

        // Identity.
        assert!(Arc::ptr_eq(
            &select_columns(Arc::clone(&mixed), &ColumnSelection::All)?,
            &mixed
        ));
        assert!(Arc::ptr_eq(
            &select_columns(
                Arc::clone(&colors_only),
                &ColumnSelection::Only(BTreeSet::from([colors]))
            )?,
            &colors_only
        ));

        // A column the chunk lacks, or nothing kept.
        assert!(matches!(
            select_columns(
                Arc::clone(&mixed),
                &ColumnSelection::Only(BTreeSet::from([labels]))
            ),
            Err(Error::IndexMismatch { missing, .. }) if missing == vec![labels]
        ));
        assert!(matches!(
            select_columns(
                Arc::clone(&mixed),
                &ColumnSelection::Except(BTreeSet::from([points, colors]))
            ),
            Err(Error::EmptySelection { .. })
        ));

        // Partial selections: a new id, the selected columns, every row kept.
        let only_colors = select_columns(
            Arc::clone(&mixed),
            &ColumnSelection::Only(BTreeSet::from([colors])),
        )?;
        assert_ne!(only_colors.id(), mixed.id());
        assert_eq!(only_colors.num_rows(), 1);
        assert_eq!(
            only_colors.components().keys().copied().collect::<Vec<_>>(),
            vec![colors]
        );

        let except_colors = select_columns(
            Arc::clone(&mixed),
            &ColumnSelection::Except(BTreeSet::from([colors])),
        )?;
        assert_ne!(except_colors.id(), mixed.id());
        assert_eq!(
            except_colors
                .components()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![points]
        );

        Ok(())
    }
}
