use std::{collections::HashMap, sync::Arc};

use arrow::{
    array::{
        Array, Float32Array, Float32Builder, Float64Array, Float64Builder, ListArray, ListBuilder,
        StringArray, StringBuilder, StructArray, StructBuilder,
    },
    datatypes::{DataType, Field},
    ipc::ListArgs,
};
use rerun::{
    ComponentDescriptor, ComponentIdentifier, DynamicArchetype, EntityPath, RecordingStream,
    Scalars, SerializedComponentColumn, SeriesLines, SeriesPoints, TextDocument, TimeCell,
    TimeColumn,
    dataframe::{EntityPathFilter, ResolvedEntityPathFilter, TimelineName},
    external::re_log,
    log::{Chunk, ChunkComponents, ChunkId, LogMsg},
    sink::{GrpcSink, PipelineTransform},
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The filepaths to be loaded and logged.
    filepaths: Vec<std::path::PathBuf>,
}

// TODO: Is this the right API.
type ChunkFunc = Box<dyn Fn(&Chunk) -> Vec<Chunk> + Send + Sync>;

pub struct PerChunkTransform {
    /// The entity path to apply the transformation to.
    pub filter: ResolvedEntityPathFilter,

    /// A closure that outputs a list of chunks
    pub func: ChunkFunc,
}

pub struct PerChunkPiplineTransform {
    transforms: Vec<PerChunkTransform>,
}

impl PipelineTransform for PerChunkPiplineTransform {
    fn apply(&self, msg: LogMsg) -> Vec<LogMsg> {
        match &msg {
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {
                vec![msg]
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => match Chunk::from_arrow_msg(arrow_msg) {
                Ok(chunk) => {
                    let mut relevant = self
                        .transforms
                        .iter()
                        .filter(|transform| transform.filter.matches(chunk.entity_path()))
                        .peekable();
                    if relevant.peek().is_some() {
                        relevant
                            .flat_map(|transform| (*transform.func)(&chunk))
                            .filter_map(|transformed| match transformed.to_arrow_msg() {
                                Ok(arrow_msg) => {
                                    Some(LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
                                }
                                Err(err) => {
                                    re_log::error_once!(
                                        "failed to create log message from chunk: {err}"
                                    );
                                    None
                                }
                            })
                            .collect()
                    } else {
                        vec![msg]
                    }
                }

                Err(err) => {
                    re_log::error_once!("Failed to convert arrow message to chunk: {err}");
                    vec![msg]
                }
            },
        }
    }
}

fn extract_field(list_array: ListArray, column_name: &str) -> ListArray {
    let (_, offsets, values, nulls) = list_array.into_parts();
    let struct_array = values.as_any().downcast_ref::<StructArray>().unwrap();
    let column = struct_array.column_by_name(column_name).unwrap();
    ListArray::new(
        Arc::new(Field::new_list_field(column.data_type().clone(), true)),
        offsets,
        column.clone(),
        nulls,
    )
}

fn cast_component_batch(list_array: ListArray, to_inner_type: &DataType) -> ListArray {
    let (field, offsets, ref array, nulls) = list_array.into_parts();
    let res = arrow::compute::cast(array, to_inner_type).unwrap();
    ListArray::new(
        Arc::new(Field::new_list_field(res.data_type().clone(), true)),
        offsets,
        res,
        nulls,
    )
}

// TODO: This looks like a weird love-child between `SerializedComponentColumn` and `ComponentColumnDescriptor`.
struct TransformedColumn {
    entity_path: EntityPath,
    column: SerializedComponentColumn,
    is_static: bool,
}

impl TransformedColumn {
    pub fn new(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: false,
        }
    }
    pub fn new_static(entity_path: EntityPath, column: SerializedComponentColumn) -> Self {
        Self {
            entity_path,
            column,
            is_static: true,
        }
    }
}

type ComponentBatchFunc =
    Box<dyn Fn(ListArray, &EntityPath) -> Vec<TransformedColumn> + Send + Sync>;

pub struct ComponentBatchTransform {
    /// The entity path to apply the transformation to.
    pub filter: ResolvedEntityPathFilter,

