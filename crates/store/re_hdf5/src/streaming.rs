//! Phase B: the lazy chunk iterator over the emission plan.

use arrow::array::{Array as _, ListArray};
use re_chunk::{Chunk, ChunkComponents, ChunkId, EntityPath, TimeColumn};
use re_sdk_types::ComponentDescriptor;

use crate::convert;
use crate::error::Hdf5Error;
use crate::plan::{EmitUnit, Hdf5Plan, PlannedTimeline};
use crate::walk::DatasetDesc;

/// Emitted-chunk row bound — parity with `re_parquet`, whose chunks are bounded
/// by arrow-rs' default record-batch size. (A byte-aware target is a possible
/// later refinement.)
const MAX_ROWS_PER_CHUNK: usize = 1024;

/// A materialized `Data` unit being emitted window-by-window.
///
/// The windows are zero-copy slices of these columns, so peak memory stays
/// ≈ one entity's data while its chunks stream out row-bounded.
struct PendingData {
    entity: EntityPath,
    columns: Vec<(ComponentDescriptor, ListArray)>,
    num_rows: usize,
    next_row: usize,
}

/// Pull-based iterator that yields [`Chunk`]s from an HDF5 file.
///
/// Between units it holds only small owned state (the file handle, the
/// remaining units, the shared time buffer) — never a borrowed HDF5 handle:
/// datasets are reopened from owned path segments inside each `next()`.
pub(crate) struct Hdf5ChunkIterator {
    file: hdf5_pure::File,
    units: std::vec::IntoIter<EmitUnit>,

    /// The shared file-wide index; `Some` iff any `Data` unit exists.
    timeline: Option<PlannedTimeline>,

    use_structs: bool,
    pending: Option<PendingData>,
}

impl Hdf5ChunkIterator {
    pub fn new(file: hdf5_pure::File, plan: Hdf5Plan, use_structs: bool) -> Self {
        Self {
            file,
            units: plan.units.into_iter(),
            timeline: plan.timeline,
            use_structs,
            pending: None,
        }
    }
}

impl Iterator for Hdf5ChunkIterator {
    type Item = Result<Chunk, Hdf5Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.next_row < pending.num_rows)
            {
                return Some(self.emit_window());
            }
            self.pending = None;

            match self.units.next()? {
                EmitUnit::Attributes { entity, attrs } => {
                    return Some(build_attributes_chunk(entity, &attrs));
                }

                EmitUnit::StaticScalars { entity, datasets } => {
                    return Some(self.build_static_scalars_chunk(entity, &datasets));
                }

                EmitUnit::Data { entity, datasets } => {
                    if let Err(err) = self.materialize_data(entity, &datasets) {
                        return Some(Err(err));
                    }
                    // Loop back to emit the unit's first window (or skip it if
                    // it has zero rows).
                }
            }
        }
    }
}

impl Hdf5ChunkIterator {
    /// Read a `Data` unit's full-length columns once and stash them in `pending`.
    fn materialize_data(
        &mut self,
        entity: EntityPath,
        datasets: &[DatasetDesc],
    ) -> Result<(), Hdf5Error> {
        re_tracing::profile_function!();

        // A single-dataset group emits a bare component even in struct mode,
        // matching `re_parquet`'s carve-out.
        let columns: Vec<(ComponentDescriptor, ListArray)> =
            if self.use_structs && datasets.len() > 1 {
                let row_values = datasets
                    .iter()
                    .map(|dataset| convert::read_row_values(&self.file, dataset))
                    .collect::<Result<Vec<_>, _>>()?;
                vec![convert::build_struct_component(row_values)?]
            } else {
                datasets
                    .iter()
                    .map(|dataset| convert::read_dataset_to_list(&self.file, dataset))
                    .collect::<Result<Vec<_>, _>>()?
            };

        let num_rows = columns.first().map_or(0, |(_, column)| column.len());
        self.pending = Some(PendingData {
            entity,
            columns,
            num_rows,
            next_row: 0,
        });
        Ok(())
    }

    /// Emit the next ≤ [`MAX_ROWS_PER_CHUNK`] row window of the pending `Data`
    /// unit as one chunk, zero-copy-slicing every column and the time buffer.
    fn emit_window(&mut self) -> Result<Chunk, Hdf5Error> {
        let pending = self
            .pending
            .as_mut()
            .expect("emit_window is only called with pending rows");
        let start = pending.next_row;
        let len = (pending.num_rows - start).min(MAX_ROWS_PER_CHUNK);
        pending.next_row += len;

        let PlannedTimeline {
            timeline,
            times,
            is_sorted,
        } = self
            .timeline
            .as_ref()
            .expect("a Data unit implies a resolved row count, hence a timeline");
        // The window is a contiguous slice of the file-wide buffer, so global
        // sortedness (checked once at planning) carries over. If the buffer is
        // not globally sorted, let `TimeColumn` check the window itself — it
        // may still be locally sorted.
        let time_column = TimeColumn::new(
            is_sorted.then_some(true),
            *timeline,
            times.slice(start, len),
        );

        let components: ChunkComponents = pending
            .columns
            .iter()
            .map(|(descriptor, column)| (descriptor.clone(), column.slice(start, len)))
            .collect();

        Ok(Chunk::from_auto_row_ids(
            ChunkId::new(),
            pending.entity.clone(),
            std::iter::once((*timeline.name(), time_column)).collect(),
            components,
        )?)
    }

    /// One static chunk with each 0-D dataset as its own single-row component.
    ///
    /// `use_structs` deliberately does not apply here: static and timed data
    /// cannot share a chunk, and packing scalars into a second struct would
    /// collide with the `Data` unit's `data` component.
    fn build_static_scalars_chunk(
        &self,
        entity: EntityPath,
        datasets: &[DatasetDesc],
    ) -> Result<Chunk, Hdf5Error> {
        re_tracing::profile_function!();

        let components: ChunkComponents = datasets
            .iter()
            .map(|dataset| convert::read_dataset_to_list(&self.file, dataset))
            .collect::<Result<_, _>>()?;

        Ok(Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity,
            Default::default(),
            components,
        )?)
    }
}

/// One static chunk with one component per attribute.
fn build_attributes_chunk(
    entity: EntityPath,
    attrs: &[(String, hdf5_pure::AttrValue)],
) -> Result<Chunk, Hdf5Error> {
    let components: ChunkComponents = attrs
        .iter()
        .map(|(name, value)| convert::attr_to_component(name, value))
        .collect::<Result<_, _>>()?;

    Ok(Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity,
        Default::default(),
        components,
    )?)
}
