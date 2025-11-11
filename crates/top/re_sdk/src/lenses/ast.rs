//! Private module with the AST-like definitions of lenses.
//!
//! **Note**: Apart from high-level entry points (like [`Op`] and [`Lens`],
//! we should not leak these elements into the public API. This allows us to
//! evolve the definition of lenses over time, if requirements change.

use std::sync::Arc;

use arrow::{
    array::{AsArray as _, Int64Array, ListArray},
    compute::take,
    datatypes::{DataType, Field},
};
use nohash_hasher::IntMap;

use re_arrow_combinators::{
    Transform as _,
    reshape::{Explode, Flatten},
};
use re_chunk::{
    ArrowArray as _, Chunk, ChunkId, ComponentIdentifier, EntityPath, TimeColumn, Timeline,
    TimelineName,
};
use re_log_types::{EntityPathFilter, TimeType};
use re_types::{ComponentDescriptor, SerializedComponentColumn};

use super::{Error, builder::LensBuilder, op};

pub struct InputColumn {
    pub entity_path_filter: EntityPathFilter,
    pub component: ComponentIdentifier,
}

/// Target entity path for lens outputs.
#[derive(Debug, Clone, Default)]
pub enum TargetEntity {
    /// Use the matched input entity path.
    #[default]
    SameAsInput,

    /// Use a specific entity path.
    Explicit(EntityPath),
}

/// A component output.
///
/// Depending on the context in which this output is used, the result from
/// applying the `ops` should be a list array (1:1) or a list array of list arrays (1:N).
#[derive(Debug)]
pub struct ComponentOutput {
    pub component_descr: ComponentDescriptor,
    pub ops: Vec<Op>,
}

/// A time extraction output.
#[derive(Debug)]
pub struct TimeOutput {
    pub timeline_name: TimelineName,
    pub timeline_type: TimeType,
    pub ops: Vec<Op>,
}

/// Determines how a lens transforms input rows to output rows.
#[derive(Debug)]
pub enum LensKind {
    /// Each input row produces exactly one output row (1:1 mapping).
    ///
    /// Outputs inherit timelines from the input chunk.
    OneToOne {
        target_entity: TargetEntity,
        components: Vec<ComponentOutput>,
        timelines: Vec<TimeOutput>,
    },

    /// Each input row produces multiple output rows (1:N flat-map).
    ///
    /// Outputs are always temporal.
    ToMany {
        target_entity: TargetEntity,
        components: Vec<ComponentOutput>,
        timelines: Vec<TimeOutput>,
    },

    /// Static lens: outputs have no timelines (timeless data).
    ///
    /// In many cases, static lenses will omit the input column entirely.
    Static {
        target_entity: TargetEntity,
        components: Vec<ComponentOutput>,
    },
}

type CustomFn = Box<dyn Fn(&ListArray) -> Result<ListArray, Error> + Sync + Send>;

/// Provides commonly used transformations of component columns.
///
/// Individual operations are wrapped to hide their implementation details.
pub enum Op {
    /// Extracts a specific field from a `StructArray`.
    AccessField(op::AccessField),

    /// Efficiently casts a component to a new `DataType`.
    Cast(op::Cast),

    /// Flattens a list array inside a component.
    ///
    /// Takes `List<List<T>>` and flattens it to `List<T>` by concatenating all inner lists
    /// within each outer list row.
    /// Inner nulls are preserved, outer nulls are skipped.
    ///
    /// Example: `[[1, 2, 3], [4, null, 5], null, [6]]` becomes `[1, 2, 3, 4, null, 5, 6]`.
    Flatten,

    /// A user-defined arbitrary function to convert a component column.
    Func(CustomFn),
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessField(inner) => f.debug_tuple("AccessField").field(inner).finish(),
            Self::Cast(inner) => f.debug_tuple("Cast").field(inner).finish(),
            Self::Flatten => f.debug_tuple("Flatten").finish(),
            Self::Func(_) => f.debug_tuple("Func").field(&"<function>").finish(),
        }
    }
}

impl Op {
    /// Extracts a specific field from a `StructArray`.
    pub fn access_field(field_name: impl Into<String>) -> Self {
        Self::AccessField(op::AccessField {
            field_name: field_name.into(),
        })
    }

