//! Phase B: the lazy chunk iterator over the emission plan.

use arrow::array::ArrayRef;
use arrow::datatypes::Field;
use re_chunk::{Chunk, ChunkComponents, ChunkId, EntityPath, TimeColumn};

use crate::config::Hdf5Config;
use crate::convert;
use crate::error::Hdf5Error;
use crate::plan::{EmitUnit, Hdf5Plan, PlannedTimeline};
use crate::walk::DatasetDesc;

/// One dataset of a `Data` unit being emitted window-by-window.
struct UnitColumn {
    desc: DatasetDesc,

    // Kept open across windows so its chunk cache carries over.
    dataset: hdf5_pure::Dataset,
}

impl UnitColumn {
    /// This column's per-row values for the row window `[start, start + len)`.
    fn window_values(&self, start: usize, len: usize) -> Result<(Field, ArrayRef), Hdf5Error> {
        convert::read_row_values(&self.dataset, &self.desc, start, len)
    }
}

/// A `Data` unit being emitted window-by-window.
///
/// Each window reads only its own rows from the file (`read_*_rows`), so peak
/// memory stays ≈ one window per column, independent of the dataset length.
struct PendingData {
    entity: EntityPath,
    columns: Vec<UnitColumn>,
    num_rows: usize,
    rows_per_window: usize,
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

    /// Byte target for one emitted chunk (and for one windowed read).
    chunk_max_bytes: u64,

    /// Row cap for one emitted chunk.
    chunk_max_rows: u64,

    pending: Option<PendingData>,
}

impl Hdf5ChunkIterator {
    pub fn new(file: hdf5_pure::File, plan: Hdf5Plan, config: &Hdf5Config) -> Self {
        Self {
            file,
            units: plan.units.into_iter(),
            timeline: plan.timeline,
            use_structs: config.use_structs,
            chunk_max_bytes: config.chunk_max_bytes,
            chunk_max_rows: config.chunk_max_rows,
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
                    if let Err(err) = self.begin_data_unit(entity, datasets) {
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
    /// Open a `Data` unit's datasets and set up its window cursor in `pending`.
    ///
    /// No row values are read here; [`Self::emit_window`] reads each window on
    /// demand.
    fn begin_data_unit(
        &mut self,
        entity: EntityPath,
        datasets: Vec<DatasetDesc>,
    ) -> Result<(), Hdf5Error> {
        re_tracing::profile_function!();

        #[expect(clippy::cast_possible_truncation)]
        let num_rows = datasets
            .first()
            .and_then(|desc| desc.shape.first())
            .copied()
            .unwrap_or(0) as usize;

        // Every column constrains the shared row window, so emitted chunks
        // stay within the configured shape.
        #[expect(clippy::cast_possible_truncation)]
        let max_rows = self.chunk_max_rows as usize;
        #[expect(clippy::cast_possible_truncation)]
        let max_bytes = self.chunk_max_bytes as usize;
        let mut rows_per_window = max_rows;
        let mut columns = Vec::with_capacity(datasets.len());
        for desc in datasets {
            let dataset = convert::open_dataset(&self.file, &desc)?;
            rows_per_window =
                rows_per_window.min((max_bytes / convert::row_byte_estimate(&desc)).max(1));
            columns.push(UnitColumn { desc, dataset });
        }

        self.pending = Some(PendingData {
            entity,
            columns,
            num_rows,
            rows_per_window,
            next_row: 0,
        });
        Ok(())
    }

    /// Emit the next row window of the pending `Data` unit as one chunk,
    /// reading each column's rows on demand and zero-copy-slicing the time
    /// buffer.
    fn emit_window(&mut self) -> Result<Chunk, Hdf5Error> {
        let pending = self
            .pending
            .as_mut()
            .expect("emit_window is only called with pending rows");
        let start = pending.next_row;
        let len = (pending.num_rows - start).min(pending.rows_per_window);
        // Advance before reading, so a failed window doesn't repeat forever.
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

        let window_values = pending
            .columns
            .iter()
            .map(|column| column.window_values(start, len))
            .collect::<Result<Vec<_>, _>>()?;

        // A single-dataset group emits a bare component even in struct mode,
        // matching `re_parquet`'s carve-out.
        let components: ChunkComponents = if self.use_structs && pending.columns.len() > 1 {
            std::iter::once(convert::build_struct_component(window_values)?).collect()
        } else {
            std::iter::zip(&pending.columns, window_values)
                .map(|(column, (field, values))| {
                    convert::values_to_component(column.desc.name(), field, values)
                })
                .collect::<Result<_, _>>()?
        };

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