    /// The component that we want to select.
    pub component: ComponentIdentifier,

    /// A closure that outputs a list of chunks
    pub func: ComponentBatchFunc,
}

pub struct ComponentBatchPipelineTransform {
    transforms: Vec<ComponentBatchTransform>,
}

impl ComponentBatchTransform {
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
}

fn apply_to_chunk(transform: &ComponentBatchTransform, chunk: &Chunk) -> Vec<Chunk> {
    let found = chunk
        .components()
        .iter()
        .find(|(descr, _array)| descr.component == transform.component);

    // TODO: This means we drop chunks that belong to the same entity but don't have the component.
    let Some((_component_descr, list_array)) = found else {
        return Default::default();
    };

    // TODO:
    // * unwrap array
    // * Guarantee that there is only one component descr
    let mut builders = HashMap::new(); // TODO: Use ahash
    let results = (transform.func)(list_array.clone(), chunk.entity_path());
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
                    )
                })
                .ok()
        })
        .collect()
}

impl PipelineTransform for ComponentBatchPipelineTransform {
    fn apply(&self, msg: LogMsg) -> Vec<LogMsg> {
        match &msg {
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {
                vec![msg]
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => match Chunk::from_arrow_msg(arrow_msg) {
                Ok(chunk) => {
                    let mut relevant = self
                        .transforms
                        .iter()
                        .filter(|transform| transform.filter.matches(chunk.entity_path()))
                        .peekable();
                    if relevant.peek().is_some() {
                        relevant
                            .flat_map(|transform| apply_to_chunk(transform, &chunk))
                            .filter_map(|transformed| match transformed.to_arrow_msg() {
                                Ok(arrow_msg) => {
                                    Some(LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
                                }
                                Err(err) => {
                                    re_log::error_once!(
                                        "failed to create log message from chunk: {err}"
                                    );
                                    None
                                }
                            })
                            .collect()
                    } else {
                        vec![msg]
                    }
                }

                Err(err) => {
                    re_log::error_once!("Failed to convert arrow message to chunk: {err}");
                    vec![msg]
                }
            },
        }
    }
}

fn per_column_pipline() -> anyhow::Result<impl PipelineTransform> {
    let instruction_transform = ComponentBatchTransform::new(
        "/instructions".parse()?,
        "com.Example.Instruction:text",
        |array, entity_path| {
            vec![TransformedColumn {
                entity_path: entity_path.clone(),
                column: SerializedComponentColumn {
                    descriptor: TextDocument::descriptor_text(),
                    list_array: array,
                },
                is_static: false,
            }]
        },
    );

    let destructure_transform = ComponentBatchTransform::new(
        "/nested".parse().unwrap(),
        "com.Example.Nested:payload",
        |array, entity_path| {
            let list_array_a = extract_field(array.clone(), "a");
            let list_array_a = cast_component_batch(list_array_a, &DataType::Float64);

            let list_array_b = extract_field(array, "b");

            vec![
                TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("a")),
                    SerializedComponentColumn {
                        descriptor: Scalars::descriptor_scalars(),
                        list_array: list_array_a,
                    },
                ),
                TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("b")),
                    SerializedComponentColumn {
                        descriptor: Scalars::descriptor_scalars(),
                        list_array: list_array_b,
                    },
                ),
            ]
        },
    );

    let flag_transform = ComponentBatchTransform::new(
        "/flag".parse()?,
        "com.Example.Flag:flag",
        |list_array, entity_path| {
            let (_, offsets, values, nulls) = list_array.into_parts();
            let flag_array = values.as_any().downcast_ref::<StringArray>().unwrap();

            let scalar_array: Float64Array = flag_array
                .iter()
                .map(|s| {
                    s.map(|v| match v {
                        "ACTIVE" => 1.0,
                        "INACTIVE" => 2.0,
                        _ => 0.0,
                    })
                })
                .collect();

            let list_array = ListArray::new(
                Arc::new(Field::new_list_field(
                    scalar_array.data_type().clone(),
                    true,
                )),
                offsets,
                Arc::new(scalar_array),
                nulls,
            );

            let series_points = SeriesPoints::new()
                .with_marker_sizes([5.0])
                .columns_of_unit_batches()
                .unwrap()
                .next()
                .unwrap();

            let series_lines = SeriesLines::new()
                .with_widths([3.0])
                .columns_of_unit_batches()
                .unwrap()
                .next()
                .unwrap();

            vec![
                TransformedColumn::new(
                    entity_path.clone(),
                    SerializedComponentColumn {
                        list_array,
                        descriptor: Scalars::descriptor_scalars(),
                    },
                ),
                TransformedColumn::new_static(entity_path.clone(), series_points),
                TransformedColumn::new_static(entity_path.clone(), series_lines),
            ]
        },
    );

    Ok(ComponentBatchPipelineTransform {
        transforms: vec![instruction_transform, destructure_transform, flag_transform],
    })
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // let transform = per_chunk_pipeline()?.to_sink(GrpcSink::default());
    let transform = per_column_pipline()?.to_sink(GrpcSink::default());

    let (rec, _serve_guard) = args.rerun.init("rerun_example_transform")?;
    // TODO: There should be a way to do this in one go.
    rec.set_sink(Box::new(transform));
    run(&rec, &args)?;

    Ok(())
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    let prefix = Some("log_file_example".into());

    if args.filepaths.is_empty() {
        log_instructions(rec)?;
        log_structs_with_scalars(rec)?;
        log_flag(rec)?;
        log_columns_with_nullability(rec)?;
        return Ok(());
    }

    for filepath in &args.filepaths {
        let filepath = filepath.as_path();

        // …or using its contents if you already have them loaded for some reason.
        if filepath.is_file() {
            let contents = std::fs::read(filepath)?;
            rec.log_file_from_contents(
                filepath,
                std::borrow::Cow::Borrowed(&contents),
                prefix.clone(),
                true, /* static */
            )?;
        }
    }

    Ok(())
}

