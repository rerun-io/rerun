//! All the APIs used specifically for `re_dataframe`.

use std::collections::{BTreeMap, BTreeSet};

use ahash::HashSet;
use arrow2::{
    array::ListArray as ArrowListArray,
    datatypes::{DataType as ArrowDatatype, Field as ArrowField},
};
use itertools::Itertools as _;

use re_chunk::{LatestAtQuery, TimelineName};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_log_types::{EntityPathFilter, ResolvedTimeRange};
use re_types_core::{ArchetypeName, ComponentName};

use crate::ChunkStore;

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// --- Descriptors ---

/// When selecting secondary component columns, specify how the joined data should be encoded.
///
/// Because range-queries often involve repeating the same joined-in data multiple times,
/// the strategy we choose for joining can have a significant impact on the size and memory
/// overhead of the `RecordBatch`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinEncoding {
    /// Slice the `RecordBatch` to minimal overlapping sub-ranges.
    ///
    /// This is the default, and should always be used for the POV component which defines
    /// the optimal size for `RecordBatch`.
    ///
    /// This minimizes the need for allocation, but at the cost of `RecordBatch`es that are
    /// almost always smaller than the optimal size. In the common worst-case, this will result
    /// in single-row `RecordBatch`es.
    #[default]
    OverlappingSlice,

    /// Dictionary-encode the joined column.
    ///
    /// Using dictionary-encoding allows any repeated data to be shared between rows,
    /// but comes with the cost of an extra dictionary-lookup indirection.
    ///
    /// Note that this changes the physical type of the returned column.
    ///
    /// Using this encoding for complex types is incompatible with some arrow libraries.
    DictionaryEncode,
    //
    // TODO(jleibs):
    // RepeatCopy,
    //
    // Repeat the joined column by physically copying the data.
    //
    // This will always allocate a new column in the `RecordBatch`, matching the size of the
    // POV component.
    //
    // This is the most expensive option, but can make working with the data more efficient,
    // especially when the copied column is small.
    //
}

// TODO(#6889): At some point all these descriptors needs to be interned and have handles or
// something. And of course they need to be codegen. But we'll get there once we're back to
// natively tagged components.

// Describes any kind of column.
//
// See:
// * [`TimeColumnDescriptor`]
// * [`ComponentColumnDescriptor`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColumnDescriptor {
    Time(TimeColumnDescriptor),
    Component(ComponentColumnDescriptor),
}

impl ColumnDescriptor {
    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::Time(_) => None,
            Self::Component(descr) => Some(&descr.entity_path),
        }
    }

    #[inline]
    pub fn datatype(&self) -> ArrowDatatype {
        match self {
            Self::Time(descr) => descr.datatype.clone(),
            Self::Component(descr) => descr.returned_datatype(),
        }
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        match self {
            Self::Time(descr) => descr.to_arrow_field(),
            Self::Component(descr) => descr.to_arrow_field(),
        }
    }

    #[inline]
    pub fn short_name(&self) -> String {
        match self {
            Self::Time(descr) => descr.timeline.name().to_string(),
            Self::Component(descr) => descr.component_name.short_name().to_owned(),
        }
    }
}

/// Describes a time column, such as `log_time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeColumnDescriptor {
    /// The timeline this column is associated with.
    pub timeline: Timeline,

    /// The Arrow datatype of the column.
    pub datatype: ArrowDatatype,
}

impl PartialOrd for TimeColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            timeline,
            datatype: _,
        } = self;
        timeline.cmp(&other.timeline)
    }
}

impl TimeColumnDescriptor {
    #[inline]
    // Time column must be nullable since static data doesn't have a time.
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self { timeline, datatype } = self;
        ArrowField::new(
            timeline.name().to_string(),
            datatype.clone(),
            true, /* nullable */
        )
    }
}

