//! All the APIs used specifically for `re_dataframe`.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};

use arrow::array::ListArray as ArrowListArray;
use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use itertools::Itertools as _;
use re_chunk::{ComponentIdentifier, LatestAtQuery, RangeQuery, TimelineName};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, Timeline};
use re_sorbet::{
    ChunkColumnDescriptors, ColumnSelector, ComponentColumnDescriptor, ComponentColumnSelector,
    IndexColumnDescriptor, TimeColumnSelector,
};
use tap::Tap as _;

use crate::{ChunkStore, ColumnMetadata};

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

/// The view contents specify which subset of the database (i.e., which columns) the query runs on.
///
/// Contents are expressed as a set of [`EntityPath`]s and their associated [`re_types_core::ComponentIdentifier`]s.
///
/// Setting an entity's identifier to `None` means: everything.
///
// TODO(cmc): we need to be able to build that easily in a command-line context, otherwise it's just
// very annoying. E.g. `--with /world/points:[positions, radius] --with /cam:[pinhole]`.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewContentsSelector(pub BTreeMap<EntityPath, Option<BTreeSet<ComponentIdentifier>>>);

impl ViewContentsSelector {
    pub fn into_inner(self) -> BTreeMap<EntityPath, Option<BTreeSet<ComponentIdentifier>>> {
        self.0
    }
}

impl Deref for ViewContentsSelector {
    type Target = BTreeMap<EntityPath, Option<BTreeSet<ComponentIdentifier>>>;

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

impl FromIterator<(EntityPath, Option<BTreeSet<ComponentIdentifier>>)> for ViewContentsSelector {
    fn from_iter<T: IntoIterator<Item = (EntityPath, Option<BTreeSet<ComponentIdentifier>>)>>(
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

// TODO(cmc): Ultimately, this shouldn't be hardcoded to `AbsoluteTimeRange`, but to a generic `I: Index`.
//            `Index` in this case should also be implemented on tuples (`(I1, I2, ...)`).
pub type IndexRange = AbsoluteTimeRange;

/// Specifies whether static columns should be included in the query.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum StaticColumnSelection {
    /// Both static and non-static columns should be included in the query.
    #[default]
    Both,

    /// Only static columns should be included in the query.
    StaticOnly,

    /// Only non-static columns should be included in the query.
    NonStaticOnly,
}

/// Describes a complete query for Rerun's dataframe API.
///
/// ## Terminology: view vs. selection vs. filtering vs. sampling
///
/// * The view contents specify which subset of the database (i.e., which columns) the query runs on,
///   expressed as a set of [`EntityPath`]s and their associated [`re_types_core::ComponentIdentifier`]s.
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
    /// associated [`re_types_core::ComponentIdentifier`]s.
    ///
    /// Defaults to `None`, which means: everything.
    ///
    /// Example (pseudo-code):
    /// ```text
    /// view_contents = {
    ///   "world/points": [rr.Position3D, rr.Radius],
    ///   "metrics": [rr.Scalars]
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

    /// Whether the `view_contents` should ignore columns corresponding to `Clear`-related components.
    ///
    /// `view_contents`: [`QueryExpression::view_contents`]
    /// `Clear`: [`re_types_core::archetypes::Clear`]
    pub include_tombstone_columns: bool,

    /// Whether the `view_contents` should include static columns.
    ///
    /// `view_contents`: [`QueryExpression::view_contents`]
    pub include_static_columns: StaticColumnSelection,

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
    /// Example: `AbsoluteTimeRange(10, 20)`.
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
    /// Example: `ComponentColumnSelector("Points3D:positions")`.
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
    /// Example: `[ColumnSelector(Time("log_time")), ColumnSelector(Component("Points3D:position"))]`.
    //
    // TODO(cmc): the selection has to be on the QueryHandle, otherwise it's hell to use.
    pub selection: Option<Vec<ColumnSelector>>,
}

impl QueryExpression {
    pub fn is_static(&self) -> bool {
        self.filtered_index.is_none()
    }

    pub fn min_latest_at(&self) -> Option<LatestAtQuery> {
        let index = self.filtered_index?;

        if let Some(using_index_values) = &self.using_index_values {
            return Some(LatestAtQuery::new(
                index,
                using_index_values.first().copied()?,
            ));
        }

        if let Some(filtered_index_values) = &self.filtered_index_values {
            return Some(LatestAtQuery::new(
                index,
                filtered_index_values.first().copied()?,
            ));
        }

        if let Some(filtered_index_range) = &self.filtered_index_range {
            return Some(LatestAtQuery::new(index, filtered_index_range.min()));
        }

        None
    }