fn log_flag(rec: &RecordingStream) -> anyhow::Result<()> {
    let flags = ["ACTIVE", "ACTIVE", "INACTIVE", "UNKNOWN"];
    for x in 0..10i64 {
        let flag = StringArray::from(vec![flags[x as usize % flags.len()]]);
        rec.set_time("tick", TimeCell::from_sequence(x));
        rec.log(
            "flag",
            &DynamicArchetype::new("com.Example.Flag")
                .with_component_from_data("flag", Arc::new(flag)),
        )?
    }

    Ok(())
}

fn log_instructions(rec: &RecordingStream) -> anyhow::Result<()> {
    rec.set_time("tick", TimeCell::from_sequence(1));
    rec.log(
        "instructions",
        &DynamicArchetype::new("com.Example.Instruction").with_component_from_data(
            "text",
            Arc::new(arrow::array::StringArray::from(vec![
                "This is a nice instruction text.",
            ])),
        ),
    )?;

    Ok(())
}

fn log_structs_with_scalars(rec: &RecordingStream) -> anyhow::Result<()> {
    for x in 0..10i64 {
        let a = Float32Array::from(vec![1.0 * x as f32, 2.0 + x as f32, 3.0 + x as f32]);
        let b = Float64Array::from(vec![5.0 * x as f64, 6.0 + x as f64, 7.0 + x as f64]);

        let struct_array = StructArray::from(vec![
            (
                Arc::new(Field::new("a", DataType::Float32, false)),
                Arc::new(a) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("b", DataType::Float64, false)),
                Arc::new(b) as Arc<dyn arrow::array::Array>,
            ),
        ]);
        rec.set_time("tick", TimeCell::from_sequence(x));
        rec.log(
            "nested",
            &DynamicArchetype::new("com.Example.Nested")
                .with_component_from_data("payload", Arc::new(struct_array)),
        )?
    }

    Ok(())
}

