//! All the APIs used specifically for `re_dataframe`.

use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
};

use arrow::{
    array::ListArray as ArrowListArray,
    datatypes::{DataType as ArrowDatatype, Field as ArrowField},
};
use itertools::Itertools;

use re_chunk::TimelineName;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, Timeline};
use re_sorbet::{
    ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor, SorbetColumnDescriptors,
};
use re_types_core::ComponentName;
use tap::Tap as _;

use crate::{ChunkStore, ColumnMetadata};

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

impl From<TimelineName> for TimeColumnSelector {
    #[inline]
    fn from(timeline: TimelineName) -> Self {
        Self { timeline }
    }
}

impl From<Timeline> for TimeColumnSelector {
    #[inline]
    fn from(timeline: Timeline) -> Self {
        Self {
            timeline: *timeline.name(),
        }
    }
}

impl From<&str> for TimeColumnSelector {
    #[inline]
    fn from(timeline: &str) -> Self {
        Self {
            timeline: timeline.into(),
        }
    }
}

impl From<String> for TimeColumnSelector {
    #[inline]
    fn from(timeline: String) -> Self {
        Self {
            timeline: timeline.into(),
        }
    }
}

impl From<IndexColumnDescriptor> for TimeColumnSelector {
    #[inline]
    fn from(desc: IndexColumnDescriptor) -> Self {
        Self {
            timeline: desc.timeline_name(),
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
    ///
    /// This string will be flexibly matched against the available component names.
    /// Valid matches are case invariant matches of either the full name or the short name.
    pub component_name: String,
}

impl From<ComponentColumnDescriptor> for ComponentColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: desc.entity_path.clone(),
            component_name: desc.component_name.short_name().to_owned(),
        }
    }
}

impl ComponentColumnSelector {
    /// Select a component of a given type, based on its  [`EntityPath`]
    #[inline]
    pub fn new<C: re_types_core::Component>(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component_name: C::name().short_name().to_owned(),
        }
    }

    /// Select a component based on its [`EntityPath`] and [`ComponentName`].
    #[inline]
    pub fn new_for_component_name(entity_path: EntityPath, component_name: ComponentName) -> Self {
        Self {
            entity_path,
            component_name: component_name.short_name().to_owned(),
        }
    }
}

impl std::fmt::Display for ComponentColumnSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            component_name,
        } = self;

        f.write_fmt(format_args!("{entity_path}:{component_name}"))
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
}
*/

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
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewContentsSelector(pub BTreeMap<EntityPath, Option<BTreeSet<ComponentName>>>);

impl ViewContentsSelector {
    pub fn into_inner(self) -> BTreeMap<EntityPath, Option<BTreeSet<ComponentName>>> {
        self.0
    }
}