    /// Efficiently casts a component to a new `DataType`.
    pub fn cast(data_type: DataType) -> Self {
        Self::Cast(op::Cast {
            to_inner_type: data_type,
        })
    }

    /// Ignores any input and returns a constant `ListArray`.
    ///
    /// Commonly used with [`LensBuilder::add_static_component_column_entity`].
    /// When used in non-static columns this function will _not_ guarantee the correct amount of rows.
    pub fn constant(value: ListArray) -> Self {
        Self::func(move |_| Ok(value.clone()))
    }

    /// Flattens a list array inside a component.
    ///
    /// Takes `List<List<T>>` and flattens it to `List<T>` by concatenating all inner lists
    /// within each outer list row.
    /// Inner nulls are preserved, outer nulls are skipped.
    ///
    /// Example: `[[1, 2, 3], [4, null, 5], null, [6]]` becomes `[1, 2, 3, 4, null, 5, 6]`.
    pub fn flatten() -> Self {
        Self::Flatten
    }

    /// A user-defined arbitrary function to convert a component column.
    pub fn func<F>(func: F) -> Self
    where
        F: for<'a> Fn(&'a ListArray) -> Result<ListArray, Error> + Send + Sync + 'static,
    {
        Self::Func(Box::new(func))
    }
}

impl Op {
    fn call(&self, list_array: &ListArray) -> Result<ListArray, Error> {
        match self {
            Self::Cast(op) => op.call(list_array),
            Self::AccessField(op) => op.call(list_array),
            Self::Flatten => Flatten::new().transform(list_array).map_err(Into::into),
            Self::Func(func) => func(list_array),
        }
    }
}

/// A lens that transforms component data from one form to another.
///
/// Lenses allow you to extract, transform, and restructure component data. They
/// are applied to chunks that match the specified entity path filter and contain
/// the target component.
///
/// # Assumptions
///
/// Works on component columns within a chunk. Because what goes into a chunk
/// is non-deterministic, and dependent on the batcher, no assumptions should be
/// made for values across rows.
pub struct Lens {
    pub(super) input: InputColumn,
    pub(super) outputs: Vec<LensKind>,
}

impl Lens {
    /// Returns a new [`LensBuilder`] with the given input column.
    ///
    /// By default, creates a one-to-one (temporal) lens. Call `.with_static()` or `.with_to_many()`
    /// on the builder to switch to a different mode.
    pub fn for_input_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> LensBuilder {
        LensBuilder::new(entity_path_filter, component)
    }

    /// Applies this lens and creates one or more chunks.
    fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        let found = chunk.components().get(self.input.component);

        // This means we drop chunks that belong to the same entity but don't have the component.
        let Some(column) = found else {
            return Default::default();
        };

        self.outputs
            .iter()
            .filter_map(|output| match output {
                LensKind::OneToOne {
                    target_entity,
                    components,
                    timelines,
                } => apply_one_to_one(chunk, column, target_entity, components, timelines),
                LensKind::Static {
                    target_entity,
                    components,
                } => apply_static(chunk, column, target_entity, components),
                LensKind::ToMany {
                    target_entity,
                    components,
                    timelines,
                } => apply_to_many(chunk, column, target_entity, components, timelines),
            })
            .collect()
    }
}

fn apply_ops(initial: ListArray, ops: &[Op]) -> Result<ListArray, Error> {
    ops.iter().try_fold(initial, |array, op| op.call(&array))
}

fn collect_output_components_iter(
    input: &SerializedComponentColumn,
    components: &[ComponentOutput],
) -> impl Iterator<Item = (ComponentDescriptor, ListArray)> {
    components.iter().filter_map(
        |output| match apply_ops(input.list_array.clone(), &output.ops) {
            Ok(list_array) => Some((output.component_descr.clone(), list_array)),
            Err(err) => {
                re_log::error!(
                    "Lens operations failed for component columns '{}': {err}",
                    output.component_descr
                );
                None
            }
        },
    )
}

fn collect_output_times_iter(
    input: &SerializedComponentColumn,
    timelines: &[TimeOutput],
) -> impl Iterator<Item = (TimelineName, TimeType, ListArray)> {
    timelines.iter().filter_map(
        |time| match apply_ops(input.list_array.clone(), &time.ops) {
            Ok(list_array) => Some((time.timeline_name, time.timeline_type, list_array)),
            Err(err) => {
                re_log::error!(
                    "Lens operations failed for time column '{}': {err}",
                    time.timeline_name,
                );
                None
            }
        },
    )
}