/// Describes a data/component column, such as `Position3D`.
//
// TODO(#6889): Fully sorbetize this thing? `ArchetypeName` and such don't make sense in that
// context. And whatever `archetype_field_name` ends up being, it needs interning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentColumnDescriptor {
    /// The path of the entity.
    pub entity_path: EntityPath,

    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    pub archetype_name: Option<ArchetypeName>,

    /// Optional name of the field within `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `positions`.
    pub archetype_field_name: Option<String>,

    /// Semantic name associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    pub component_name: ComponentName,

    /// The Arrow datatype of the stored column.
    ///
    /// This is the log-time datatype corresponding to how this data is encoded
    /// in a chunk. Currently this will always be an [`ArrowListArray`], but as
    /// we introduce mono-type optimization, this might be a native type instead.
    pub store_datatype: ArrowDatatype,

    /// How the data will be joined into the resulting `RecordBatch`.
    //
    // TODO(cmc): remove with the old re_dataframe.
    pub join_encoding: JoinEncoding,

    /// Whether this column represents static data.
    pub is_static: bool,
}

impl PartialOrd for ComponentColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ComponentColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            join_encoding: _,
            store_datatype: _,
            is_static: _,
        } = self;

        entity_path
            .cmp(&other.entity_path)
            .then_with(|| component_name.cmp(&other.component_name))
            .then_with(|| archetype_name.cmp(&other.archetype_name))
            .then_with(|| archetype_field_name.cmp(&other.archetype_field_name))
    }
}

impl std::fmt::Display for ComponentColumnDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            join_encoding: _,
            store_datatype: _,
            is_static,
        } = self;

        let s = match (archetype_name, component_name, archetype_field_name) {
            (None, component_name, None) => component_name.to_string(),
            (Some(archetype_name), component_name, None) => format!(
                "{entity_path}@{}::{}",
                archetype_name.short_name(),
                component_name.short_name(),
            ),
            (None, component_name, Some(archetype_field_name)) => format!(
                "{entity_path}@{}#{archetype_field_name}",
                component_name.short_name(),
            ),
            (Some(archetype_name), component_name, Some(archetype_field_name)) => format!(
                "{entity_path}@{}::{}#{archetype_field_name}",
                archetype_name.short_name(),
                component_name.short_name(),
            ),
        };

        if *is_static {
            f.write_fmt(format_args!("|{s}|"))
        } else {
            f.write_str(&s)
        }
    }
}

impl ComponentColumnDescriptor {
    #[inline]
    pub fn new<C: re_types_core::Component>(entity_path: EntityPath) -> Self {
        let join_encoding = JoinEncoding::default();

        // NOTE: The data is always a at least a list, whether it's latest-at or range.
        // It might be wrapped further in e.g. a dict, but at the very least
        // it's a list.
        let store_datatype = ArrowListArray::<i32>::default_datatype(C::arrow_datatype());

        Self {
            entity_path,
            archetype_name: None,
            archetype_field_name: None,
            component_name: C::name(),
            join_encoding,
            store_datatype,
            is_static: false,
        }
    }

    #[inline]
    pub fn matches(&self, entity_path: &EntityPath, component_name: &ComponentName) -> bool {
        &self.entity_path == entity_path && &self.component_name == component_name
    }

    fn metadata(&self) -> arrow2::datatypes::Metadata {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            join_encoding: _,
            store_datatype: _,
            is_static,
        } = self;

        [
            (*is_static).then_some(("sorbet.is_static".to_owned(), "yes".to_owned())),
            Some(("sorbet.path".to_owned(), entity_path.to_string())),
            Some((
                "sorbet.semantic_type".to_owned(),
                component_name.short_name().to_owned(),
            )),
            archetype_name.map(|name| {
                (
                    "sorbet.semantic_family".to_owned(),
                    name.short_name().to_owned(),
                )
            }),
            archetype_field_name
                .as_ref()
                .map(|name| ("sorbet.logical_type".to_owned(), name.to_owned())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    pub fn returned_datatype(&self) -> ArrowDatatype {
        match self.join_encoding {
            JoinEncoding::OverlappingSlice => self.store_datatype.clone(),
            JoinEncoding::DictionaryEncode => ArrowDatatype::Dictionary(
                arrow2::datatypes::IntegerType::Int32,
                std::sync::Arc::new(self.store_datatype.clone()),
                true,
            ),
        }
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        ArrowField::new(
            format!(
                "{}:{}",
                self.entity_path,
                self.component_name.short_name().to_owned()
            ),
            self.returned_datatype(),
            true, /* nullable */
        )
        // TODO(#6889): This needs some proper sorbetization -- I just threw these names randomly.
        .with_metadata(self.metadata())
    }

    #[inline]
    pub fn with_join_encoding(mut self, join_encoding: JoinEncoding) -> Self {
        self.join_encoding = join_encoding;
        self
    }
}

// --- Selectors ---

/// Describes a column selection to return as part of a query.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnSelector {
    Time(TimeColumnSelector),
    Component(ComponentColumnSelector),
    //TODO(jleibs): Add support for archetype-based component selection.
    //ArchetypeField(ArchetypeFieldColumnSelector),
}

impl From<ColumnDescriptor> for ColumnSelector {
    #[inline]
    fn from(desc: ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::Time(desc) => Self::Time(desc.into()),
            ColumnDescriptor::Component(desc) => Self::Component(desc.into()),
        }
    }
}