fn log_columns_with_nullability(rec: &RecordingStream) -> anyhow::Result<()> {
    let chunk = nullability_chunk();
    rec.send_chunk(chunk);
    Ok(())
}

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
        [(TimelineName::new("tick"), time_column)]
            .into_iter()
            .collect(),
        components.collect(),
    )
    .unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use arrow::array::{FixedSizeListBuilder, Int32Builder};
    use rerun::{
        StoreId,
        external::re_format_arrow::{self, RecordBatchFormatOpts},
    };

    const FORMAT_OPTS: RecordBatchFormatOpts = RecordBatchFormatOpts {
        transposed: false,
        width: Some(240usize),
        include_metadata: true,
        include_column_metadata: true,
        trim_field_names: true,
        trim_metadata_keys: true,
        trim_metadata_values: true,
        redact_non_deterministic: true,
    };

    #[test]
    fn test_destructure_cast() {
        let chunk = nullability_chunk();
        println!("{chunk}");
        let arrow_msg = nullability_chunk().to_arrow_msg().unwrap();
        let msg = LogMsg::ArrowMsg(StoreId::empty_recording(), arrow_msg);

        let destructure_transform = ComponentBatchTransform::new(
            "nullability".parse().unwrap(),
            "structs",
            |list_array, entity_path| {
                let list_array = extract_field(list_array, "a");
                let list_array = cast_component_batch(list_array, &DataType::Float64);

                vec![TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("a")),
                    SerializedComponentColumn {
                        list_array,
                        descriptor: Scalars::descriptor_scalars(),
                    },
                )]
            },
        );

        let pipeline = ComponentBatchPipelineTransform {
            transforms: vec![destructure_transform],
        };

        let mut res = pipeline.apply(msg.clone());
        assert_eq!(res.len(), 1);

        let transformed_batch = res[0].arrow_record_batch_mut().unwrap();
        insta::assert_snapshot!(
            "destructure_cast",
            re_format_arrow::format_record_batch_opts(transformed_batch, &FORMAT_OPTS,)
        );
    }

    #[test]
    fn test_destructure() {
        let chunk = nullability_chunk();
        println!("{chunk}");
        let arrow_msg = nullability_chunk().to_arrow_msg().unwrap();
        let msg = LogMsg::ArrowMsg(StoreId::empty_recording(), arrow_msg);

        let destructure_transform = ComponentBatchTransform::new(
            "nullability".parse().unwrap(),
            "structs",
            |list_array, entity_path| {
                let list_array = extract_field(list_array, "b");

                vec![TransformedColumn::new(
                    entity_path.join(&EntityPath::parse_forgiving("b")),
                    SerializedComponentColumn {
                        list_array,
                        descriptor: Scalars::descriptor_scalars(),
                    },
                )]
            },
        );

        let pipeline = ComponentBatchPipelineTransform {
            transforms: vec![destructure_transform],
        };

        let mut res = pipeline.apply(msg);
        assert_eq!(res.len(), 1);

        let transformed_batch = res[0].arrow_record_batch_mut().unwrap();
        insta::assert_snapshot!(
            "destructure_only",
            re_format_arrow::format_record_batch_opts(transformed_batch, &FORMAT_OPTS,)
        )
    }

    #[test]
    fn test_inner_count() {
        let chunk = nullability_chunk();
        println!("{chunk}");
        let arrow_msg = nullability_chunk().to_arrow_msg().unwrap();
        let msg = LogMsg::ArrowMsg(StoreId::empty_recording(), arrow_msg);

        let count_transform = ComponentBatchTransform::new(
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

        let pipeline = ComponentBatchPipelineTransform {
            transforms: vec![count_transform],
        };

        let mut res = pipeline.apply(msg);
        assert_eq!(res.len(), 1);

        let transformed_batch = res[0].arrow_record_batch_mut().unwrap();
        insta::assert_snapshot!(
            "inner_count",
            re_format_arrow::format_record_batch_opts(transformed_batch, &FORMAT_OPTS,)
        )
    }
}