/// Check if the `list_array` is a [`arrow::array::Int64Array`] and if so, creates a [`re_chunk::TimeColumn`].
fn convert_to_time_column(
    (timeline_name, timeline_type, list_array): (TimelineName, TimeType, ListArray),
) -> Option<(TimelineName, re_chunk::TimeColumn)> {
    if let Some(time_vals) = list_array.values().as_any().downcast_ref::<Int64Array>() {
        let time_column = re_chunk::TimeColumn::new(
            None,
            Timeline::new(timeline_name, timeline_type),
            time_vals.values().clone(),
        );
        Some((timeline_name, time_column))
    } else {
        re_log::error_once!(
            "Output for timeline '{timeline_name}' must produce data type {}",
            DataType::List(Arc::new(Field::new_list_field(DataType::Int64, false))),
        );
        None
    }
}

fn resolve_entity_path<'a>(chunk: &'a Chunk, target_entity: &'a TargetEntity) -> &'a EntityPath {
    match target_entity {
        TargetEntity::SameAsInput => chunk.entity_path(),
        TargetEntity::Explicit(path) => path,
    }
}

/// Applies a one-to-one lens transformation where each input row produces exactly one output row.
///
/// The output chunk inherits all timelines from the input chunk, with additional timelines
/// extracted from the component data if specified. Component columns are transformed according
/// to the provided operations.
fn apply_one_to_one(
    chunk: &Chunk,
    input: &SerializedComponentColumn,
    target_entity: &TargetEntity,
    components: &[ComponentOutput],
    timelines: &[TimeOutput],
) -> Option<Chunk> {
    let entity_path = resolve_entity_path(chunk, target_entity);

    let output_component_columns = collect_output_components_iter(input, components);
    let output_time_columns =
        collect_output_times_iter(input, timelines).filter_map(convert_to_time_column);

    // Inherit all existing timelines as-is (since row count doesn't change),
    // then add any additional timelines extracted from component data.
    let mut final_timelines = chunk.timelines().clone();
    final_timelines.extend(output_time_columns);

    Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path.clone(),
        final_timelines,
        output_component_columns.collect(),
    )
    .inspect_err(|err| {
        re_log::error_once!("Failed to build lens output at entity path '{entity_path}': {err}");
    })
    .ok()
}

/// Applies a static lens transformation that produces timeless output data.
///
/// The output chunk contains no timelines, only the transformed component columns.
/// This is useful for metadata or other data that should not be associated with any timeline.
fn apply_static(
    chunk: &Chunk,
    input: &SerializedComponentColumn,
    target_entity: &TargetEntity,
    components: &[ComponentOutput],
) -> Option<Chunk> {
    let entity_path = resolve_entity_path(chunk, target_entity);

    // TODO(grtlr): In case of static, should we enforce single rows (i.e. unit chunks)?
    Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path.clone(),
        Default::default(),
        collect_output_components_iter(input, components).collect(),
    )
    .inspect_err(|err| {
        re_log::error_once!("Failed to build lens output at entity path '{entity_path}': {err}");
    })
    .ok()
}