impl From<TimeColumnSelector> for ColumnSelector {
    #[inline]
    fn from(desc: TimeColumnSelector) -> Self {
        Self::Time(desc)
    }
}

impl From<ComponentColumnSelector> for ColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnSelector) -> Self {
        Self::Component(desc)
    }
}

/// Select a time column.
//
// TODO(cmc): This shouldn't be specific to time, this should be an `IndexColumnSelector` or smth.
// Particularly unfortunate that this one already leaks into the public API…
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeColumnSelector {
    /// The name of the timeline.
    pub timeline: TimelineName,
}

impl From<TimeColumnDescriptor> for TimeColumnSelector {
    #[inline]
    fn from(desc: TimeColumnDescriptor) -> Self {
        Self {
            timeline: *desc.timeline.name(),
        }
    }
}

/// Select a component based on its `EntityPath` and `ComponentName`.
///
/// Note, that in the future when Rerun supports duplicate tagged components
/// on the same entity, this selector may be ambiguous. In this case, the
/// query result will return an Error if it cannot determine a single selected
/// component.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentColumnSelector {
    /// The path of the entity.
    pub entity_path: EntityPath,

    /// Semantic name associated with this data.
    //
    // TODO(cmc): this should be `component_name`.
    pub component: ComponentName,

    /// How to join the data into the `RecordBatch`.
    //
    // TODO(cmc): remove once old `re_dataframe` is gone.
    pub join_encoding: JoinEncoding,
}

impl From<ComponentColumnDescriptor> for ComponentColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: desc.entity_path.clone(),
            component: desc.component_name,
            join_encoding: desc.join_encoding,
        }
    }
}

impl ComponentColumnSelector {
    /// Select a component of a given type, based on its  [`EntityPath`]
    #[inline]
    pub fn new<C: re_types_core::Component>(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component: C::name(),
            join_encoding: JoinEncoding::default(),
        }
    }

    /// Select a component based on its [`EntityPath`] and [`ComponentName`].
    #[inline]
    pub fn new_for_component_name(entity_path: EntityPath, component: ComponentName) -> Self {
        Self {
            entity_path,
            component,
            join_encoding: JoinEncoding::default(),
        }
    }

    /// Specify how the data should be joined into the `RecordBatch`.
    #[inline]
    pub fn with_join_encoding(mut self, join_encoding: JoinEncoding) -> Self {
        self.join_encoding = join_encoding;
        self
    }
}

impl std::fmt::Display for ComponentColumnSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            component,
            join_encoding: _,
        } = self;

        f.write_fmt(format_args!("{entity_path}@{}", component.short_name()))
    }
}

// TODO(jleibs): Add support for archetype-based column selection.
/*
/// Select a component based on its `Archetype` and field.
pub struct ArchetypeFieldColumnSelector {
    /// The path of the entity.
    entity_path: EntityPath,

    /// Name of the `Archetype` associated with this data.
    archetype: ArchetypeName,

    /// The field within the `Archetype` associated with this data.
    field: String,

    /// How to join the data into the `RecordBatch`.
    join_encoding: JoinEncoding,
}
*/

// --- Queries ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QueryExpression {
    LatestAt(LatestAtQueryExpression),
    Range(RangeQueryExpression),
}

impl From<LatestAtQueryExpression> for QueryExpression {
    #[inline]
    fn from(query: LatestAtQueryExpression) -> Self {
        Self::LatestAt(query)
    }
}

