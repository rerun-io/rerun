//! All the APIs used specifically for `re_dataframe`.

use std::collections::BTreeSet;

use ahash::HashSet;
use arrow2::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use itertools::Itertools as _;

use re_chunk::LatestAtQuery;
use re_log_types::ResolvedTimeRange;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::{ArchetypeName, ComponentName, Loggable as _};

use crate::ChunkStore;

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// --- Descriptors ---

// TODO(#6889): At some point all these descriptors needs to be interned and have handles or
// something. And of course they need to be codegen. But we'll get there once we're back to
// natively tagged components.

// Describes any kind of column.
//
// See:
// * [`ControlColumnDescriptor`]
// * [`TimeColumnDescriptor`]
// * [`ComponentColumnDescriptor`]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColumnDescriptor {
    Control(ControlColumnDescriptor),
    Time(TimeColumnDescriptor),
    Component(ComponentColumnDescriptor),
}

impl ColumnDescriptor {
    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::Component(descr) => Some(&descr.entity_path),
            Self::Control(_) | Self::Time(_) => None,
        }
    }

    #[inline]
    pub fn datatype(&self) -> &ArrowDatatype {
        match self {
            Self::Control(descr) => &descr.datatype,
            Self::Component(descr) => &descr.datatype,
            Self::Time(descr) => &descr.datatype,
        }
    }

    #[inline]
    pub fn to_arrow_field(&self, datatype: Option<ArrowDatatype>) -> ArrowField {
        match self {
            Self::Control(descr) => descr.to_arrow_field(),
            Self::Time(descr) => descr.to_arrow_field(),
            Self::Component(descr) => descr.to_arrow_field(datatype),
        }
    }

    #[inline]
    pub fn short_name(&self) -> String {
        match self {
            Self::Control(descr) => descr.component_name.short_name().to_owned(),
            Self::Time(descr) => descr.timeline.name().to_string(),
            Self::Component(descr) => descr.component_name.short_name().to_owned(),
        }
    }
}

/// Describes a column used to control Rerun's behavior, such as `RowId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlColumnDescriptor {
    /// Semantic name associated with this data.
    ///
    /// Example: `rerun.controls.RowId`.
    pub component_name: ComponentName,

    /// The Arrow datatype of the column.
    pub datatype: ArrowDatatype,
}

impl PartialOrd for ControlColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ControlColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            component_name,
            datatype: _,
        } = self;
        component_name.cmp(&other.component_name)
    }
}

impl ControlColumnDescriptor {
    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self {
            component_name,
            datatype,
        } = self;

        ArrowField::new(
            component_name.to_string(),
            datatype.clone(),
            false, /* nullable */
        )
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
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self { timeline, datatype } = self;
        ArrowField::new(
            timeline.name().to_string(),
            datatype.clone(),
            false, /* nullable */
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

    /// The Arrow datatype of the column.
    pub datatype: ArrowDatatype,

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
            datatype: _,
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
            datatype: _,
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
        Self {
            entity_path,
            archetype_name: None,
            archetype_field_name: None,
            component_name: C::name(),
            datatype: C::arrow_datatype(),
            // TODO(cmc): one of the many reasons why using `ComponentColumnDescriptor` for this
            // gets a bit weird… Good enough for now though.
            is_static: false,
        }
    }

    #[inline]
    pub fn to_arrow_field(&self, wrapped_datatype: Option<ArrowDatatype>) -> ArrowField {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
            component_name,
            datatype,
            is_static,
        } = self;

        // NOTE: Only the system doing the actual packing knows the final datatype with all of
        // its wrappers (is it a component array? is it a list? is it a dict?).
        let datatype = wrapped_datatype.unwrap_or_else(|| datatype.clone());

        // TODO(cmc): figure out who's in charge of adding the outer list layer.
        ArrowField::new(
            component_name.short_name().to_owned(),
            datatype,
            false, /* nullable */
        )
        // TODO(#6889): This needs some proper sorbetization -- I just threw these names randomly.
        .with_metadata(
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
            .collect(),
        )
    }
}

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
    pub fn entity_path_expr(&self) -> &EntityPathExpression {
        match self {
            Self::LatestAt(query) => &query.entity_path_expr,
            Self::Range(query) => &query.entity_path_expr,
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
    pub entity_path_expr: EntityPathExpression,

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
            entity_path_expr,
            timeline,
            at,
        } = self;

        f.write_fmt(format_args!(
            "latest state for '{entity_path_expr}' at {} on {:?}",
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
    pub entity_path_expr: EntityPathExpression,

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
    pub pov: ComponentColumnDescriptor,
    //
    // TODO(cmc): custom join policy support
}

impl std::fmt::Display for RangeQueryExpression {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path_expr,
            timeline,
            time_range,
            pov,
        } = self;

        f.write_fmt(format_args!(
            "{entity_path_expr} ranging {}..={} on {:?} as seen from {pov}",
            timeline.typ().format_utc(time_range.min()),
            timeline.typ().format_utc(time_range.max()),
            timeline.name(),
        ))
    }
}

/// An expression to select one or more entities to query.
///
/// Example: `world/camera/**`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntityPathExpression {
    pub path: EntityPath,

    /// If true, ALSO include children and grandchildren of this path (recursive rule).
    pub include_subtree: bool,
}

impl std::fmt::Display for EntityPathExpression {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            path,
            include_subtree,
        } = self;

        f.write_fmt(format_args!(
            "{path}{}{}",
            if path.is_root() { "" } else { "/" },
            if *include_subtree { "**" } else { "" }
        ))
    }
}

impl EntityPathExpression {
    #[inline]
    pub fn matches(&self, path: &EntityPath) -> bool {
        if self.include_subtree {
            path.starts_with(&self.path)
        } else {
            path == &self.path
        }
    }
}

impl<S: AsRef<str>> From<S> for EntityPathExpression {
    #[inline]
    fn from(s: S) -> Self {
        let s = s.as_ref();
        if s == "/**" {
            Self {
                path: EntityPath::root(),
                include_subtree: true,
            }
        } else if let Some(path) = s.strip_suffix("/**") {
            Self {
                path: EntityPath::parse_forgiving(path),
                include_subtree: true,
            }
        } else {
            Self {
                path: EntityPath::parse_forgiving(s),
                include_subtree: false,
            }
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

        let controls = std::iter::once(ColumnDescriptor::Control(ControlColumnDescriptor {
            component_name: RowId::name(),
            datatype: RowId::arrow_datatype(),
        }));

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
                                datatype: datatype.clone(),
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
                            datatype: datatype.clone(),
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

        controls.chain(timelines).chain(components).collect()
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
                    query.entity_path_expr().matches(entity_path)
                })
            })
            .collect_vec();

        // Then, discard any column descriptor which cannot possibly have data for the given query.
        //
        // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
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
}