fn apply_to_many(
    chunk: &Chunk,
    input: &SerializedComponentColumn,
    target_entity: &TargetEntity,
    components: &[ComponentOutput],
    timelines: &[TimeOutput],
) -> Option<Chunk> {
    use arrow::array::UInt32Array;

    let entity_path = resolve_entity_path(chunk, target_entity);

    let mut output_components = collect_output_components_iter(input, components).peekable();

    // Peek at the first component to establish the scatter pattern (how many output rows
    // each input row produces). All components must have the same outer list structure.
    // We use .peek() instead of consuming the iterator so we can still process all
    // components (including this first one) later.
    let Some((_descr, reference_array)) = output_components.peek() else {
        re_log::error_once!(
            "scatter lens requires at least one component output for entity '{entity_path}'"
        );
        return None;
    };

    // Build scatter indices: tracks which input row each output row came from
    // Example: [0, 0, 0, 1, 2] means rows 0-2 from input 0, row 3 from input 1, row 4 from input 2
    let mut scatter_indices = Vec::new();
    let offsets = reference_array.value_offsets();

    for (row_idx, window) in offsets.windows(2).enumerate() {
        let start = window[0] as usize;
        let end = window[1] as usize;
        let count = end - start;

        if reference_array.is_null(row_idx) || count == 0 {
            // Null or empty list produces one output row
            scatter_indices.push(row_idx as u32);
        } else {
            // Each element produces one output row
            for _ in 0..count {
                scatter_indices.push(row_idx as u32);
            }
        }
    }

    let scatter_indices_array = UInt32Array::from(scatter_indices);

    // Replicate all existing timeline values using scatter indices
    let mut final_timelines: IntMap<TimelineName, TimeColumn> = Default::default();
    for (timeline_name, time_column) in chunk.timelines() {
        let time_values = time_column.times_raw();
        let time_values_array = Int64Array::from(time_values.to_vec());

        // `arrow::compute::take` is fine to use in this context, because we want to allow nullability.
        #[expect(clippy::disallowed_methods)]
        match take(&time_values_array, &scatter_indices_array, None) {
            Ok(scattered) => {
                let scattered_i64 = scattered.as_primitive::<arrow::datatypes::Int64Type>();
                let new_time_column = re_chunk::TimeColumn::new(
                    None,
                    *time_column.timeline(),
                    scattered_i64.values().clone(),
                );
                final_timelines.insert(*timeline_name, new_time_column);
            }
            Err(err) => {
                re_log::error_once!(
                    "Failed to replicate timeline '{}' for entity '{entity_path}': {err}",
                    timeline_name
                );
                return None;
            }
        }
    }

    // Explode all timeline outputs
    let exploded_time_columns = collect_output_times_iter(input, timelines).filter_map(
        |(timeline_name, timeline_type, list_array)| match Explode.transform(&list_array) {
            Ok(exploded) => convert_to_time_column((timeline_name, timeline_type, exploded)),
            Err(err) => {
                re_log::error_once!(
                    "Failed to scatter timeline '{}' for entity '{entity_path}': {err}",
                    timeline_name
                );
                None
            }
        },
    );
    final_timelines.extend(exploded_time_columns);

    // Explode all component outputs
    let chunk_components = output_components.filter_map(|(component_descr, list_array)| {
        match Explode.transform(&list_array) {
            Ok(exploded) => Some(SerializedComponentColumn::new(exploded, component_descr)),
            Err(err) => {
                re_log::error_once!(
                    "Failed to scatter component '{}' for entity '{entity_path}': {err}",
                    component_descr.component
                );
                None
            }
        }
    });

    // Verify that all columns have the same length happens during chunk creation.
    Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path.clone(),
        final_timelines,
        chunk_components.collect(),
    )
    .inspect_err(|err| {
        re_log::error_once!("Failed to build lens output at entity path '{entity_path}': {err}");
    })
    .ok()
}

#[derive(Default)]
pub struct LensRegistry {
    lenses: Vec<Lens>,
}

impl LensRegistry {
    pub fn add_lens(&mut self, lens: Lens) {
        self.lenses.push(lens);
    }

    fn relevant(&self, chunk: &Chunk) -> impl Iterator<Item = &Lens> {
        self.lenses.iter().filter(|lens| {
            lens.input
                .entity_path_filter
                .clone()
                .resolve_without_substitutions()
                .matches(chunk.entity_path())
        })
    }

    /// Applies all relevant lenses to a chunk and returns the transformed chunks.
    ///
    /// This will only transform component columns that match registered lenses.
    /// Other component columns are dropped. To retain original data, use identity
    /// lenses or multi-sink configurations.
    pub fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        self.relevant(chunk)
            .flat_map(|transform| transform.apply(chunk))
            .collect()
    }
}

#[cfg(test)]
mod test {
    #![expect(clippy::cast_possible_wrap)]

    use std::sync::Arc;

    use re_chunk::{
        TimeColumn, TimelineName,
        external::arrow::{
            array::{
                Float32Builder, Float64Builder, Int32Builder, ListBuilder, StringBuilder,
                StructBuilder,
            },
            datatypes::{DataType, Field},
        },
    };
    use re_types::{ComponentDescriptor, archetypes::Scalars};