    pub fn max_range(&self) -> Option<RangeQuery> {
        let index = self.filtered_index?;

        if let Some(using_index_values) = &self.using_index_values {
            return Some(RangeQuery::new(
                index,
                AbsoluteTimeRange::new(
                    using_index_values.first().copied()?,
                    using_index_values.last().copied()?,
                ),
            ));
        }

        if let Some(filtered_index_values) = &self.filtered_index_values {
            return Some(RangeQuery::new(
                index,
                AbsoluteTimeRange::new(
                    filtered_index_values.first().copied()?,
                    filtered_index_values.last().copied()?,
                ),
            ));
        }

        if let Some(filtered_index_range) = &self.filtered_index_range {
            return Some(RangeQuery::new(index, *filtered_index_range));
        }

        None
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
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema(&self) -> ChunkColumnDescriptors {
        re_tracing::profile_function!();

        let indices = self
            .timelines()
            .values()
            .map(|timeline| IndexColumnDescriptor::from(*timeline))
            .collect();

        let components = self
            .per_column_metadata
            .iter()
            .flat_map(|(entity_path, per_identifier)| {
                per_identifier
                    .values()
                    .map(move |(descr, _, datatype)| (entity_path, descr, datatype))
            })
            .filter_map(|(entity_path, component_descr, datatype)| {
                let metadata =
                    self.lookup_column_metadata(entity_path, component_descr.component)?;

                Some(((entity_path, component_descr), (metadata, datatype)))
            })
            .map(|((entity_path, component_descr), (metadata, datatype))| {
                let ColumnMetadata {
                    is_static,
                    is_tombstone,
                    is_semantically_empty,
                } = metadata;

                if let Some(c) = component_descr.component_type {
                    c.sanity_check();
                }

                ComponentColumnDescriptor {
                    // NOTE: The data is always a at least a list, whether it's latest-at or range.
                    // It might be wrapped further in e.g. a dict, but at the very least
                    // it's a list.
                    store_datatype: ArrowListArray::DATA_TYPE_CONSTRUCTOR(
                        ArrowField::new("item", datatype.clone(), true).into(),
                    ),

                    entity_path: entity_path.clone(),
                    archetype: component_descr.archetype,
                    component: component_descr.component,
                    component_type: component_descr.component_type,
                    is_static,
                    is_tombstone,
                    is_semantically_empty,
                }
            })
            .collect_vec()
            .tap_mut(|components| components.sort());

        ChunkColumnDescriptors {
            row_id: self.row_id_descriptor(),
            indices,
            components,
        }
        .tap(|schema| schema.sanity_check())
    }

    #[expect(clippy::unused_self)]
    pub fn row_id_descriptor(&self) -> re_sorbet::RowIdColumnDescriptor {
        re_sorbet::RowIdColumnDescriptor::from_sorted(false)
    }

    /// Given a [`TimeColumnSelector`], returns the corresponding [`IndexColumnDescriptor`].
    pub fn resolve_time_selector(&self, selector: &TimeColumnSelector) -> IndexColumnDescriptor {
        let timelines = self.timelines();

        let timeline = timelines
            .get(&selector.timeline)
            .copied()
            .unwrap_or_else(|| {
                re_log::warn_once!("Unknown timeline {selector:?}; assuming sequence timeline.");
                Timeline::new_sequence(selector.timeline)
            });

        IndexColumnDescriptor::from(timeline)
    }

    /// Given a [`ComponentColumnSelector`], returns the corresponding [`ComponentColumnDescriptor`].
    ///
    /// If the component is not found in the store, a default descriptor is returned with a null datatype.
    pub fn resolve_component_selector(
        &self,
        selector: &ComponentColumnSelector,
    ) -> ComponentColumnDescriptor {
        // Unfortunately, we can't return an error here, so we craft a default descriptor and
        // add information to it that we find.

        // TODO(#7699) This currently interns every string ever queried which could be wasteful, especially
        // in long-running servers. In practice this probably doesn't matter.
        let mut result = ComponentColumnDescriptor {
            store_datatype: ArrowDatatype::Null,
            component_type: None,
            entity_path: selector.entity_path.clone(),
            archetype: None,
            component: selector.component.as_str().into(),
            is_static: false,
            is_tombstone: false,
            is_semantically_empty: false,
        };

        let Some(per_identifier) = self.per_column_metadata.get(&selector.entity_path) else {
            return result;
        };

        // We perform a scan over all component descriptors in the queried entity path.
        let Some((component_descr, _, datatype)) =
            per_identifier.get(&selector.component.as_str().into())
        else {
            return result;
        };
        result.store_datatype = datatype.clone();
        result.archetype = component_descr.archetype;
        result.component_type = component_descr.component_type;

        if let Some(ColumnMetadata {
            is_static,
            is_tombstone,
            is_semantically_empty,
        }) = self.lookup_column_metadata(&selector.entity_path, component_descr.component)
        {
            result.is_static = is_static;
            result.is_tombstone = is_tombstone;
            result.is_semantically_empty = is_semantically_empty;
        }

        result
    }

    /// Returns the filtered schema for the given [`QueryExpression`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    pub fn schema_for_query(&self, query: &QueryExpression) -> ChunkColumnDescriptors {
        re_tracing::profile_function!();

        let filter = Self::create_component_filter_from_query(query);

        self.schema().filter_components(filter)
    }

    pub fn create_component_filter_from_query(
        query: &QueryExpression,
    ) -> impl Fn(&ComponentColumnDescriptor) -> bool {
        let QueryExpression {
            view_contents,
            include_semantically_empty_columns,
            include_tombstone_columns,
            include_static_columns,
            filtered_index: _,
            filtered_index_range: _,
            filtered_index_values: _,
            using_index_values: _,
            filtered_is_not_null: _,
            sparse_fill_strategy: _,
            selection: _,
        } = query;

        move |column: &ComponentColumnDescriptor| {
            let is_part_of_view_contents = || {
                view_contents.as_ref().is_none_or(|view_contents| {
                    view_contents
                        .get(&column.entity_path)
                        .is_some_and(|components| {
                            components
                                .as_ref()
                                .is_none_or(|components| components.contains(&column.component))
                        })
                })
            };

            let passes_semantically_empty_check =
                || *include_semantically_empty_columns || !column.is_semantically_empty;

            let passes_tombstone_check = || *include_tombstone_columns || !column.is_tombstone;

            let passes_static_check = || match include_static_columns {
                StaticColumnSelection::Both => true,
                StaticColumnSelection::StaticOnly => column.is_static,
                StaticColumnSelection::NonStaticOnly => !column.is_static,
            };

            is_part_of_view_contents()
                && passes_semantically_empty_check()
                && passes_tombstone_check()
                && passes_static_check()
        }
    }
}