impl From<RangeQueryExpression> for QueryExpression {
    #[inline]
    fn from(query: RangeQueryExpression) -> Self {
        Self::Range(query)
    }
}

impl QueryExpression {
    #[inline]
    pub fn entity_path_filter(&self) -> &EntityPathFilter {
        match self {
            Self::LatestAt(query) => &query.entity_path_filter,
            Self::Range(query) => &query.entity_path_filter,
        }
    }
}

impl std::fmt::Display for QueryExpression {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LatestAt(query) => query.fmt(f),
            Self::Range(query) => query.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LatestAtQueryExpression {
    /// The entity path expression to query.
    ///
    /// Example: `world/camera/**`
    pub entity_path_filter: EntityPathFilter,

    /// The timeline to query.
    ///
    /// Example: `frame`.
    pub timeline: Timeline,

    /// The time at which to query.
    ///
    /// Example: `18`.
    pub at: TimeInt,
}

impl std::fmt::Display for LatestAtQueryExpression {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path_filter,
            timeline,
            at,
        } = self;

        f.write_fmt(format_args!(
            "latest state for '{}' at {} on {:?}",
            entity_path_filter.iter_expressions().join(", "),
            timeline.typ().format_utc(*at),
            timeline.name(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeQueryExpression {
    /// The entity path expression to query.
    ///
    /// Example: `world/camera/**`
    pub entity_path_filter: EntityPathFilter,

    /// The timeline to query.
    ///
    /// Example `frame`
    pub timeline: Timeline,

    /// The time range to query.
    pub time_range: ResolvedTimeRange,

    /// The point-of-view of the query, as described by its [`ComponentColumnDescriptor`].
    ///
    /// In a range query results, each non-null value of the point-of-view component column
    /// will generate a row in the result.
    ///
    /// Note that a component can be logged multiple times at the same timestamp (e.g. something
    /// happened multiple times during a single frame), in which case the results will contain
    /// multiple rows at a given timestamp.
    //
    // TODO(cmc): issue for multi-pov support
    pub pov: ComponentColumnSelector,
    //
    // TODO(cmc): custom join policy support
}

impl std::fmt::Display for RangeQueryExpression {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path_filter,
            timeline,
            time_range,
            pov,
        } = self;

        f.write_fmt(format_args!(
            "{} ranging {}..={} on {:?} as seen from {pov}",
            entity_path_filter.iter_expressions().join(", "),
            timeline.typ().format_utc(time_range.min()),
            timeline.typ().format_utc(time_range.max()),
            timeline.name(),
        ))
    }
}

// --- Queries v2 ---

/// Specifies how null values should be filled in the returned dataframe.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SparseFillStrategy {
    /// No sparse filling. Nulls stay nulls.
    #[default]
    None,

    /// Fill null values using global-scope latest-at semantics.
    ///
    /// The latest-at semantics are applied on the entire dataset as opposed to just the current
    /// view contents: it is possible to end up with values from outside the view!
    LatestAtGlobal,
    //
    // TODO(cmc): `LatestAtView`?
}

impl std::fmt::Display for SparseFillStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::LatestAtGlobal => f.write_str("latest-at (global)"),
        }
    }
}

/// The view contents specify which subset of the database (i.e., which columns) the query runs on,
/// expressed as a set of [`EntityPath`]s and their associated [`ComponentName`]s.
///
/// Setting an entity's components to `None` means: everything.
///
// TODO(cmc): we need to be able to build that easily in a command-line context, otherwise it's just
// very annoying. E.g. `--with /world/points:[rr.Position3D, rr.Radius] --with /cam:[rr.Pinhole]`.
pub type ViewContentsSelector = BTreeMap<EntityPath, Option<BTreeSet<ComponentName>>>;

// TODO(cmc): Ultimately, this shouldn't be hardcoded to `Timeline`, but to a generic `I: Index`.
//            `Index` in this case should also be implemented on tuples (`(I1, I2, ...)`).
pub type Index = Timeline;

// TODO(cmc): Ultimately, this shouldn't be hardcoded to `TimeInt`, but to a generic `I: Index`.
//            `Index` in this case should also be implemented on tuples (`(I1, I2, ...)`).
pub type IndexValue = TimeInt;