    use super::*;

    /// Creates a chunk that contains all sorts of validity, nullability, and empty lists.
    // ┌──────────────┬───────────┐
    // │ [{a:0,b:0}]  │ ["zero"]  │
    // ├──────────────┼───────────┤
    // │[{a:1,b:null}]│["one","1"]│
    // ├──────────────┼───────────┤
    // │      []      │    []     │
    // ├──────────────┼───────────┤
    // │     null     │ ["three"] │
    // ├──────────────┼───────────┤
    // │ [{a:4,b:4}]  │   null    │
    // ├──────────────┼───────────┤
    // │    [null]    │ ["five"]  │
    // ├──────────────┼───────────┤
    // │ [{a:6,b:6}]  │  [null]   │
    // └──────────────┴───────────┘
    fn nullability_chunk() -> Chunk {
        let mut struct_column_builder = ListBuilder::new(StructBuilder::new(
            [
                Arc::new(Field::new("a", DataType::Float32, true)),
                Arc::new(Field::new("b", DataType::Float64, true)),
            ],
            vec![
                Box::new(Float32Builder::new()),
                Box::new(Float64Builder::new()),
            ],
        ));
        let mut string_column_builder = ListBuilder::new(StringBuilder::new());

        // row 0
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(0.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(0.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_value("zero");
        string_column_builder.append(true);

        // row 1
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(1.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_null();
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_value("one");
        string_column_builder.values().append_value("1");
        string_column_builder.append(true);

        // row 2
        struct_column_builder.append(true); // empty list

        string_column_builder.append(true); // empty list

        // row 3
        struct_column_builder.append(false); // null

        string_column_builder.values().append_value("three");
        string_column_builder.append(true);

        // row 4
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(4.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(4.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.append(false); // null

        // row 5
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_null(); // placeholder for null struct
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_null(); // placeholder for null struct
        struct_column_builder.values().append(false); // null struct element
        struct_column_builder.append(true);

        string_column_builder.values().append_value("five");
        string_column_builder.append(true);

        // row 6
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(6.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(6.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_null();
        string_column_builder.append(true);

        let struct_column = struct_column_builder.finish();
        let string_column = string_column_builder.finish();

        let components = [
            (ComponentDescriptor::partial("structs"), struct_column),
            (ComponentDescriptor::partial("strings"), string_column),
        ]
        .into_iter();

        let time_column = TimeColumn::new_sequence("tick", [0, 1, 2, 3, 4, 5, 6]);

        Chunk::from_auto_row_ids(
            ChunkId::new(),
            "nullability".into(),
            std::iter::once((TimelineName::new("tick"), time_column)).collect(),
            components.collect(),
        )
        .unwrap()
    }

    #[test]
    fn test_destructure_cast() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "structs")
                .output_columns_at("nullability/a", |out| {
                    out.component(
                        Scalars::descriptor_scalars(),
                        [Op::access_field("a"), Op::cast(DataType::Float64)],
                    )
                })
                .build();

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_cast", format!("{chunk:-240}"));
    }

    #[test]
    fn test_destructure() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "structs")
                .output_columns_at("nullability/b", |out| {
                    out.component(Scalars::descriptor_scalars(), [Op::access_field("b")])
                })
                .build();

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_only", format!("{chunk:-240}"));
    }

    #[test]
    fn test_inner_count() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let count_fn = |list_array: &ListArray| {
            let mut builder = ListBuilder::new(Int32Builder::new());

            for maybe_array in list_array.iter() {
                match maybe_array {
                    None => builder.append_null(),
                    Some(component_batch_array) => {
                        builder
                            .values()
                            .append_value(component_batch_array.len() as i32);
                        builder.append(true);
                    }
                }
            }

            Ok(builder.finish())
        };

        let count =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "strings")
                .output_columns(|out| {
                    out.component(ComponentDescriptor::partial("counts"), [Op::func(count_fn)])
                        .component(ComponentDescriptor::partial("original"), [])
                })
                .build();

        let pipeline = LensRegistry {
            lenses: vec![count],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("inner_count", format!("{chunk:-240}"));
    }

    #[test]
    fn test_static_chunk_creation() {
        let original_chunk = nullability_chunk();

        let mut metadata_builder_a = ListBuilder::new(StringBuilder::new());
        metadata_builder_a
            .values()
            .append_value("static_metadata_a");
        metadata_builder_a.append(true);

        let mut metadata_builder_b = ListBuilder::new(StringBuilder::new());
        metadata_builder_b
            .values()
            .append_value("static_metadata_b");
        metadata_builder_b.append(true);

        let static_lens =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "strings")
                .output_static_columns_at("nullability/static", |out| {
                    out.component(
                        ComponentDescriptor::partial("static_metadata_a"),
                        [Op::constant(metadata_builder_a.finish())],
                    )
                    .component(
                        ComponentDescriptor::partial("static_metadata_b"),
                        [Op::constant(metadata_builder_b.finish())],
                    )
                })
                .build();

        let pipeline = LensRegistry {
            lenses: vec![static_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("single_static", format!("{chunk:-240}"));
    }

    #[test]
    fn test_time_column_extraction() {
        // Create a chunk with timestamp data that can be extracted as a time column
        let mut timestamp_builder = ListBuilder::new(arrow::array::Int64Builder::new());
        let mut value_builder = ListBuilder::new(Int32Builder::new());

        // Add rows with timestamps and corresponding values
        for i in 0..5 {
            timestamp_builder.values().append_value(100 + i * 10);
            timestamp_builder.append(true);

            value_builder.values().append_value(i as i32);
            value_builder.append(true);
        }

        let timestamp_column = timestamp_builder.finish();
        let value_column = value_builder.finish();

        let components = [
            (
                ComponentDescriptor::partial("my_timestamp"),
                timestamp_column,
            ),
            (ComponentDescriptor::partial("value"), value_column),
        ]
        .into_iter();

        // Create chunk without the custom timeline initially
        let time_column = TimeColumn::new_sequence("tick", [0, 1, 2, 3, 4]);

        let original_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            "timestamped".into(),
            std::iter::once((TimelineName::new("tick"), time_column)).collect(),
            components.collect(),
        )
        .unwrap();

        println!("{original_chunk}");

        // Create a lens that extracts the timestamp as a time column and keeps the original timestamp as a component
        let time_lens = Lens::for_input_column(
            EntityPathFilter::parse_forgiving("timestamped"),
            "my_timestamp",
        )
        .output_columns(|out| {
            out.time("my_timeline", TimeType::Sequence, [])
                .component(ComponentDescriptor::partial("extracted_time"), [])
        })
        .build();

        let pipeline = LensRegistry {
            lenses: vec![time_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        println!("{chunk}");

        // Verify the chunk has both the original timeline and the new custom timeline
        assert!(chunk.timelines().contains_key(&TimelineName::new("tick")));
        assert!(
            chunk
                .timelines()
                .contains_key(&TimelineName::new("my_timeline"))
        );

        // Verify the custom timeline has the correct values
        let my_timeline = chunk
            .timelines()
            .get(&TimelineName::new("my_timeline"))
            .unwrap();
        assert_eq!(my_timeline.times_raw().len(), 5);
        assert_eq!(my_timeline.times_raw()[0], 100);
        assert_eq!(my_timeline.times_raw()[1], 110);
        assert_eq!(my_timeline.times_raw()[2], 120);
        assert_eq!(my_timeline.times_raw()[3], 130);
        assert_eq!(my_timeline.times_raw()[4], 140);
    }

    // Helper function to create test data: list of structs with {timestamp: i64, value: String}
    fn create_test_struct_list() -> ListArray {
        use arrow::array::Int64Builder;

        let mut struct_list_builder = ListBuilder::new(StructBuilder::new(
            [
                Arc::new(Field::new("timestamp", DataType::Int64, true)),
                Arc::new(Field::new("value", DataType::Utf8, true)),
            ],
            vec![
                Box::new(Int64Builder::new()),
                Box::new(StringBuilder::new()),
            ],
        ));

        let mut timestamp_counter = 1i64..;

        // Row 0: [{1, "one"}, {2, "two"}, {3, "three"}]
        struct_list_builder
            .values()
            .field_builder::<Int64Builder>(0)
            .unwrap()
            .append_value(timestamp_counter.next().unwrap());
        struct_list_builder
            .values()
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_value("one");
        struct_list_builder.values().append(true);

        struct_list_builder
            .values()
            .field_builder::<Int64Builder>(0)
            .unwrap()
            .append_value(timestamp_counter.next().unwrap());
        struct_list_builder
            .values()
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_value("two");
        struct_list_builder.values().append(true);

        struct_list_builder
            .values()
            .field_builder::<Int64Builder>(0)
            .unwrap()
            .append_value(timestamp_counter.next().unwrap());
        struct_list_builder
            .values()
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_value("three");
        struct_list_builder.values().append(true);

        struct_list_builder.append(true);

        // Row 1: [{4, "four"}]
        struct_list_builder
            .values()
            .field_builder::<Int64Builder>(0)
            .unwrap()
            .append_value(timestamp_counter.next().unwrap());
        struct_list_builder
            .values()
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_value("four");
        struct_list_builder.values().append(true);
        struct_list_builder.append(true);

        // Row 2: [{5, null}]
        struct_list_builder
            .values()
            .field_builder::<Int64Builder>(0)
            .unwrap()
            .append_value(timestamp_counter.next().unwrap());
        struct_list_builder
            .values()
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_null();
        struct_list_builder.values().append(true);
        struct_list_builder.append(true);

        struct_list_builder.finish()
    }

    #[test]
    fn test_scatter_columns() {
        // Create a chunk with list of structs that should be exploded/scattered
        // Each element is a struct with {timestamp: i64, value: String}
        let struct_list = create_test_struct_list();

        let components =
            std::iter::once((ComponentDescriptor::partial("nested_data"), struct_list));

        let time_column = TimeColumn::new_sequence("tick", [1, 2, 3]);

        let original_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            "scatter_test".into(),
            std::iter::once((time_column.timeline().name().to_owned(), time_column)).collect(),
            components.collect(),
        )
        .unwrap();

        println!("Original chunk:");
        println!("{original_chunk}");

        // Helper to extract value field from structs: List<Struct> -> List<String>
        let extract_value = |list_array: &ListArray| -> Result<ListArray, Error> {
            use re_arrow_combinators::{Transform as _, map::MapList, reshape::GetField};
            Ok(MapList::new(GetField::new("value")).transform(list_array)?)
        };

        // Helper to extract timestamp field from structs: List<Struct> -> List<Int64>
        let extract_timestamp = |list_array: &ListArray| -> Result<ListArray, Error> {
            use re_arrow_combinators::{Transform as _, map::MapList, reshape::GetField};
            Ok(MapList::new(GetField::new("timestamp")).transform(list_array)?)
        };

        // Create a scatter lens that explodes the nested lists
        let scatter_lens = Lens::for_input_column(EntityPathFilter::all(), "nested_data")
            .output_scatter_columns_at("scatter_test/exploded", |out| {
                out.component(
                    ComponentDescriptor::partial("exploded_strings"),
                    [Op::func(extract_value)],
                )
                .time(
                    "my_timestamp",
                    TimeType::Sequence,
                    [Op::func(extract_timestamp)],
                )
            })
            .build();

        let pipeline = LensRegistry {
            lenses: vec![scatter_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        println!("\nExploded chunk:");
        println!("{chunk}");

        // Verify the structure
        // Input had 3 rows with list of structs:
        // Row 0: [{1, "one"}, {2, "two"}, {3, "three"}] → 3 output rows
        // Row 1: [{4, "four"}] → 1 output row
        // Row 2: [{5, null}] → 1 output row
        // Total: 5 output rows
        assert_eq!(chunk.num_rows(), 5);

        // Verify tick timeline is replicated correctly
        // Original tick: [1, 2, 3]
        // Scattered tick: [1, 1, 1, 2, 3] (row 0 scatters into 3 rows)
        let tick_timeline = chunk.timelines().get(&TimelineName::new("tick")).unwrap();
        assert_eq!(tick_timeline.times_raw().len(), 5);
        assert_eq!(tick_timeline.times_raw()[0], 1);
        assert_eq!(tick_timeline.times_raw()[1], 1);
        assert_eq!(tick_timeline.times_raw()[2], 1);
        assert_eq!(tick_timeline.times_raw()[3], 2);
        assert_eq!(tick_timeline.times_raw()[4], 3);

        // Verify my_timestamp timeline is extracted from the timestamp field
        // The timestamps are: 1, 2, 3 (from row 0), 4 (row 1), 5 (row 2)
        // After scattering: [1, 2, 3, 4, 5]
        let event_timeline = chunk
            .timelines()
            .get(&TimelineName::new("my_timestamp"))
            .unwrap();
        assert_eq!(event_timeline.times_raw().len(), 5);
        assert_eq!(event_timeline.times_raw()[0], 1);
        assert_eq!(event_timeline.times_raw()[1], 2);
        assert_eq!(event_timeline.times_raw()[2], 3);
        assert_eq!(event_timeline.times_raw()[3], 4);
        assert_eq!(event_timeline.times_raw()[4], 5);

        insta::assert_snapshot!("scatter_columns", format!("{chunk:-240}"));
    }

    #[test]
    fn test_scatter_columns_static() {
        // Test scatter with no existing timelines - only exploded timeline outputs
        let struct_list = create_test_struct_list();

        let components =
            std::iter::once((ComponentDescriptor::partial("nested_data"), struct_list));

        // Create chunk WITHOUT any timelines
        let original_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            "scatter_test".into(),
            std::iter::empty().collect(), // No timelines!
            components.collect(),
        )
        .unwrap();

        println!("Original chunk (no timelines):");
        println!("{original_chunk}");

        // Helper to extract value field from structs: List<Struct> -> List<String>
        let extract_value = |list_array: &ListArray| -> Result<ListArray, Error> {
            use re_arrow_combinators::{Transform as _, map::MapList, reshape::GetField};
            Ok(MapList::new(GetField::new("value")).transform(list_array)?)
        };

        // Helper to extract timestamp field from structs: List<Struct> -> List<Int64>
        let extract_timestamp = |list_array: &ListArray| -> Result<ListArray, Error> {
            use re_arrow_combinators::{Transform as _, map::MapList, reshape::GetField};
            Ok(MapList::new(GetField::new("timestamp")).transform(list_array)?)
        };

        // Create a scatter lens that explodes the nested lists
        let scatter_lens = Lens::for_input_column(EntityPathFilter::all(), "nested_data")
            .output_scatter_columns_at("scatter_test/exploded", |out| {
                out.component(
                    ComponentDescriptor::partial("exploded_strings"),
                    [Op::func(extract_value)],
                )
                .time(
                    "my_timestamp",
                    TimeType::Sequence,
                    [Op::func(extract_timestamp)],
                )
            })
            .build();

        let pipeline = LensRegistry {
            lenses: vec![scatter_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        println!("\nExploded chunk:");
        println!("{chunk}");

        // Verify the structure
        // Input had 3 rows with list of structs:
        // Row 0: [{1, "one"}, {2, "two"}, {3, "three"}] → 3 output rows
        // Row 1: [{4, "four"}] → 1 output row
        // Row 2: [{5, null}] → 1 output row
        // Total: 5 output rows
        assert_eq!(chunk.num_rows(), 5);

        // Verify there are NO scattered timelines from input (since input had none)
        // Only the exploded my_timestamp timeline should exist
        assert_eq!(chunk.timelines().len(), 1);

        // Verify my_timestamp timeline is extracted from the timestamp field
        // The timestamps are: 1, 2, 3 (from row 0), 4 (row 1), 5 (row 2)
        // After scattering: [1, 2, 3, 4, 5]
        let event_timeline = chunk
            .timelines()
            .get(&TimelineName::new("my_timestamp"))
            .unwrap();
        assert_eq!(event_timeline.times_raw().len(), 5);
        assert_eq!(event_timeline.times_raw()[0], 1);
        assert_eq!(event_timeline.times_raw()[1], 2);
        assert_eq!(event_timeline.times_raw()[2], 3);
        assert_eq!(event_timeline.times_raw()[3], 4);
        assert_eq!(event_timeline.times_raw()[4], 5);

        // Verify exploded_strings component exists
        let strings_component = chunk
            .components()
            .get(ComponentDescriptor::partial("exploded_strings").component)
            .unwrap();
        assert_eq!(strings_component.list_array.len(), 5);

        insta::assert_snapshot!("scatter_columns_static", format!("{chunk:-240}"));
    }
}
