use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};

use crate::{ComponentName, DataCell, DataRow, DataRowError, EntityPath, MsgId, TimePoint};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataTableError {
    #[error("Error with one or more the underlying data rows")]
    DataRow(#[from] DataRowError),

    #[error("Could not serialize/deserialize component instances to/from Arrow")]
    Arrow(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type DataTableResult<T> = ::std::result::Result<T, DataTableError>;

// ---

/// A sparse table's worth of data, i.e. a batch of events: a collection of [`DataRow`]s.
/// This is the top-level layer in our data model.
///
/// Behind the scenes, a `DataTable` is organized in columns, where columns are represented by
/// sparse lists of [`DataCell`]s.
/// Cells within a single list are likely to reference shared, contiguous heap memory.
///
/// Cloning a `DataTable` can be _very_ costly depending on the contents.
///
/// ## Field visibility
///
/// To facilitate destructuring (`let DataTable { .. } = row`), all the fields in `DataTable` are
/// public.
///
/// Modifying any of these fields from outside this crate is considered undefined behavior.
/// Use the appropriate getters and setters instead.
///
/// ## Layout
///
/// A table is a collection of sparse rows, which are themselves collections of cells, where each
/// cell must either be empty (a clear), unit-lengthed (a splat) or `num_instances` long
/// (standard):
/// ```text
/// [
///   [[C1, C1, C1], [], [C3], [C4, C4, C4], ...],
///   [None, [C2, C2], [], [C4], ...],
///   [None, [C2, C2], [], None, ...],
///   ...
/// ]
/// ```
///
/// TODO(#1619): practical demo once we have support for table serialization (next PR)
///
/// ## Example
///
/// ```rust
/// # use re_log_types::{
/// #     component_types::{ColorRGBA, Label, MsgId, Point2D},
/// #     DataRow, DataTable, Timeline, TimePoint,
/// # };
/// #
/// # let table_id = MsgId::ZERO; // not used (yet)
/// #
/// # let timepoint = |frame_nr: i64, pouet: i64| {
/// #     TimePoint::from([
/// #         (Timeline::new_sequence("frame_nr"), frame_nr.into()),
/// #         (Timeline::new_sequence("pouet"), pouet.into()),
/// #     ])
/// # };
/// #
/// let row1 = {
///     let num_instances = 2;
///     let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
///     let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
///     let labels: &[Label] = &[];
///
///     DataRow::from_cells3(
///         MsgId::random(),
///         "a",
///         timepoint(1, 1),
///         num_instances,
///         (points, colors, labels),
///     )
/// };
///
/// let row2 = {
///     let num_instances = 0;
///     let colors: &[ColorRGBA] = &[];
///
///     DataRow::from_cells1(MsgId::random(), "b", timepoint(1, 2), num_instances, colors)
/// };
///
/// let row3 = {
///     let num_instances = 1;
///     let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
///     let labels: &[_] = &[Label("hey".into())];
///
///     DataRow::from_cells2(
///         MsgId::random(),
///         "c",
///         timepoint(2, 1),
///         num_instances,
///         (colors, labels),
///     )
/// };
///
/// let table = DataTable::from_rows(table_id, [row1, row2, row3]);
/// eprintln!("{table}");
/// ```
// TODO(#1619): introduce RowId & TableId
#[derive(Debug, Clone)]
pub struct DataTable {
    /// Auto-generated `TUID`, uniquely identifying this batch of data and keeping track of the
    /// client's wall-clock.
    // TODO(#1619): use once batching lands
    pub table_id: MsgId,

    /// The entire column of `RowId`s.
    pub row_id: Vec<MsgId>,

    /// The entire column of [`TimePoint`]s.
    pub timepoint: Vec<TimePoint>,

    /// The entire column of [`EntityPath`]s.
    pub entity_path: Vec<EntityPath>,

    /// The entire column of `num_instances`.
    pub num_instances: Vec<u32>,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component
    /// (i.e. the table is sparse).
    pub table: IntMap<ComponentName, Vec<Option<DataCell>>>,
}

impl DataTable {
    /// Builds a new `DataTable` from an iterable of [`DataRow`]s.
    pub fn from_rows(table_id: MsgId, rows: impl IntoIterator<Item = DataRow>) -> Self {
        crate::profile_function!();

        let rows = rows.into_iter();

        // Explode all rows into columns, and keep track of which components are involved.
        let mut components = IntSet::default();
        let (row_id, timepoint, entity_path, num_instances, rows): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = rows
            .map(|row| {
                components.extend(row.components());
                let DataRow {
                    row_id,
                    timepoint,
                    entity_path,
                    num_instances,
                    cells,
                } = row;
                (row_id, timepoint, entity_path, num_instances, cells)
            })
            .multiunzip();

        // Pre-allocate all columns (one per component).
        let mut table = IntMap::default();
        for component in components {
            table.insert(component, vec![None; rows.len()]);
        }

        // Fill all columns (where possible: data is likely sparse).
        for (i, row) in rows.into_iter().enumerate() {
            for cell in row {
                let component = cell.component_name();
                // NOTE: unwrap cannot fail, all arrays pre-allocated above.
                table.get_mut(&component).unwrap()[i] = Some(cell);
            }
        }

        Self {
            table_id,
            row_id,
            timepoint,
            entity_path,
            num_instances,
            table,
        }
    }
}

impl DataTable {
    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.row_id.len() as _
    }
}

// ---

// TODO(#1619): Temporary stuff while we're transitioning away from ComponentBundle/MsgBundle and
// single-row payloads. Will go away asap.

use crate::msg_bundle::MsgBundle;

impl DataTable {
    pub fn as_rows(&self) -> impl ExactSizeIterator<Item = DataRow> + '_ {
        let num_rows = self.num_rows() as usize;

        let Self {
            table_id: _,
            row_id,
            timepoint,
            entity_path,
            num_instances,
            table,
        } = self;

        (0..num_rows).map(move |i| {
            let cells = table
                .values()
                .filter_map(|rows| rows[i].clone() /* shallow */);

            DataRow::from_cells(
                row_id[i],
                timepoint[i].clone(),
                entity_path[i].clone(),
                num_instances[i],
                cells,
            )
        })
    }

    pub fn from_msg_bundle(msg_bundle: MsgBundle) -> Self {
        let MsgBundle {
            msg_id,
            entity_path,
            time_point,
            cells,
        } = msg_bundle;

        Self::from_rows(
            MsgId::ZERO, // not used (yet)
            [DataRow::from_cells(
                msg_id,
                time_point,
                entity_path,
                cells.first().map_or(0, |cell| cell.num_instances()),
                cells,
            )],
        )
    }

    pub fn into_msg_bundle(self) -> MsgBundle {
        let mut rows = self.as_rows();
        assert!(rows.len() == 1, "must have 1 row, got {}", rows.len());
        let row = rows.next().unwrap();

        let DataRow {
            row_id,
            timepoint,
            entity_path,
            num_instances: _,
            cells,
        } = row;

        let table_id = row_id; // !

        MsgBundle::new(table_id, entity_path, timepoint, cells)
    }
}

// ---

// TODO(#1619): real display impl once we have serialization support
impl std::fmt::Display for DataTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in self.as_rows() {
            writeln!(f, "{row}")?;
        }
        Ok(())
    }
}
