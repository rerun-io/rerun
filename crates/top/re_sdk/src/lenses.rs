use re_chunk::{
    Chunk, ChunkComponents, ChunkId, ComponentIdentifier, EntityPath,
    external::arrow::array::ListArray,
};
use re_log_types::{EntityPathFilter, LogMsg, ResolvedEntityPathFilter};
use re_types::{ComponentDescriptor, SerializedComponentColumn};

use crate::sink::LogSink;

/// A sink which can transform a `LogMsg` and forward the result to an underlying backing `LogSink`.
///
/// The sink will only forward components that are matched by a lens specified via [`Self::with_lens`].
pub struct LensesSink<S: LogSink> {
    sink: S,
    registry: LensRegistry,
}

impl<S: LogSink> LensesSink<S> {
    /// Create a new sink with the given lenses.
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            registry: Default::default(),
        }
    }

    /// Adds a [`Lens`] to this sink.
    pub fn with_lens(mut self, lens: Lens) -> Self {
        self.registry.lenses.push(lens);
        self
    }
}

impl<S: LogSink> LogSink for LensesSink<S> {
    fn send(&self, msg: re_log_types::LogMsg) {
        match &msg {
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {
                self.sink.send(msg);
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => match Chunk::from_arrow_msg(arrow_msg) {
                Ok(chunk) => {
                    let new_chunks = self.registry.apply(&chunk);
                    // TODO(grtlr): Should we use `self.sink.send_all` here?
                    for new_chunk in new_chunks {
                        match new_chunk.to_arrow_msg() {
                            Ok(arrow_msg) => {
                                self.sink
                                    .send(LogMsg::ArrowMsg(store_id.clone(), arrow_msg));
                            }
                            Err(err) => {
                                re_log::error_once!(
                                    "failed to create log message from chunk: {err}"
                                );
                            }
                        }
                    }
                }

                Err(err) => {
                    re_log::error_once!("Failed to convert arrow message to chunk: {err}");
                    self.sink.send(msg);
                }
            },
        }
    }

    fn flush_blocking(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), crate::sink::SinkFlushError> {
        self.sink.flush_blocking(timeout)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Provides a declarative interface for constructing lenses.
pub struct LensBuilder {
    /// The entity path to apply the transformation to.
    filter: EntityPathFilter,

    /// The component that we want to select.
    component: ComponentIdentifier,

    ops: Vec<(op::Op, ComponentDescriptor)>,
}

impl LensBuilder {
    /// Creates a new builder.
    ///
    /// The resulting lens will be applied to all components that are on entities matching the `filter`.
    pub fn new(filter: EntityPathFilter, component: impl Into<ComponentIdentifier>) -> Self {
        Self {
            filter,
            component: component.into(),
            ops: Default::default(),
        }
    }

    /// Manipulates the component and attaches a new [`ComponentDescriptor`].
    pub fn view_as(mut self, op: op::Op, descriptor: ComponentDescriptor) -> Self {
        self.ops.push((op, descriptor));
        self
    }

    /// Attaches a new [`ComponentDescriptor`].
    pub fn describe(self, descriptor: ComponentDescriptor) -> Self {
        self.view_as(op::nop(), descriptor)
    }

    /// Consumes the builder and constructs a [`Lens`].
    pub fn build(self) -> Lens {
        Lens::new(
            self.filter,
            self.component,
            move |list_array, entity_path| {
                self.ops
                    .clone()
                    .into_iter()
                    .map(|(op, descriptor)| {
                        op.describe(entity_path.clone(), list_array.clone(), descriptor)
                            .unwrap()
                    })
                    .collect()
            },
        )
    }
}

/// A transformed column result from applying a lens operation.
///
/// Contains the output of a lens transformation, including the new entity path,
/// the serialized component data, and whether the data should be treated as static.
#[derive(Debug)]
pub struct TransformedColumn {
    /// The entity path where this transformed column should be logged.
    pub entity_path: EntityPath,
    /// The serialized component column containing the transformed data.
    pub column: SerializedComponentColumn,
    /// Whether this column represents static data.
    pub is_static: bool,
}

impl TransformedColumn {
    /// Creates a new transformed column.
    pub fn new(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: false,
        }
    }

    /// Creates a new static transformed column.
    pub fn new_static(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: true,
        }
    }
}

type LensFunc = Box<dyn Fn(ListArray, &EntityPath) -> Vec<TransformedColumn> + Send + Sync>;

/// A lens that transforms component data from one form to another.
///
/// Lenses allow you to extract, transform, and restructure component data
/// as it flows through the logging pipeline. They are applied to chunks
/// that match the specified entity path filter and contain the target component.
pub struct Lens {
    /// The entity path to apply the transformation to.
    pub filter: ResolvedEntityPathFilter,

