use ahash::HashSetExt;
use nohash_hasher::IntSet;
use smallvec::SmallVec;

use crate::{
    ComponentName, DataCell, DataCellError, DataTable, EntityPath, SizeBytes, TableId, TimePoint,
};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataRowError {
    #[error(
        "Each cell must contain either 0, 1 or `num_instances` instances, \
        but cell '{component}' in '{entity_path}' holds {num_instances} instances \
        (expected {expected_num_instances}"
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

type DataCellVec = SmallVec<[DataCell; 4]>;

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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    arrow2_convert::ArrowField,
    arrow2_convert::ArrowSerialize,
    arrow2_convert::ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
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
        Self(re_tuid::Tuid::random())
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
/// (a splat) or `num_instances` long (standard): `[[C1, C1, C1], [], [C3], [C4, C4, C4], ...]`.
///
/// Consider this example:
/// ```ignore
/// let num_instances = 2;
/// let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
/// let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
/// let labels: &[Label] = &[];
/// let row = DataRow::from_cells3(row_id, timepoint, ent_path, num_instances, (points, colors, labels));
/// ```
///
/// A row has no arrow representation nor datatype of its own, as it is merely a collection of
/// independent cells.
///
/// Visualized in the context of a larger table, it is simply a row of cells:
/// ```text
/// ┌──────────────────────────────────┬─────────────────┬─────────────┐
/// │ rerun.point2d                    ┆ rerun.colorrgba ┆ rerun.label │
/// ╞══════════════════════════════════╪═════════════════╪═════════════╡
/// │ [{x: 10, y: 10}, {x: 20, y: 20}] ┆ [2155905279]    ┆ []          │
/// └──────────────────────────────────┴─────────────────┴─────────────┘
/// ```
///
/// ## Example
///
/// ```rust
/// # use re_log_types::{
/// #     component_types::{ColorRGBA, Label, Point2D},
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
/// let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
/// let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
/// let labels: &[Label] = &[];
///
/// let row = DataRow::from_cells3(
///     row_id,
///     "a/b/c",
///     timepoint,
///     num_instances,
///     (points, colors, labels),
/// );
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
    pub num_instances: u32,

    /// The actual cells (== columns, == components).
    pub cells: DataCellRow,
}

impl DataRow {
    /// Builds a new `DataRow` from an iterable of [`DataCell`]s.
    ///
    /// Fails if:
    /// - one or more cell isn't 0, 1 or `num_instances` long,
    /// - two or more cells share the same component type.
    pub fn try_from_cells(
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        entity_path: impl Into<EntityPath>,
        num_instances: u32,
        cells: impl IntoIterator<Item = DataCell>,
    ) -> DataRowResult<Self> {
        let cells = DataCellRow(cells.into_iter().collect());

        let entity_path = entity_path.into();
        let timepoint = timepoint.into();

        let mut components = IntSet::with_capacity(cells.len());
        for cell in cells.iter() {
            let component = cell.component_name();

            if !components.insert(component) {
                return Err(DataRowError::DupedComponent {
                    entity_path,
                    component,
                });
            }

            match cell.num_instances() {
                0 | 1 => {}
                n if n == num_instances => {}
                n => {
                    return Err(DataRowError::WrongNumberOfInstances {
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
            num_instances,
            cells,
        })
    }

    /// Builds a new `DataRow` from an iterable of [`DataCell`]s.
    ///
    /// Panics if:
    /// - one or more cell isn't 0, 1 or `num_instances` long,
    /// - two or more cells share the same component type.
    ///
    /// See [`Self::try_from_cells`] for the fallible alternative.
    pub fn from_cells(
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        entity_path: impl Into<EntityPath>,
        num_instances: u32,
        cells: impl IntoIterator<Item = DataCell>,
    ) -> Self {
        Self::try_from_cells(row_id, timepoint, entity_path, num_instances, cells).unwrap()
    }

    /// A helper that combines [`Self::from_cells`] followed by [`Self::compute_all_size_bytes`].
    ///
    /// See respective documentations for more information.
    ///
    /// Beware: this is costly!
    pub fn from_cells_sized(
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        entity_path: impl Into<EntityPath>,
        num_instances: u32,
        cells: impl IntoIterator<Item = DataCell>,
    ) -> Self {
        let mut this =
            Self::try_from_cells(row_id, timepoint, entity_path, num_instances, cells).unwrap();
        this.compute_all_size_bytes();
        this
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
    pub fn num_instances(&self) -> u32 {
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
    pub fn from_cells1<C0>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: C0,
    ) -> DataRow
    where
        C0: Into<DataCell>,
    {
        Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [into_cells.into()],
        )
    }

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
    ) -> DataRow
    where
        C0: Into<DataCell>,
    {
        let mut this = Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [into_cells.into()],
        );
        this.compute_all_size_bytes();
        this
    }

    pub fn try_from_cells1<C0>(
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
        Self::try_from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [into_cells.try_into()?],
        )
    }