// TODO(cmc): Ultimately, this shouldn't be hardcoded to `ResolvedTimeRange`, but to a generic `I: Index`.
//            `Index` in this case should also be implemented on tuples (`(I1, I2, ...)`).
pub type IndexRange = ResolvedTimeRange;

/// Describes a complete query for Rerun's dataframe API.
///
/// ## Terminology: view vs. selection vs. filtering vs. sampling
///
/// * The view contents specify which subset of the database (i.e., which columns) the query runs on,
///   expressed as a set of [`EntityPath`]s and their associated [`ComponentName`]s.
///
/// * The filters filter out _rows_ of data from the view contents.
///   A filter cannot possibly introduce new rows, it can only remove existing ones from the view contents.
///
/// * The samplers sample _rows_ of data from the view contents at user-specified values.
///   Samplers don't necessarily return existing rows: they might introduce new ones if the sampled value
///   isn't present in the view contents in the first place.
///
/// * The selection applies last and samples _columns_ of data from the filtered/sampled view contents.
///   Selecting a column that isn't present in the view contents results in an empty column in the
///   final dataframe (null array).
///
/// A very rough mental model, in SQL terms:
/// ```text
/// SELECT <Self::selection> FROM <Self::view_contents> WHERE <Self::filtered_*>
/// ```
//
// TODO(cmc): ideally we'd like this to be the same type as the one used in the blueprint, possibly?
// TODO(cmc): Get rid of all re_dataframe (as opposed to re_dataframe2) stuff and rename this.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryExpression2 {
    /// The subset of the database that the query will run on: a set of [`EntityPath`]s and their
    /// associated [`ComponentName`]s.
    ///
    /// Defaults to `None`, which means: everything.
    ///
    /// Example (pseudo-code):
    /// ```text
    /// view_contents = {
    ///   "world/points": [rr.Position3D, rr.Radius],
    ///   "metrics": [rr.Scalar]
    /// }
    /// ```
    pub view_contents: Option<ViewContentsSelector>,

    /// The index used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 column contains non-null data at that index will be kept in the
    /// final dataset.
    ///
    /// Example: `Timeline("frame")`.
    //
    // TODO(cmc): this has to be a selector otherwise this is a horrible UX.
    pub filtered_index: Timeline,

    /// The range of index values used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 of the view-contents contains non-null data within that range will be kept in
    /// the final dataset.
    ///
    /// This is ignored if [`QueryExpression2::using_index_values`] is set.
    ///
    /// Example: `ResolvedTimeRange(10, 20)`.
    pub filtered_index_range: Option<IndexRange>,

    /// The specific index values used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 column contains non-null data at these specific values will be kept
    /// in the final dataset.
    ///
    /// This is ignored if [`QueryExpression2::using_index_values`] is set.
    ///
    /// Example: `[TimeInt(12), TimeInt(14)]`.
    pub filtered_index_values: Option<BTreeSet<IndexValue>>,

    /// The specific index values used to sample _rows_ from the view contents.
    ///
    /// The final dataset will contain one row per sampled index value, regardless of whether data
    /// existed for that index value in the view contents.
    /// The semantics of the query are consistent with all other settings: the results will be
    /// sorted on the `filtered_index`, and only contain unique index values.
    ///
    /// The order of the samples will be respected in the final result.
    ///
    /// If [`QueryExpression2::using_index_values`] is set, it overrides both [`QueryExpression2::filtered_index_range`]
    /// and [`QueryExpression2::filtered_index_values`].
    ///
    /// Example: `[TimeInt(12), TimeInt(14)]`.
    pub using_index_values: Option<BTreeSet<IndexValue>>,

    /// The component column used to filter out _rows_ from the view contents.
    ///
    /// Only rows where this column contains non-null data be kept in the final dataset.
    ///
    /// Example: `ComponentColumnSelector("rerun.components.Position3D")`.
    //
    // TODO(cmc): multi-pov support
    pub filtered_point_of_view: Option<ComponentColumnSelector>,

    /// Specifies how null values should be filled in the returned dataframe.
    ///
    /// Defaults to [`SparseFillStrategy::None`].
    pub sparse_fill_strategy: SparseFillStrategy,

    /// The specific _columns_ to sample from the final view contents.
    ///
    /// The order of the samples will be respected in the final result.
    ///
    /// Defaults to `None`, which means: everything.
    ///
    /// Example: `[ColumnSelector(Time("log_time")), ColumnSelector(Component("rerun.components.Position3D"))]`.
    //
    // TODO(cmc): the selection has to be on the QueryHandle, otherwise it's hell to use.
    pub selection: Option<Vec<ColumnSelector>>,
}