    /// The component that we want to select.
    pub component: ComponentIdentifier,

    /// A closure that outputs a list of chunks
    pub func: LensFunc,
}

#[derive(Default)]
struct LensRegistry {
    lenses: Vec<Lens>,
}

impl LensRegistry {
    fn relevant(&self, chunk: &Chunk) -> impl Iterator<Item = &Lens> {
        self.lenses
            .iter()
            .filter(|transform| transform.filter.matches(chunk.entity_path()))
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

impl Lens {
    /// Creates a new lens with the specified filter, component, and transformation function.
    ///
    /// # Arguments
    /// * `entity_path_filter` - Filter to match entity paths this lens should apply to
    /// * `component` - The component identifier to transform
    /// * `func` - Transformation function that takes a ListArray and EntityPath and returns transformed columns
    pub fn new<F>(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
        func: F,
    ) -> Self
    where
        F: Fn(ListArray, &EntityPath) -> Vec<TransformedColumn> + Send + Sync + 'static,
    {
        Self {
            filter: entity_path_filter.resolve_without_substitutions(),
            component: component.into(),
            func: Box::new(func),
        }
    }

    fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        let found = chunk
            .components()
            .iter()
            .find(|(descr, _array)| descr.component == self.component);

        // TODO: This means we drop chunks that belong to the same entity but don't have the component.
        let Some((_component_descr, list_array)) = found else {
            return Default::default();
        };

        // TODO:
        // * unwrap array
        // * Guarantee that there is only one component descr
        let mut builders = ahash::HashMap::default();
        let results = (self.func)(list_array.clone(), chunk.entity_path());
        for transformed in results {
            let components = builders
                .entry((transformed.entity_path, transformed.is_static))
                .or_insert_with(ChunkComponents::default);

            if components.contains_component(&transformed.column.descriptor) {
                re_log::warn_once!(
                    "Replacing duplicated component {}",
                    transformed.column.descriptor.component
                );
            }

            components.insert(transformed.column.descriptor, transformed.column.list_array);
        }

        builders
            .into_iter()
            .filter_map(|((entity_path, is_static), components)| {
                let timelines = if is_static {
                    Default::default()
                } else {
                    chunk.timelines().clone()
                };

                // TODO: In case of static, should we use sparse rows instead?
                Chunk::from_auto_row_ids(ChunkId::new(), entity_path.clone(), timelines, components)
                    .inspect_err(|err| {
                        re_log::error_once!(
                            "Failed to build chunk at entity path '{entity_path}': {err}"
                        );
                    })
                    .ok()
            })
            .collect()
    }
}

/// Provides commonly used transformations of Arrow arrays.
///
/// # Experimental
///
/// This is an experimental API and may change in future releases.
pub mod op {

    // TODO(grtlr): Make this into proper objects, with APIs similar to Datafusion's UDFs.

    use std::sync::Arc;

    use re_chunk::{
        EntityPath,
        external::arrow::{
            array::{
                Float32Builder, Float64Builder, ListArray, ListBuilder, StructArray, StructBuilder,
            },
            compute,
            datatypes::{DataType, Field},
        },
    };
    use re_types::{ComponentDescriptor, SerializedComponentColumn};

    use super::TransformedColumn;

    #[derive(Debug)]
    pub struct Error;

    /// TODO
    #[derive(Clone)]
    pub struct Op {
        view_fn: Arc<
            dyn Fn(EntityPath, ListArray) -> Result<(EntityPath, ListArray), Error>
                // TODO: Proper error handling
                + Send
                + Sync,
        >,
        // set_fn: Arc<dyn Fn(EntityPath, ListArray, ComponentDescriptor) -> Result<TransformedColumn, Error> + Send + Sync;
    }

    /// TODO
    impl Op {
        pub fn new<V>(view: V) -> Self
        where
            V: Fn(EntityPath, ListArray) -> Result<(EntityPath, ListArray), Error>
                + Send
                + Sync
                + 'static,
        {
            Self {
                view_fn: Arc::new(view),
            }
        }

        /// TODO
        pub fn view(
            &self,
            entity_path: EntityPath,
            list_array: ListArray,
        ) -> Result<(EntityPath, ListArray), Error> {
            (self.view_fn)(entity_path, list_array)
        }

