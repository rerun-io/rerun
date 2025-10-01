use re_chunk::{
    Chunk, ChunkComponents, ChunkId, ComponentIdentifier, EntityPath,
    external::arrow::array::ListArray,
};
use re_log_types::{EntityPathFilter, LogMsg, ResolvedEntityPathFilter};
use re_types::SerializedComponentColumn;

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

/// TODO: Better defintions + pub / private distinctions.
pub struct TransformedColumn {
    /// TODO
    pub entity_path: EntityPath,
    /// TODO
    pub column: SerializedComponentColumn,
    /// TODO
    pub is_static: bool,
}

impl TransformedColumn {
    /// TODO
    pub fn new(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: false,
        }
    }

    /// TODO
    pub fn new_static(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: true,
        }
    }
}

type LensFunc = Box<dyn Fn(ListArray, &EntityPath) -> Vec<TransformedColumn> + Send + Sync>;

/// TODO
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

    /// TODO: This will drop component columns that are not relevant.
    ///
    /// Retaining some of the original data could be done via idenity lenses, or via multi sinks.
    pub fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        self.relevant(chunk)
            .flat_map(|transform| transform.apply(chunk))
            .collect()
    }
}

impl Lens {
    /// TODO
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

    use re_chunk::external::arrow::{
        array::{ListArray, StructArray},
        compute,
        datatypes::{DataType, Field},
    };

    /// TODO
    pub fn extract_field(list_array: ListArray, column_name: &str) -> ListArray {
        let (_field, offsets, values, nulls) = list_array.into_parts();
        let struct_array = values.as_any().downcast_ref::<StructArray>().unwrap();
        let column = struct_array.column_by_name(column_name).unwrap();
        ListArray::new(
            Arc::new(Field::new_list_field(column.data_type().clone(), true)),
            offsets,
            column.clone(),
            nulls,
        )
    }

    /// TODO
    pub fn cast_component_batch(list_array: ListArray, to_inner_type: &DataType) -> ListArray {
        let (_field, offsets, ref array, nulls) = list_array.into_parts();
        let res = compute::cast(array, to_inner_type).unwrap();
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
}