impl QueryExpression2 {
    #[inline]
    pub fn new(index: impl Into<Timeline>) -> Self {
        let index = index.into();

        Self {
            view_contents: None,
            filtered_index: index,
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_point_of_view: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            selection: None,
        }
    }
}

// ---

impl ChunkStore {
    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema(&self) -> Vec<ColumnDescriptor> {
        re_tracing::profile_function!();

        let timelines = self.all_timelines().into_iter().map(|timeline| {
            ColumnDescriptor::Time(TimeColumnDescriptor {
                timeline,
                datatype: timeline.datatype(),
            })
        });

        let static_components =
            self.static_chunk_ids_per_entity
                .iter()
                .flat_map(|(entity_path, per_component)| {
                    // TODO(#6889): Fill `archetype_name`/`archetype_field_name` (or whatever their
                    // final name ends up being) once we generate tags.
                    per_component.keys().filter_map(|component_name| {
                        self.lookup_datatype(component_name).map(|datatype| {
                            ColumnDescriptor::Component(ComponentColumnDescriptor {
                                entity_path: entity_path.clone(),
                                archetype_name: None,
                                archetype_field_name: None,
                                component_name: *component_name,
                                store_datatype: ArrowListArray::<i32>::default_datatype(
                                    datatype.clone(),
                                ),
                                join_encoding: JoinEncoding::default(),
                                is_static: true,
                            })
                        })
                    })
                });

        // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
        let temporal_components = self
            .temporal_chunk_ids_per_entity_per_component
            .iter()
            .flat_map(|(entity_path, per_timeline)| {
                per_timeline
                    .iter()
                    .map(move |(timeline, per_component)| (entity_path, timeline, per_component))
            })
            .flat_map(|(entity_path, _timeline, per_component)| {
                // TODO(#6889): Fill `archetype_name`/`archetype_field_name` (or whatever their
                // final name ends up being) once we generate tags.
                per_component.keys().filter_map(|component_name| {
                    self.lookup_datatype(component_name).map(|datatype| {
                        ColumnDescriptor::Component(ComponentColumnDescriptor {
                            entity_path: entity_path.clone(),
                            archetype_name: None,
                            archetype_field_name: None,
                            component_name: *component_name,
                            // NOTE: The data is always a at least a list, whether it's latest-at or range.
                            // It might be wrapped further in e.g. a dict, but at the very least
                            // it's a list.
                            store_datatype: ArrowListArray::<i32>::default_datatype(
                                datatype.clone(),
                            ),
                            join_encoding: JoinEncoding::default(),
                            // NOTE: This will make it so shadowed temporal data automatically gets
                            // discarded from the schema.
                            is_static: self
                                .static_chunk_ids_per_entity
                                .get(entity_path)
                                .map_or(false, |per_component| {
                                    per_component.contains_key(component_name)
                                }),
                        })
                    })
                })
            });

        let components = static_components
            .chain(temporal_components)
            .collect::<BTreeSet<_>>();

        timelines.chain(components).collect()
    }

    /// Given a [`TimeColumnSelector`], returns the corresponding [`TimeColumnDescriptor`].
    pub fn resolve_time_selector(&self, selector: &TimeColumnSelector) -> TimeColumnDescriptor {
        let timelines = self.all_timelines();

        let timeline = timelines
            .iter()
            .find(|timeline| timeline.name() == &selector.timeline)
            .copied()
            .unwrap_or_else(|| Timeline::new_temporal(selector.timeline));

        TimeColumnDescriptor {
            timeline,
            datatype: timeline.datatype(),
        }
    }