        pub(super) fn describe(
            self,
            entity_path: EntityPath,
            list_array: ListArray,
            descriptor: ComponentDescriptor,
        ) -> Result<TransformedColumn, Error> {
            let (entity_path, list_array) = self.view(entity_path, list_array)?;
            Ok(TransformedColumn {
                entity_path,
                column: SerializedComponentColumn {
                    list_array,
                    descriptor,
                },
                is_static: false,
            })
        }

        pub fn and_then(self, other: Self) -> Self {
            let view_fn = {
                let self_view = Arc::clone(&self.view_fn);
                let other_view = Arc::clone(&other.view_fn);
                move |entity_path, list_array| {
                    let (entity_path, list_array) = self_view(entity_path, list_array)?;
                    other_view(entity_path, list_array)
                }
            };
            Self {
                view_fn: Arc::new(view_fn),
            }
        }
    }

    /// Extracts a specific field from a struct component within a ListArray.
    pub fn access_field(name: impl Into<String>) -> Op {
        let name = name.into();
        Op::new(move |entity_path, list_array| {
            let (_, offsets, values, nulls) = list_array.into_parts();
            let struct_array = values
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| Error)?;
            let column = struct_array.column_by_name(&name).ok_or_else(|| Error)?;
            Ok((
                entity_path.join(&EntityPath::parse_forgiving(&name)),
                ListArray::new(
                    Arc::new(Field::new_list_field(column.data_type().clone(), true)),
                    offsets,
                    column.clone(),
                    nulls,
                ),
            ))
        })
    }

    /// Casts the inner array of a `ListArray` to a different data type.
    pub fn cast(to_inner_type: DataType) -> Op {
        Op::new(move |entity_path, list_array| {
            let (_, offsets, ref array, nulls) = list_array.into_parts();
            let res = compute::cast(array, &to_inner_type).map_err(|_| Error)?;
            Ok((
                entity_path,
                ListArray::new(
                    Arc::new(Field::new_list_field(res.data_type().clone(), true)),
                    offsets,
                    res,
                    nulls,
                ),
            ))
        })
    }

    pub(super) fn nop() -> Op {
        Op::new(move |entity_path, list_array| Ok((entity_path, list_array)))
    }

    #[test]
    fn test_op_describe() {
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

        let struct_list_array = struct_column_builder.finish();

        let op = access_field("a").and_then(cast(DataType::Float64));
        insta::assert_debug_snapshot!(
            op.describe(
                EntityPath::parse_forgiving("test"),
                struct_list_array,
                ComponentDescriptor::partial("test")
            )
            .unwrap()
        );
    }

    /// Extracts a specific field from a struct component within a ListArray.
    ///
    /// Takes a ListArray containing StructArrays and extracts the specified field,
    /// returning a new ListArray containing only that field's data.
    /// Returns an empty ListArray if the extraction fails.
    #[deprecated]
    pub fn extract_field(list_array: ListArray, column_name: &str) -> ListArray {
        let (field, offsets, values, nulls) = list_array.into_parts();
        let struct_array = match values.as_any().downcast_ref::<StructArray>() {
            Some(array) => array,
            None => {
                re_log::error_once!("Expected StructArray in ListArray, but found different type");
                return ListArray::new_null(field, offsets.len() - 1);
            }
        };
        let column = match struct_array.column_by_name(column_name) {
            Some(col) => col,
            None => {
                re_log::error_once!("Field '{}' not found in struct", column_name);
                return ListArray::new_null(field, offsets.len() - 1);
            }
        };
        ListArray::new(
            Arc::new(Field::new_list_field(column.data_type().clone(), true)),
            offsets,
            column.clone(),
            nulls,
        )
    }