impl Deref for ViewContentsSelector {
    type Target = BTreeMap<EntityPath, Option<BTreeSet<ComponentName>>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ViewContentsSelector {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<(EntityPath, Option<BTreeSet<ComponentName>>)> for ViewContentsSelector {
    fn from_iter<T: IntoIterator<Item = (EntityPath, Option<BTreeSet<ComponentName>>)>>(
        iter: T,
    ) -> Self {
        Self(iter.into_iter().collect())
    }
}

// TODO(cmc): Ultimately, this shouldn't be hardcoded to `Timeline`, but to a generic `I: Index`.
//            `Index` in this case should also be implemented on tuples (`(I1, I2, ...)`).
pub type Index = TimelineName;

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
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryExpression {
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

    /// Whether the `view_contents` should ignore semantically empty columns.
    ///
    /// A semantically empty column is a column that either contains no data at all, or where all
    /// values are either nulls or empty arrays (`[]`).
    ///
    /// `view_contents`: [`QueryExpression::view_contents`]
    pub include_semantically_empty_columns: bool,

    /// Whether the `view_contents` should ignore columns corresponding to indicator components.
    ///
    /// Indicator components are marker components, generally automatically inserted by Rerun, that
    /// helps keep track of the original context in which a piece of data was logged/sent.
    ///
    /// `view_contents`: [`QueryExpression::view_contents`]
    pub include_indicator_columns: bool,

    /// Whether the `view_contents` should ignore columns corresponding to `Clear`-related components.
    ///
    /// `view_contents`: [`QueryExpression::view_contents`]
    /// `Clear`: [`re_types_core::archetypes::Clear`]
    pub include_tombstone_columns: bool,

    /// The index used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 column contains non-null data at that index will be kept in the
    /// final dataset.
    ///
    /// If left unspecified, the results will only contain static data.
    ///
    /// Examples: `Some(TimelineName("frame"))`, `None` (only static data).
    //
    // TODO(cmc): this has to be a selector otherwise this is a horrible UX.
    pub filtered_index: Option<Index>,

    /// The range of index values used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 of the view-contents contains non-null data within that range will be kept in
    /// the final dataset.
    ///
    /// * This has no effect if `filtered_index` isn't set.
    /// * This has no effect if [`QueryExpression::using_index_values`] is set.
    ///
    /// Example: `ResolvedTimeRange(10, 20)`.
    pub filtered_index_range: Option<IndexRange>,

    /// The specific index values used to filter out _rows_ from the view contents.
    ///
    /// Only rows where at least 1 column contains non-null data at these specific values will be kept
    /// in the final dataset.
    ///
    /// * This has no effect if `filtered_index` isn't set.
    /// * This has no effect if [`QueryExpression::using_index_values`] is set.
    /// * Using [`TimeInt::STATIC`] as index value has no effect.
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
    /// * This has no effect if `filtered_index` isn't set.
    /// * If set, this overrides both [`QueryExpression::filtered_index_range`] and
    ///   [`QueryExpression::filtered_index_values`].
    /// * Using [`TimeInt::STATIC`] as index value has no effect.
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
    pub filtered_is_not_null: Option<ComponentColumnSelector>,

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

// ---

impl ChunkStore {
    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema(&self) -> SorbetColumnDescriptors {
        re_tracing::profile_function!();

        let indices = self
            .timelines()
            .values()
            .map(|timeline| IndexColumnDescriptor::from(*timeline))
            .collect();

        let components = self
            .per_column_metadata
            .iter()
            .flat_map(|(entity_path, per_name)| {
                per_name.values().flat_map(move |per_descr| {
                    per_descr.keys().map(move |descr| (entity_path, descr))
                })
            })
            .filter_map(|(entity_path, component_descr)| {
                let metadata =
                    self.lookup_column_metadata(entity_path, &component_descr.component_name)?;
                let datatype = self.lookup_datatype(&component_descr.component_name)?;

                Some(((entity_path, component_descr), (metadata, datatype)))
            })
            .map(|((entity_path, component_descr), (metadata, datatype))| {
                let ColumnMetadata {
                    is_static,
                    is_indicator,
                    is_tombstone,
                    is_semantically_empty,
                } = metadata;

                component_descr.component_name.sanity_check();

                ComponentColumnDescriptor {
                    // NOTE: The data is always a at least a list, whether it's latest-at or range.
                    // It might be wrapped further in e.g. a dict, but at the very least
                    // it's a list.
                    store_datatype: ArrowListArray::DATA_TYPE_CONSTRUCTOR(
                        ArrowField::new("item", datatype.clone(), true).into(),
                    ),

                    entity_path: entity_path.clone(),
                    archetype_name: component_descr.archetype_name,
                    archetype_field_name: component_descr.archetype_field_name,
                    component_name: component_descr.component_name,
                    is_static,
                    is_indicator,
                    is_tombstone,
                    is_semantically_empty,
                }
            })
            .collect_vec()
            .tap_mut(|components| components.sort());

        SorbetColumnDescriptors {
            row_id: Some(re_sorbet::RowIdColumnDescriptor { is_sorted: false }),
            indices,
            components,
        }
        .tap(|schema| schema.sanity_check())
    }

    /// Given a [`TimeColumnSelector`], returns the corresponding [`IndexColumnDescriptor`].
    pub fn resolve_time_selector(&self, selector: &TimeColumnSelector) -> IndexColumnDescriptor {
        let timelines = self.timelines();

        let timeline = timelines
            .get(&selector.timeline)
            .copied()
            .unwrap_or_else(|| Timeline::new_temporal(selector.timeline));

        IndexColumnDescriptor::from(timeline)
    }

    /// Given a [`ComponentColumnSelector`], returns the corresponding [`ComponentColumnDescriptor`].
    ///
    /// If the component is not found in the store, a default descriptor is returned with a null datatype.
    pub fn resolve_component_selector(
        &self,
        selector: &ComponentColumnSelector,
    ) -> ComponentColumnDescriptor {
        // Happy path if this string is a valid component
        // TODO(#7699) This currently interns every string ever queried which could be wasteful, especially
        // in long-running servers. In practice this probably doesn't matter.
        let selected_component_name = ComponentName::from(selector.component_name.clone());

        let column_info = self
            .per_column_metadata
            .get(&selector.entity_path)
            .and_then(|per_name| {
                per_name.get(&selected_component_name).or_else(|| {
                    per_name.iter().find_map(|(component_name, per_descr)| {
                        component_name
                            .matches(&selector.component_name)
                            .then_some(per_descr)
                    })
                })
            })
            .and_then(|per_descr| per_descr.iter().next());

        let component_descr = column_info.map(|(descr, _metadata)| descr);
        let _column_metadata = column_info.map(|(_descr, metadata)| metadata).cloned();

        let component_name =
            component_descr.map_or(selected_component_name, |descr| descr.component_name);

        component_name.sanity_check();

        let ColumnMetadata {
            is_static,
            is_indicator,
            is_tombstone,
            is_semantically_empty,
        } = self
            .lookup_column_metadata(&selector.entity_path, &component_name)
            .unwrap_or(ColumnMetadata {
                is_static: false,
                is_indicator: false,
                is_tombstone: false,
                is_semantically_empty: false,
            });

        let datatype = self
            .lookup_datatype(&component_name)
            .unwrap_or(ArrowDatatype::Null);

        ComponentColumnDescriptor {
            entity_path: selector.entity_path.clone(),
            archetype_name: component_descr.and_then(|descr| descr.archetype_name),
            archetype_field_name: component_descr.and_then(|descr| descr.archetype_field_name),
            component_name,
            store_datatype: ArrowListArray::DATA_TYPE_CONSTRUCTOR(
                ArrowField::new("item", datatype, true).into(),
            ),
            is_static,
            is_indicator,
            is_tombstone,
            is_semantically_empty,
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

    /// Returns the filtered schema for the given [`QueryExpression`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema_for_query(&self, query: &QueryExpression) -> SorbetColumnDescriptors {
        re_tracing::profile_function!();

        let QueryExpression {
            view_contents,
            include_semantically_empty_columns,
            include_indicator_columns,
            include_tombstone_columns,
            filtered_index: _,
            filtered_index_range: _,
            filtered_index_values: _,
            using_index_values: _,
            filtered_is_not_null: _,
            sparse_fill_strategy: _,
            selection: _,
        } = query;

        let filter = |column: &ComponentColumnDescriptor| {
            let is_part_of_view_contents = || {
                view_contents.as_ref().map_or(true, |view_contents| {
                    view_contents
                        .get(&column.entity_path)
                        .is_some_and(|components| {
                            components.as_ref().map_or(true, |components| {
                                components.contains(&column.component_name)
                            })
                        })
                })
            };

            let passes_semantically_empty_check =
                || *include_semantically_empty_columns || !column.is_semantically_empty;

            let passes_indicator_check = || *include_indicator_columns || !column.is_indicator;

            let passes_tombstone_check = || *include_tombstone_columns || !column.is_tombstone;

            is_part_of_view_contents()
                && passes_semantically_empty_check()
                && passes_indicator_check()
                && passes_tombstone_check()
        };

        self.schema().filter_components(filter)
    }
}
