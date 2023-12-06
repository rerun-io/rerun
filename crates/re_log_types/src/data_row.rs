use ahash::HashSetExt;
use nohash_hasher::IntSet;
use smallvec::SmallVec;

use re_types_core::{AsComponents, ComponentName, SizeBytes};

use crate::{DataCell, DataCellError, DataTable, EntityPath, NumInstances, TableId, TimePoint};

// ---

/// An error that can occur because a row in the store has inconsistent columns.
#[derive(thiserror::Error, Debug)]
pub enum DataReadError {
    #[error(
        "Each cell must contain either 0, 1 or `num_instances` instances, \
        but cell '{component}' in '{entity_path}' holds {num_instances} instances \
        (expected {expected_num_instances})"
    )]
    WrongNumberOfInstances {
        entity_path: EntityPath,
        component: ComponentName,
        expected_num_instances: u32,
        num_instances: u32,
    },

    #[error(
        "Same component type present multiple times within a single row: \
        '{component}' in '{entity_path}'"
    )]
    DupedComponent {
        entity_path: EntityPath,
        component: ComponentName,
    },
}

pub type DataReadResult<T> = ::std::result::Result<T, DataReadError>;

/// A problem with a row of data in the store.
#[derive(thiserror::Error, Debug)]
pub enum DataRowError {
    #[error(transparent)]
    DataRead(#[from] DataReadError),

    #[error("Error with one or more the underlying data cells: {0}")]
    DataCell(#[from] DataCellError),

    #[error("Could not serialize/deserialize data to/from Arrow: {0}")]
    Arrow(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type DataRowResult<T> = ::std::result::Result<T, DataRowError>;

// ---

pub type DataCellVec = SmallVec<[DataCell; 4]>;

/// A row's worth of [`DataCell`]s: a collection of independent [`DataCell`]s with different
/// underlying datatypes and pointing to different parts of the heap.
///
/// Each cell in the row corresponds to a different column of the same row.
#[derive(Debug, Clone, PartialEq)]
pub struct DataCellRow(pub DataCellVec);

impl std::ops::Deref for DataCellRow {
    type Target = [DataCell];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for DataCellRow {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Index<usize> for DataCellRow {
    type Output = DataCell;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for DataCellRow {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl SizeBytes for DataCellRow {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

// ---

/// A unique ID for a [`DataRow`].
///
/// ## Semantics
///
/// [`RowId`]s play an important role in how we store, query and garbage collect data.
///
/// ### Storage
///
/// [`RowId`]s must be unique within a `DataStore`. This is enforced by the store's APIs.
///
/// This makes it easy to build and maintain secondary indices around `RowId`s with few to no
/// extraneous state tracking.
///
/// ### Query
///
/// Queries (both latest-at & range semantics) will defer to `RowId` order as a tie-breaker when
/// looking at several rows worth of data that rest at the exact same timestamp.
///
/// In pseudo-code:
/// ```text
/// rr.set_time_sequence("frame", 10)
///
/// rr.log("my_entity", point1, row_id=#1)
/// rr.log("my_entity", point2, row_id=#0)
///
/// rr.query("my_entity", at=("frame", 10))  # returns `point1`
/// ```
///
/// Think carefully about your `RowId`s when logging a lot of data at the same timestamp.
///
/// ### Garbage collection
///
/// Garbage collection happens in `RowId`-order, which roughly means that it happens in the
/// logger's wall-clock order.
///
/// This has very important implications where inserting data far into the past or into the future:
/// think carefully about your `RowId`s in these cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RowId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl RowId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);

    #[inline]
    pub fn random() -> Self {
        Self(re_tuid::Tuid::new())
    }

    /// Returns the next logical [`RowId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::random`] when unsure.
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`RowId`].
    ///
    /// This is equivalent to calling [`RowId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::random`] when unsure.
    #[inline]
    pub fn increment(&self, n: u64) -> Self {
        Self(self.0.increment(n))
    }
}

impl SizeBytes for RowId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl std::ops::Deref for RowId {
    type Target = re_tuid::Tuid;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for RowId {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

re_tuid::delegate_arrow_tuid!(RowId as "rerun.controls.RowId");

/// A row's worth of data, i.e. an event: a list of [`DataCell`]s associated with an auto-generated
/// `RowId`, a user-specified [`TimePoint`] and [`EntityPath`], and an expected number of
/// instances.
/// This is the middle layer in our data model.
///
/// Behind the scenes, a `DataRow` is backed by a collection of independent [`DataCell`]s which
/// likely refer to unrelated/non-contiguous parts of the heap.
/// Cloning a `DataRow` is not too costly but needs to be avoided on the happy path.
///
/// ## Field visibility
///
/// To facilitate destructuring (`let DataRow { .. } = row`), all the fields in `DataRow` are
/// public.
///
/// Modifying any of these fields from outside this crate is considered undefined behavior.
/// Use the appropriate getters and setters instead.
///
/// ## Layout
///
/// A row is a collection of cells where each cell must either be empty (a clear), unit-lengthed
/// (a splat) or `num_instances` long (standard): `[[C1, C1, C1], [], [C3], [C4, C4, C4], …]`.
///
/// Consider this example:
/// ```ignore
/// let num_instances = 2;
/// let points: &[MyPoint] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
/// let colors: &[_] = &[MyColor::from_rgb(128, 128, 128)];
/// let labels: &[MyLabel] = &[];
/// let row = DataRow::from_cells3(row_id, timepoint, ent_path, num_instances, (points, colors, labels));
/// ```
///
/// A row has no arrow representation nor datatype of its own, as it is merely a collection of
/// independent cells.
///
/// Visualized in the context of a larger table, it is simply a row of cells:
/// ```text
/// ┌──────────────────────────────────┬─────────────────┬───────┐
/// │ Point2D                          ┆ Color           ┆ Text  │
/// ╞══════════════════════════════════╪═════════════════╪═══════╡
/// │ [{x: 10, y: 10}, {x: 20, y: 20}] ┆ [2155905279]    ┆ []    │
/// └──────────────────────────────────┴─────────────────┴───────┘
/// ```
///
/// ## Example
///
/// ```rust
/// # use re_log_types::{
/// #     example_components::{MyColor, MyLabel, MyPoint},
/// #     DataRow, RowId, Timeline,
/// # };
/// #
/// # let row_id = RowId::ZERO;
/// # let timepoint = [
/// #     (Timeline::new_sequence("frame_nr"), 42.into()), //
/// #     (Timeline::new_sequence("clock"), 666.into()),   //
/// # ];
/// #
/// let num_instances = 2;
/// let points: &[MyPoint] = &[MyPoint { x: 10.0, y: 10.0}, MyPoint { x: 20.0, y: 20.0 }];
/// let colors: &[_] = &[MyColor(0xff7f7f7f)];
/// let labels: &[MyLabel] = &[];
///
/// let row = DataRow::from_cells3(
///     row_id,
///     "a/b/c",
///     timepoint,
///     num_instances,
///     (points, colors, labels),
/// ).unwrap();
/// eprintln!("{row}");
/// ```
#[derive(Debug, Clone)]
pub struct DataRow {
    /// Auto-generated `TUID`, uniquely identifying this event and keeping track of the client's
    /// wall-clock.
    pub row_id: RowId,

    /// User-specified [`TimePoint`] for this event.
    pub timepoint: TimePoint,

    /// User-specified [`EntityPath`] for this event.
    pub entity_path: EntityPath,

    /// The expected number of values (== component instances) in each cell.
    ///
    /// Each cell must have either:
    /// - 0 instance (clear),
    /// - 1 instance (splat),
    /// - `num_instances` instances (standard).
    pub num_instances: NumInstances,

    /// The actual cells (== columns, == components).
    pub cells: DataCellRow,
}

impl DataRow {
    /// Builds a new `DataRow` from anything implementing [`AsComponents`].
    pub fn from_archetype(
        row_id: RowId,
        timepoint: TimePoint,
        entity_path: EntityPath,
        as_components: &dyn AsComponents,
    ) -> anyhow::Result<Self> {
        re_tracing::profile_function!();

        let batches = as_components.as_component_batches();
        Self::from_component_batches(
            row_id,
            timepoint,
            entity_path,
            batches.iter().map(|batch| batch.as_ref()),
        )
    }

    /// Builds a new `DataRow` from anything implementing [`AsComponents`].
    pub fn from_component_batches<'a>(
        row_id: RowId,
        timepoint: TimePoint,
        entity_path: EntityPath,
        comp_batches: impl IntoIterator<Item = &'a dyn re_types_core::ComponentBatch>,
    ) -> anyhow::Result<Self> {
        re_tracing::profile_function!();

        let data_cells = comp_batches
            .into_iter()
            .map(DataCell::from_component_batch)
            .collect::<Result<Vec<DataCell>, _>>()?;

        // TODO(emilk): should `DataRow::from_cells` calculate `num_instances` instead?
        let num_instances = data_cells.iter().map(|cell| cell.num_instances()).max();
        let num_instances = num_instances.unwrap_or(0);

        let mut row =
            DataRow::from_cells(row_id, timepoint, entity_path, num_instances, data_cells)?;
        row.compute_all_size_bytes();
        Ok(row)
    }

    /// Builds a new `DataRow` from an iterable of [`DataCell`]s.
    ///
    /// Fails if:
    /// - one or more cell isn't 0, 1 or `num_instances` long,
    /// - two or more cells share the same component type.
    pub fn from_cells(
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        entity_path: impl Into<EntityPath>,
        num_instances: u32,
        cells: impl IntoIterator<Item = DataCell>,
    ) -> DataReadResult<Self> {
        let cells = DataCellRow(cells.into_iter().collect());

        let entity_path = entity_path.into();
        let timepoint = timepoint.into();

        let mut components = IntSet::with_capacity(cells.len());
        for cell in &*cells {
            let component = cell.component_name();

            if !components.insert(component) {
                return Err(DataReadError::DupedComponent {
                    entity_path,
                    component,
                });
            }

            match cell.num_instances() {
                0 | 1 => {}
                n if n == num_instances => {}
                n => {
                    return Err(DataReadError::WrongNumberOfInstances {
                        entity_path,
                        component,
                        expected_num_instances: num_instances,
                        num_instances: n,
                    })
                }
            }
        }

        Ok(Self {
            row_id,
            entity_path,
            timepoint,
            num_instances: num_instances.into(),
            cells,
        })
    }

    /// Consumes the [`DataRow`] and returns a new one with an incremented [`RowId`].
    #[inline]
    pub fn next(self) -> Self {
        Self {
            row_id: self.row_id.next(),
            ..self
        }
    }

    /// Turns the `DataRow` into a single-row [`DataTable`].
    #[inline]
    pub fn into_table(self) -> DataTable {
        DataTable::from_rows(TableId::random(), [self])
    }
}

impl SizeBytes for DataRow {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            row_id,
            timepoint,
            entity_path,
            num_instances,
            cells,
        } = self;

        row_id.heap_size_bytes()
            + timepoint.heap_size_bytes()
            + entity_path.heap_size_bytes()
            + num_instances.heap_size_bytes()
            + cells.heap_size_bytes()
    }
}

impl DataRow {
    #[inline]
    pub fn row_id(&self) -> RowId {
        self.row_id
    }

    #[inline]
    pub fn timepoint(&self) -> &TimePoint {
        &self.timepoint
    }

    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    #[inline]
    pub fn num_cells(&self) -> usize {
        self.cells.len()
    }

    #[inline]
    pub fn component_names(&self) -> impl ExactSizeIterator<Item = ComponentName> + '_ {
        self.cells.iter().map(|cell| cell.component_name())
    }

    #[inline]
    pub fn num_instances(&self) -> NumInstances {
        self.num_instances
    }

    #[inline]
    pub fn cells(&self) -> &DataCellRow {
        &self.cells
    }

    #[inline]
    pub fn into_cells(self) -> DataCellRow {
        self.cells
    }

    /// Returns the index of the cell with the given component type in the row, if it exists.
    ///
    /// This is `O(n)`.
    #[inline]
    pub fn find_cell(&self, component: &ComponentName) -> Option<usize> {
        self.cells
            .iter()
            .map(|cell| cell.component_name())
            .position(|name| name == *component)
    }

    /// Compute and cache the total (heap) allocated size of each individual underlying
    /// [`DataCell`].
    /// This does nothing for cells whose size has already been computed and cached before.
    ///
    /// Beware: this is _very_ costly!
    #[inline]
    pub fn compute_all_size_bytes(&mut self) {
        for cell in &mut self.cells.0 {
            cell.compute_size_bytes();
        }
    }
}

// ---

impl DataRow {
    /// A helper that combines [`Self::from_cells1`] followed by [`Self::compute_all_size_bytes`].
    ///
    /// See respective documentations for more information.
    ///
    /// Beware: this is costly!
    pub fn from_cells1_sized<C0>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: C0,
    ) -> DataReadResult<DataRow>
    where
        C0: Into<DataCell>,
    {
        let mut this = Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [into_cells.into()],
        )?;
        this.compute_all_size_bytes();
        Ok(this)
    }

    pub fn from_cells1<C0>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: C0,
    ) -> DataRowResult<DataRow>
    where
        C0: TryInto<DataCell>,
        DataRowError: From<<C0 as TryInto<DataCell>>::Error>,
    {
        Ok(Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [into_cells.try_into()?],
        )?)
    }

    /// A helper that combines [`Self::from_cells2`] followed by [`Self::compute_all_size_bytes`].
    ///
    /// See respective documentations for more information.
    ///
    /// Beware: this is costly!
    pub fn from_cells2_sized<C0, C1>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1),
    ) -> DataRowResult<DataRow>
    where
        C0: Into<DataCell>,
        C1: Into<DataCell>,
    {
        let mut this = Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.into(), //
                into_cells.1.into(), //
            ],
        )?;
        this.compute_all_size_bytes();
        Ok(this)
    }

    pub fn from_cells2<C0, C1>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1),
    ) -> DataRowResult<DataRow>
    where
        C0: TryInto<DataCell>,
        C1: TryInto<DataCell>,
        DataRowError: From<<C0 as TryInto<DataCell>>::Error>,
        DataRowError: From<<C1 as TryInto<DataCell>>::Error>,
    {
        Ok(Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.try_into()?, //
                into_cells.1.try_into()?, //
            ],
        )?)
    }

    pub fn from_cells3<C0, C1, C2>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1, C2),
    ) -> DataRowResult<DataRow>
    where
        C0: TryInto<DataCell>,
        C1: TryInto<DataCell>,
        C2: TryInto<DataCell>,
        DataRowError: From<<C0 as TryInto<DataCell>>::Error>,
        DataRowError: From<<C1 as TryInto<DataCell>>::Error>,
        DataRowError: From<<C2 as TryInto<DataCell>>::Error>,
    {
        Ok(Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.try_into()?, //
                into_cells.1.try_into()?, //
                into_cells.2.try_into()?, //
            ],
        )?)
    }
}

// ---

impl std::fmt::Display for DataRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Row #{} @ '{}'", self.row_id, self.entity_path)?;
        for (timeline, time) in &self.timepoint {
            writeln!(
                f,
                "- {}: {}",
                timeline.name(),
                timeline.typ().format_utc(*time)
            )?;
        }

        re_format::arrow::format_table(
            self.cells.iter().map(|cell| cell.to_arrow_monolist()),
            self.cells.iter().map(|cell| cell.component_name()),
        )
        .fmt(f)
    }
}