    pub fn from_cells2<C0, C1>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1),
    ) -> DataRow
    where
        C0: Into<DataCell>,
        C1: Into<DataCell>,
    {
        Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.into(), //
                into_cells.1.into(), //
            ],
        )
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
    ) -> DataRow
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
        );
        this.compute_all_size_bytes();
        this
    }

    pub fn try_from_cells2<C0, C1>(
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
        Self::try_from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.try_into()?, //
                into_cells.1.try_into()?, //
            ],
        )
    }

    pub fn from_cells3<C0, C1, C2>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1, C2),
    ) -> DataRow
    where
        C0: Into<DataCell>,
        C1: Into<DataCell>,
        C2: Into<DataCell>,
    {
        Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.into(), //
                into_cells.1.into(), //
                into_cells.2.into(), //
            ],
        )
    }

    /// A helper that combines [`Self::from_cells3`] followed by [`Self::compute_all_size_bytes`].
    ///
    /// See respective documentations for more information.
    ///
    /// Beware: this is costly!
    pub fn from_cells3_sized<C0, C1, C2>(
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        timepoint: impl Into<TimePoint>,
        num_instances: u32,
        into_cells: (C0, C1, C2),
    ) -> DataRow
    where
        C0: Into<DataCell>,
        C1: Into<DataCell>,
        C2: Into<DataCell>,
    {
        let mut this = Self::from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.into(), //
                into_cells.1.into(), //
                into_cells.2.into(), //
            ],
        );
        this.compute_all_size_bytes();
        this
    }

    pub fn try_from_cells3<C0, C1, C2>(
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
        Self::try_from_cells(
            row_id,
            timepoint.into(),
            entity_path.into(),
            num_instances,
            [
                into_cells.0.try_into()?, //
                into_cells.1.try_into()?, //
                into_cells.2.try_into()?, //
            ],
        )
    }
}

// ---

impl std::fmt::Display for DataRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Row #{} @ '{}'", self.row_id, self.entity_path)?;
        for (timeline, time) in &self.timepoint {
            writeln!(f, "- {}: {}", timeline.name(), timeline.typ().format(*time))?;
        }

        re_format::arrow::format_table(
            self.cells.iter().map(|cell| cell.to_arrow_monolist()),
            self.cells.iter().map(|cell| cell.component_name()),
        )
        .fmt(f)
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        component_types::{ColorRGBA, Label, Point2D},
        Component as _,
    };

    #[test]
    fn data_row_error_num_instances() {
        let row_id = RowId::ZERO;
        let timepoint = TimePoint::timeless();

        let num_instances = 2;
        let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
        let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
        let labels: &[Label] = &[];

        // 0 = clear: legal
        DataRow::try_from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, labels)
            .unwrap();

        // 1 = splat: legal
        DataRow::try_from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, colors)
            .unwrap();

        // num_instances = standard: legal
        DataRow::try_from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, points)
            .unwrap();

        // anything else is illegal
        let points: &[Point2D] = &[
            [10.0, 10.0].into(),
            [20.0, 20.0].into(),
            [30.0, 30.0].into(),
        ];
        let err = DataRow::try_from_cells1(row_id, "a/b/c", timepoint, num_instances, points)
            .unwrap_err();

        match err {
            DataRowError::WrongNumberOfInstances {
                entity_path,
                component,
                expected_num_instances,
                num_instances,
            } => {
                assert_eq!(EntityPath::from("a/b/c"), entity_path);
                assert_eq!(Point2D::name(), component);
                assert_eq!(2, expected_num_instances);
                assert_eq!(3, num_instances);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn data_row_error_duped_components() {
        let row_id = RowId::ZERO;
        let timepoint = TimePoint::timeless();

        let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];

        let err =
            DataRow::try_from_cells2(row_id, "a/b/c", timepoint, 2, (points, points)).unwrap_err();

        match err {
            DataRowError::DupedComponent {
                entity_path,
                component,
            } => {
                assert_eq!(EntityPath::from("a/b/c"), entity_path);
                assert_eq!(Point2D::name(), component);
            }
            _ => unreachable!(),
        }
    }
}