    /// Casts the inner array of a ListArray to a different data type.
    ///
    /// Performs type casting on the component data within the ListArray,
    /// preserving the list structure while changing the inner data type.
    /// Returns an empty ListArray if the cast fails.
    #[deprecated]
    pub fn cast_component_batch(list_array: ListArray, to_inner_type: &DataType) -> ListArray {
        let (field, offsets, ref array, nulls) = list_array.into_parts();
        let res = match compute::cast(array, to_inner_type) {
            Ok(casted) => casted,
            Err(err) => {
                re_log::error_once!("Failed to cast array to {:?}: {}", to_inner_type, err);
                return ListArray::new_null(field, offsets.len() - 1);
            }
        };
        ListArray::new(
            Arc::new(Field::new_list_field(res.data_type().clone(), true)),
            offsets,
            res,
            nulls,
        )
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use re_chunk::{
        TimeColumn, TimelineName,
        external::arrow::{
            array::{
                Float32Builder, Float64Array, Float64Builder, Int32Builder, ListBuilder,
                StringBuilder, StructBuilder,
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

        let destructure = Lens::new(
            "nullability".parse().unwrap(),
            "structs",
            |list_array, entity_path| {
                let list_array = op::extract_field(list_array, "a");
                let list_array = op::cast_component_batch(list_array, &DataType::Float64);

                vec![TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("a")),
                    SerializedComponentColumn {
                        list_array,
                        descriptor: Scalars::descriptor_scalars(),
                    },
                )]
            },
        );

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_cast", format!("{chunk:-240}"));
    }

    #[test]
    fn test_destructure_cast_builder() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure = LensBuilder::new("nullability".parse().unwrap(), "structs")
            .view_as(
                op::access_field("a").and_then(op::cast(DataType::Float64)),
                Scalars::descriptor_scalars(),
            )
            .build();

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_cast_builder", format!("{chunk:-240}"));
    }

    #[test]
    fn test_destructure() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure = Lens::new(
            "nullability".parse().unwrap(),
            "structs",
            |list_array, entity_path| {
                let list_array = op::extract_field(list_array, "b");

                vec![TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("b")),
                    SerializedComponentColumn {
                        list_array,
                        descriptor: Scalars::descriptor_scalars(),
                    },
                )]
            },
        );

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

        let count = Lens::new(
            "nullability".parse().unwrap(),
            "strings",
            |list_array, entity_path| {
                // We keep the original `list_array` around for better comparability.
                let original_list_array = list_array.clone();
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

                let list_array = builder.finish();

                vec![
                    TransformedColumn::new(
                        entity_path.join(&EntityPath::parse_forgiving("b_count")),
                        SerializedComponentColumn {
                            list_array,
                            descriptor: ComponentDescriptor::partial("counts"),
                        },
                    ),
                    TransformedColumn::new(
                        entity_path.join(&EntityPath::parse_forgiving("b_count")),
                        SerializedComponentColumn {
                            list_array: original_list_array,
                            descriptor: ComponentDescriptor::partial("original"),
                        },
                    ),
                ]
            },
        );

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

        let static_lens_a = Lens::new(
            "nullability".parse().unwrap(),
            "strings",
            |_, entity_path| {
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

                vec![
                    TransformedColumn::new_static(
                        entity_path.join(&EntityPath::parse_forgiving("static")),
                        SerializedComponentColumn {
                            list_array: metadata_builder_a.finish(),
                            descriptor: ComponentDescriptor::partial("static_metadata_a"),
                        },
                    ),
                    TransformedColumn::new_static(
                        entity_path.join(&EntityPath::parse_forgiving("static")),
                        SerializedComponentColumn {
                            list_array: metadata_builder_b.finish(),
                            descriptor: ComponentDescriptor::partial("static_metadata_b"),
                        },
                    ),
                ]
            },
        );

        let pipeline = LensRegistry {
            lenses: vec![static_lens_a],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("single_static", format!("{chunk:-240}"));
    }

    #[test]
    fn test_archtype_from_column() {
        let mut f64_column_builder = ListBuilder::new(Float64Builder::new());

        // row 0
        f64_column_builder.values().append_value(0.0);
        f64_column_builder.append(true);

        // row 1
        f64_column_builder.values().append_value(1.0);
        f64_column_builder.append(true);

        // row 2
        f64_column_builder.append(true); // empty list

        // row 3
        f64_column_builder.values().append_value(3.0);
        f64_column_builder.append(true);

        // row 4
        f64_column_builder.append(false); // null

        // row 5
        f64_column_builder.values().append_value(5.0);
        f64_column_builder.append(true);

        // row 6
        f64_column_builder.values().append_null();
        f64_column_builder.append(true);

        let f64_column = f64_column_builder.finish();

        let value_array = f64_column
            .values()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();

        // dbg!(&value_array);

        let lengths = dbg!(value_array.offsets().lengths().collect::<Vec<_>>());

        let archetype = Scalars::new(value_array.iter().filter_map(|x| x)).columns(lengths);

        let archetype = archetype.unwrap().collect::<Vec<_>>();

        dbg!(archetype);

        assert!(false);
    }
}