    /// Given a [`ComponentColumnSelector`], returns the corresponding [`ComponentColumnDescriptor`].
    ///
    /// If the component is not found in the store, a default descriptor is returned with a null datatype.
    pub fn resolve_component_selector(
        &self,
        selector: &ComponentColumnSelector,
    ) -> ComponentColumnDescriptor {
        let datatype = self
            .lookup_datatype(&selector.component)
            .cloned()
            .unwrap_or_else(|| ArrowDatatype::Null);

        let is_static = self
            .static_chunk_ids_per_entity
            .get(&selector.entity_path)
            .map_or(false, |per_component| {
                per_component.contains_key(&selector.component)
            });

        // TODO(#6889): Fill `archetype_name`/`archetype_field_name` (or whatever their
        // final name ends up being) once we generate tags.
        ComponentColumnDescriptor {
            entity_path: selector.entity_path.clone(),
            archetype_name: None,
            archetype_field_name: None,
            component_name: selector.component,
            store_datatype: ArrowListArray::<i32>::default_datatype(datatype.clone()),
            join_encoding: selector.join_encoding,
            is_static,
        }
    }

    /// Given a set of [`ColumnSelector`]s, returns the corresponding [`ColumnDescriptor`]s.
    pub fn resolve_selectors(
        &self,
        selectors: impl IntoIterator<Item = impl Into<ColumnSelector>>,
    ) -> Vec<ColumnDescriptor> {
        // TODO(jleibs): When, if ever, should this return an error?
        selectors
            .into_iter()
            .map(|selector| {
                let selector = selector.into();
                match selector {
                    ColumnSelector::Time(selector) => {
                        ColumnDescriptor::Time(self.resolve_time_selector(&selector))
                    }

                    ColumnSelector::Component(selector) => {
                        ColumnDescriptor::Component(self.resolve_component_selector(&selector))
                    }
                }
            })
            .collect()
    }

    /// Returns the filtered schema for the given query expression.
    ///
    /// This will only include columns which may contain non-empty values from the perspective of
    /// the query semantics.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    ///
    /// This does not run a full-blown query, but rather just inspects `Chunk`-level metadata,
    /// which can lead to false positives, but makes this very cheap to compute.
    pub fn schema_for_query(&self, query: &QueryExpression) -> Vec<ColumnDescriptor> {
        re_tracing::profile_function!(format!("{query:?}"));

        // First, grab the full schema and filters out every entity path that isn't covered by the query.
        let schema = self
            .schema()
            .into_iter()
            .filter(|descr| {
                descr.entity_path().map_or(true, |entity_path| {
                    query.entity_path_filter().matches(entity_path)
                })
            })
            .collect_vec();

        // Then, discard any column descriptor which cannot possibly have data for the given query.
        //
        // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
        // TODO(jleibs): This filtering actually seems incorrect. This operation should be based solely
        // on the timeline,
        let mut filtered_out = HashSet::default();
        for column_descr in &schema {
            let ColumnDescriptor::Component(descr) = column_descr else {
                continue;
            };

            match query {
                QueryExpression::LatestAt(query) => {
                    let q = LatestAtQuery::new(query.timeline, query.at);
                    if self
                        .latest_at_relevant_chunks(&q, &descr.entity_path, descr.component_name)
                        .is_empty()
                    {
                        filtered_out.insert(column_descr.clone());
                    }
                }

                QueryExpression::Range(query) => {
                    let q = LatestAtQuery::new(query.timeline, query.time_range.max());
                    if self
                        .latest_at_relevant_chunks(&q, &descr.entity_path, descr.component_name)
                        .is_empty()
                    {
                        filtered_out.insert(column_descr.clone());
                    }
                }
            }
        }

        schema
            .into_iter()
            .filter(|descr| !filtered_out.contains(descr))
            .collect()
    }

    /// Returns the filtered schema for the given [`ViewContentsSelector`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema_for_view_contents(
        &self,
        view_contents: &ViewContentsSelector,
    ) -> Vec<ColumnDescriptor> {
        re_tracing::profile_function!();

        self.schema()
            .into_iter()
            .filter(|column| match column {
                ColumnDescriptor::Time(_) => true,
                ColumnDescriptor::Component(column) => view_contents
                    .get(&column.entity_path)
                    .map_or(false, |components| {
                        components.as_ref().map_or(true, |components| {
                            components.contains(&column.component_name)
                        })
                    }),
            })
            .collect()
    }
}
