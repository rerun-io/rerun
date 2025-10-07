use op::{cast_component_batch, extract_field};
use re_chunk::{
    ArrowArray, Chunk, ChunkComponents, ChunkId, ComponentIdentifier, EntityPath,
    external::arrow::{
        array::ListArray, datatypes::DataType, error::ArrowError,
        ipc::MessageHeaderUnionTableOffset,
    },
};
use re_log_types::{EntityPathFilter, LogMsg, ResolvedEntityPathFilter};
use re_types::{AsComponents, ComponentDescriptor, SerializedComponentColumn};

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
    pub fn with_lens(mut self, lens: LensN) -> Self {
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

// /// Provides a declarative interface for constructing lenses.
// pub struct LensBuilder {
//     /// The entity path to apply the transformation to.
//     filter: EntityPathFilter,

//     /// The component that we want to select.
//     component: ComponentIdentifier,

//     ops: Vec<(op::Op, ComponentDescriptor)>,
// }

// impl LensBuilder {
//     /// Creates a new builder.
//     ///
//     /// The resulting lens will be applied to all components that are on entities matching the `filter`.
//     pub fn new(filter: EntityPathFilter, component: impl Into<ComponentIdentifier>) -> Self {
//         Self {
//             filter,
//             component: component.into(),
//             ops: Default::default(),
//         }
//     }

//     /// Manipulates the component and attaches a new [`ComponentDescriptor`].
//     pub fn view_as(mut self, op: op::Op, descriptor: ComponentDescriptor) -> Self {
//         self.ops.push((op, descriptor));
//         self
//     }

//     /// Attaches a new [`ComponentDescriptor`].
//     pub fn describe(self, descriptor: ComponentDescriptor) -> Self {
//         self.view_as(op::nop(), descriptor)
//     }

//     /// Consumes the builder and constructs a [`Lens`].
//     pub fn build(self) -> Lens {
//         Lens::new(
//             self.filter,
//             self.component,
//             move |list_array, entity_path| {
//                 self.ops
//                     .clone()
//                     .into_iter()
//                     .map(|(op, descriptor)| {
//                         op.describe(entity_path.clone(), list_array.clone(), descriptor)
//                             .unwrap()
//                     })
//                     .collect()
//             },
//         )
//     }
// }

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
pub struct LensOld {
    /// The entity path to apply the transformation to.
    pub filter: ResolvedEntityPathFilter,

    /// The component that we want to select.
    pub component: ComponentIdentifier,

    /// A closure that outputs a list of chunks
    pub func: LensFunc,
}

#[derive(Default)]
struct LensRegistry {
    lenses: Vec<LensN>,
}

impl LensRegistry {
    fn relevant(&self, chunk: &Chunk) -> impl Iterator<Item = &LensN> {
        self.lenses
            .iter()
            .filter(|lens| lens.input.entity_path_filter.matches(chunk.entity_path()))
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

impl LensOld {
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

// /// Provides commonly used transformations of Arrow arrays.
// ///
// /// # Experimental
// ///
// /// This is an experimental API and may change in future releases.
mod op {

    //     // TODO(grtlr): Make this into proper objects, with APIs similar to Datafusion's UDFs.

    use std::sync::Arc;

    use re_chunk::{
        ArrowArray, EntityPath,
        external::arrow::{
            array::{
                Float32Builder, Float64Builder, ListArray, ListBuilder, StructArray, StructBuilder,
            },
            compute,
            datatypes::{DataType, Field},
        },
    };
    use re_types::{ComponentDescriptor, SerializedComponentColumn};

    use super::{Error, TransformedColumn};

    //     /// TODO
    //     #[derive(Clone)]
    //     pub struct Op {
    //         view_fn: Arc<
    //             dyn Fn(EntityPath, ListArray) -> Result<(EntityPath, ListArray), Error>
    //                 // TODO: Proper error handling
    //                 + Send
    //                 + Sync,
    //         >,
    //         // set_fn: Arc<dyn Fn(EntityPath, ListArray, ComponentDescriptor) -> Result<TransformedColumn, Error> + Send + Sync;
    //     }

    //     /// TODO
    //     impl Op {
    //         pub fn new<V>(view: V) -> Self
    //         where
    //             V: Fn(EntityPath, ListArray) -> Result<(EntityPath, ListArray), Error>
    //                 + Send
    //                 + Sync
    //                 + 'static,y
    //         {
    //             Self {
    //                 view_fn: Arc::new(view),
    //             }
    //         }

    //         /// TODO
    //         pub fn view(
    //             &self,
    //             entity_path: EntityPath,
    //             list_array: ListArray,
    //         ) -> Result<(EntityPath, ListArray), Error> {
    //             (self.view_fn)(entity_path, list_array)
    //         }

    //         pub(super) fn describe(
    //             self,
    //             entity_path: EntityPath,
    //             list_array: ListArray,
    //             descriptor: ComponentDescriptor,
    //         ) -> Result<TransformedColumn, Error> {
    //             let (entity_path, list_array) = self.view(entity_path, list_array)?;
    //             Ok(TransformedColumn {
    //                 entity_path,
    //                 column: SerializedComponentColumn {
    //                     list_array,
    //                     descriptor,
    //                 },
    //                 is_static: false,
    //             })
    //         }

    //         pub fn and_then(self, other: Self) -> Self {
    //             let view_fn = {
    //                 let self_view = Arc::clone(&self.view_fn);
    //                 let other_view = Arc::clone(&other.view_fn);
    //                 move |entity_path, list_array| {
    //                     let (entity_path, list_array) = self_view(entity_path, list_array)?;
    //                     other_view(entity_path, list_array)
    //                 }
    //             };
    //             Self {
    //                 view_fn: Arc::new(view_fn),
    //             }
    //         }
    //     }

    //     /// Extracts a specific field from a struct component within a ListArray.
    //     pub fn access_field(name: impl Into<String>) -> Op {
    //         let name = name.into();
    //         Op::new(move |entity_path, list_array| {
    //             let (_, offsets, values, nulls) = list_array.into_parts();
    //             let struct_array = values
    //                 .as_any()
    //                 .downcast_ref::<StructArray>()
    //                 .ok_or_else(|| Error)?;
    //             let column = struct_array.column_by_name(&name).ok_or_else(|| Error)?;
    //             Ok((
    //                 entity_path.join(&EntityPath::parse_forgiving(&name)),
    //                 ListArray::new(
    //                     Arc::new(Field::new_list_field(column.data_type().clone(), true)),
    //                     offsets,
    //                     column.clone(),
    //                     nulls,
    //                 ),
    //             ))
    //         })
    //     }

    //     /// Casts the inner array of a `ListArray` to a different data type.
    //     pub fn cast(to_inner_type: DataType) -> Op {
    //         Op::new(move |entity_path, list_array| {
    //             let (_, offsets, ref array, nulls) = list_array.into_parts();
    //             let res = compute::cast(array, &to_inner_type).map_err(|_| Error)?;
    //             Ok((
    //                 entity_path,
    //                 ListArray::new(
    //                     Arc::new(Field::new_list_field(res.data_type().clone(), true)),
    //                     offsets,
    //                     res,
    //                     nulls,
    //                 ),
    //             ))
    //         })
    //     }

    //     pub(super) fn nop() -> Op {
    //         Op::new(move |entity_path, list_array| Ok((entity_path, list_array)))
    //     }

    //     #[test]
    //     fn test_op_describe() {
    //         let mut struct_column_builder = ListBuilder::new(StructBuilder::new(
    //             [
    //                 Arc::new(Field::new("a", DataType::Float32, true)),
    //                 Arc::new(Field::new("b", DataType::Float64, true)),
    //             ],
    //             vec![
    //                 Box::new(Float32Builder::new()),
    //                 Box::new(Float64Builder::new()),
    //             ],
    //         ));

    //         // row 0
    //         struct_column_builder
    //             .values()
    //             .field_builder::<Float32Builder>(0)
    //             .unwrap()
    //             .append_value(0.0);
    //         struct_column_builder
    //             .values()
    //             .field_builder::<Float64Builder>(1)
    //             .unwrap()
    //             .append_value(0.0);
    //         struct_column_builder.values().append(true);
    //         struct_column_builder.append(true);

    //         let struct_list_array = struct_column_builder.finish();

    //         let op = access_field("a").and_then(cast(DataType::Float64));
    //         insta::assert_debug_snapshot!(
    //             op.describe(
    //                 EntityPath::parse_forgiving("test"),
    //                 struct_list_array,
    //                 ComponentDescriptor::partial("test")
    //             )
    //             .unwrap()
    //         );
    //     }

    /// Extracts a specific field from a struct component within a ListArray.
    ///
    /// Takes a ListArray containing StructArrays and extracts the specified field,
    /// returning a new ListArray containing only that field's data.
    /// Returns an empty ListArray if the extraction fails.
    pub fn extract_field(list_array: ListArray, column_name: &str) -> Result<ListArray, Error> {
        let (field, offsets, values, nulls) = list_array.into_parts();
        let struct_array = values
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                actual: field.data_type().clone(),
                expected: "StructArray",
            })?;

        let column =
            struct_array
                .column_by_name(column_name)
                .ok_or_else(|| Error::MissingField {
                    expected: column_name.to_owned(),
                    found: struct_array
                        .fields()
                        .iter()
                        .map(|f| f.name().clone())
                        .collect(),
                })?;

        Ok(ListArray::new(
            Arc::new(Field::new_list_field(column.data_type().clone(), true)),
            offsets,
            column.clone(),
            nulls,
        ))
    }

    /// Casts the inner array of a ListArray to a different data type.
    ///
    /// Performs type casting on the component data within the ListArray,
    /// preserving the list structure while changing the inner data type.
    /// Returns an empty ListArray if the cast fails.
    pub fn cast_component_batch(
        list_array: ListArray,
        to_inner_type: &DataType,
    ) -> Result<ListArray, Error> {
        let (_field, offsets, ref array, nulls) = list_array.into_parts();
        let res = compute::cast(array, to_inner_type)?;
        Ok(ListArray::new(
            Arc::new(Field::new_list_field(res.data_type().clone(), true)),
            offsets,
            res,
            nulls,
        ))
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("expected data type `{expected}` but found data type `{actual}`")]
    TypeMismatch {
        actual: DataType,
        expected: &'static str,
    },
    #[error("missing field `{expected}, found {}`", found.join(", "))]
    MissingField {
        expected: String,
        found: Vec<String>,
    },
    #[error(transparent)]
    ArrowError(#[from] ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error>),
}

pub enum Op {
    Cast { data_type: DataType },
    AccessField { field_name: String },
    Func(Box<dyn Fn(ListArray) -> Result<ListArray, Error> + Sync + Send>),
}

impl Op {
    pub fn access_field(field_name: impl Into<String>) -> Self {
        Self::AccessField {
            field_name: field_name.into(),
        }
    }

    pub fn cast(data_type: DataType) -> Self {
        Self::Cast { data_type }
    }

    // TODO: this should be improved
    pub fn constant(value: ListArray) -> Self {
        Self::func(move |_| Ok(value.clone()))
    }

    pub fn func<F>(func: F) -> Self
    where
        F: Fn(ListArray) -> Result<ListArray, Error> + Send + Sync + 'static,
    {
        Self::Func(Box::new(func))
    }
}

impl Op {
    fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        match self {
            Op::Cast { data_type } => cast_component_batch(list_array, data_type),
            Op::AccessField { field_name } => extract_field(list_array, field_name),
            Op::Func(func) => func(list_array),
        }
    }
}

struct InputColumn {
    entity_path_filter: ResolvedEntityPathFilter,
    component: ComponentIdentifier,
}

struct OutputColumn {
    entity_path: EntityPath,
    component_descr: ComponentDescriptor,
    ops: Vec<Op>,
    is_static: bool,
}

pub struct LensBuilder(LensN);

impl LensBuilder {
    pub fn new_for_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> Self {
        Self(LensN {
            input: InputColumn {
                entity_path_filter: entity_path_filter.resolve_without_substitutions(),
                component: component.into(),
            },
            outputs: vec![],
        })
    }

    pub fn add_output_column(
        mut self,
        entity_path: impl Into<EntityPath>,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        let column = OutputColumn {
            entity_path: entity_path.into(),
            component_descr,
            ops: ops.into_iter().collect(),
            is_static: false,
        };
        self.0.outputs.push(column);
        self
    }

    pub fn add_static_output_column(
        mut self,
        entity_path: impl Into<EntityPath>,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        let column = OutputColumn {
            entity_path: entity_path.into(),
            component_descr,
            ops: ops.into_iter().collect(),
            is_static: true,
        };
        self.0.outputs.push(column);
        self
    }

    pub fn build(self) -> LensN {
        self.0
    }
}

pub struct LensN {
    input: InputColumn,
    outputs: Vec<OutputColumn>,
}

impl LensN {
    pub fn for_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> LensBuilder {
        LensBuilder::new_for_column(entity_path_filter, component)
    }
}

impl LensN {
    fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        let found = chunk
            .components()
            .iter()
            .find(|(descr, _array)| descr.component == self.input.component);

        // TODO: This means we drop chunks that belong to the same entity but don't have the component.
        let Some((_component_descr, list_array)) = found else {
            return Default::default();
        };

        let mut builders = ahash::HashMap::default();
        for output in &self.outputs {
            let components = builders
                .entry((output.entity_path.clone(), output.is_static))
                .or_insert_with(ChunkComponents::default);

            if components.contains_component(&output.component_descr) {
                re_log::warn_once!("Replacing duplicated component {}", output.component_descr);
            }

            let mut list_array_result = list_array.clone();
            for op in &output.ops {
                if let Ok(result) = op.call(list_array_result) {
                    list_array_result = result;
                } else {
                    // TODO: context!
                    re_log::error!("failed");
                    return vec![];
                }
            }

            components.insert(output.component_descr.clone(), list_array_result);
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

        let destructure = LensN {
            input: InputColumn::new("nullability".parse().unwrap(), "structs"),
            outputs: vec![OutputColumn {
                entity_path: EntityPath::parse_forgiving("nullability/a"),
                component_descr: Scalars::descriptor_scalars(),
                ops: vec![Op::access_field("a"), Op::cast(DataType::Float64)],
                is_static: false,
            }],
        };
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

        let destructure = LensN {
            input: InputColumn::new("nullability".parse().unwrap(), "structs"),
            outputs: vec![OutputColumn {
                entity_path: EntityPath::parse_forgiving("nullability/b"),
                component_descr: Scalars::descriptor_scalars(),
                ops: vec![Op::access_field("b")],
                is_static: false,
            }],
        };

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

        let count_fn = |list_array: ListArray| {
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

        let count = LensN {
            input: InputColumn::new("nullability".parse().unwrap(), "strings"),
            outputs: vec![
                OutputColumn {
                    entity_path: EntityPath::parse_forgiving("nullability/b_count"),
                    component_descr: ComponentDescriptor::partial("counts"),
                    ops: vec![Op::func(count_fn)],
                    is_static: false,
                },
                OutputColumn {
                    entity_path: EntityPath::parse_forgiving("nullability/b_count"),
                    component_descr: ComponentDescriptor::partial("original"),
                    ops: vec![],
                    is_static: false,
                },
            ],
        };

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

        let static_fn_a = |_| {
            let mut metadata_builder_a = ListBuilder::new(StringBuilder::new());
            metadata_builder_a
                .values()
                .append_value("static_metadata_a");
            metadata_builder_a.append(true);
            Ok(metadata_builder_a.finish())
        };

        let static_fn_b = |_| {
            let mut metadata_builder_b = ListBuilder::new(StringBuilder::new());
            metadata_builder_b
                .values()
                .append_value("static_metadata_b");
            metadata_builder_b.append(true);
            Ok(metadata_builder_b.finish())
        };

        let static_lens = LensN {
            input: InputColumn::new("nullability".parse().unwrap(), "strings"),
            outputs: vec![
                OutputColumn {
                    entity_path: EntityPath::parse_forgiving("nullability/static"),
                    component_descr: ComponentDescriptor::partial("static_metadata_a"),
                    ops: vec![Op::func(static_fn_a)],
                    is_static: true,
                },
                OutputColumn {
                    entity_path: EntityPath::parse_forgiving("nullability/static"),
                    component_descr: ComponentDescriptor::partial("static_metadata_b"),
                    ops: vec![Op::func(static_fn_b)],
                    is_static: true,
                },
            ],
        };

        let pipeline = LensRegistry {
            lenses: vec![static_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("single_static", format!("{chunk:-240}"));
    }
}
