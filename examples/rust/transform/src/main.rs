use std::sync::Arc;

use arrow::{
    array::{Array, Float32Array, Float64Array, ListArray, StringArray, StructArray},
    datatypes::{DataType, Field},
};
use rerun::{
    ComponentDescriptor, ComponentIdentifier, DynamicArchetype, EntityPath, RecordingStream,
    Scalars, SeriesLines, SeriesPoints, TextDocument, TimeCell,
    dataframe::{EntityPathFilter, ResolvedEntityPathFilter},
    external::{nohash_hasher::IntMap, re_log},
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

type ComponentBatchFunc = Box<
    dyn Fn(Arc<dyn Array>, &EntityPath) -> Vec<(EntityPath, ComponentDescriptor, Arc<dyn Array>)>
        + Send
        + Sync,
>;

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
        F: Fn(
                Arc<dyn Array>,
                &EntityPath,
            ) -> Vec<(EntityPath, ComponentDescriptor, Arc<dyn Array>)>
            + Send
            + Sync
            + 'static,
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
    let Some((_component_descr, outer_array)) = found else {
        return Default::default();
    };

    let inner_array = outer_array.values();

    // TODO:
    // * unwrap array
    // * Guarantee that there is only one component descr
    let mut builders = IntMap::default();
    let results = (transform.func)(inner_array.clone(), chunk.entity_path());
    for (entity_path, component_descr, new_array) in results {
        let components = builders
            .entry(entity_path)
            .or_insert_with(ChunkComponents::default);

        if components.contains_component(&component_descr) {
            re_log::warn_once!(
                "Replacing duplicated component {}",
                component_descr.component
            );
        }

        components.insert(
            component_descr,
            ListArray::new(
                Field::new_list_field(new_array.data_type().clone(), true).into(),
                outer_array.offsets().clone(),
                // TODO: box from the start
                new_array.into(),
                outer_array.nulls().cloned(),
            ),
        );
    }

    builders
        .into_iter()
        .filter_map(|(entity_path, components)| {
            Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                chunk.timelines().clone(),
                components,
            )
            .inspect_err(|err| {
                re_log::error_once!("Failed to build chunk at entity path '{entity_path}': {err}")
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

fn per_chunk_pipeline() -> anyhow::Result<impl PipelineTransform> {
    let instruction_transform = PerChunkTransform {
        filter: "/instructions"
            .parse::<EntityPathFilter>()?
            .resolve_without_substitutions(), // TODO: call the right thing here.
        func: Box::new(|chunk: &rerun::log::Chunk| {
            let mut components = chunk.components().clone();

            let maybe_array = components
                .get(&ComponentDescriptor {
                    archetype: Some("com.Example.Instruction".into()),
                    component: "com.Example.Instruction:text".into(),
                    component_type: None,
                })
                .cloned();
            if let Some(array) = maybe_array {
                components.insert(TextDocument::descriptor_text(), array);
            }

            let mut new_chunk = chunk.clone().components_removed().with_id(ChunkId::new());
            for (component_descr, array) in components.iter() {
                new_chunk
                    .add_component(component_descr.clone(), array.clone())
                    .unwrap();
            }
            vec![new_chunk]
        }),
    };

    let destructure_transform = PerChunkTransform {
        filter: "/nested"
            .parse::<EntityPathFilter>()?
            .resolve_without_substitutions(), // TODO: call the right thing here.
        func: Box::new(|chunk: &rerun::log::Chunk| {
            let mut components = chunk.components().clone();

            let maybe_array = components
                .get(&ComponentDescriptor {
                    archetype: Some("com.Example.Nested".into()),
                    component: "com.Example.Nested:payload".into(),
                    component_type: None,
                })
                .cloned();

            if let Some(list_struct_array) = maybe_array {
                let list_array = list_struct_array
                    .as_any()
                    .downcast_ref::<ListArray>()
                    .unwrap();

                let struct_array = list_array
                    .values()
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .unwrap();

                let child_b_array = struct_array.column_by_name("b").unwrap();

                let field = Arc::new(Field::new_list_field(
                    child_b_array.data_type().clone(),
                    true,
                ));

                let new_list_array = ListArray::new(
                    field,
                    list_array.offsets().clone(), // Use ListArray's offsets
                    child_b_array.clone(),        // Values from field "b"
                    list_array.nulls().cloned(),  // Preserve null mask
                );

                components.insert(Scalars::descriptor_scalars(), new_list_array);
            }

            let mut new_chunk = chunk.clone().components_removed().with_id(ChunkId::new());
            for (component_descr, array) in components.iter() {
                new_chunk
                    .add_component(component_descr.clone(), array.clone())
                    .unwrap();
            }
            vec![new_chunk]
        }),
    };

    Ok(PerChunkPiplineTransform {
        transforms: vec![instruction_transform, destructure_transform],
    })
}

fn per_column_pipline() -> anyhow::Result<impl PipelineTransform> {
    // Takes an existing component that has the right backing data and apply a new component descriptor too it.
    // TODO: For these simple cases, we could have premade constructors that hide the closure. This could also lead to more efficient Python mappings.
    let instruction_transform = ComponentBatchTransform::new(
        "/instructions".parse()?,
        "com.Example.Instruction:text",
        |array, entity_path| vec![(entity_path.clone(), TextDocument::descriptor_text(), array)],
    );

    // Extracts two fields from a struct, and adds them to new sub-entities as scalars.
    let destructure_transform = ComponentBatchTransform::new(
        "/nested".parse()?,
        "com.Example.Nested:payload",
        |array, entity_path| {
            let struct_array = array.as_any().downcast_ref::<StructArray>().unwrap();

            let child_a_array = struct_array.column_by_name("a").unwrap();
            let child_a_array = arrow::compute::cast(child_a_array, &DataType::Float64).unwrap();

            let child_b_array = struct_array.column_by_name("b").unwrap();

            vec![
                (
                    entity_path.join(&EntityPath::parse_forgiving("a")),
                    Scalars::descriptor_scalars(),
                    child_a_array,
                ),
                (
                    entity_path.join(&EntityPath::parse_forgiving("b")),
                    Scalars::descriptor_scalars(),
                    child_b_array.clone(),
                ),
            ]
        },
    );

    let flag_transform = ComponentBatchTransform::new(
        "/flag".parse()?,
        "com.Example.Flag:flag",
        |array, entity_path| {
            let flag_array = array.as_any().downcast_ref::<StringArray>().unwrap();

            let scalar_array: Float64Array = flag_array
                .iter()
                .map(|s| {
                    s.map(|v| match v {
                        "ACTIVE" => 1.0,
                        "INACTIVE" => 2.0,
                        _ => f64::NAN,
                        // _ => 0.0,
                    })
                })
                .collect();

            vec![
                (
                    entity_path.clone(),
                    Scalars::descriptor_scalars(),
                    Arc::new(scalar_array),
                ),
                // TODO: Very sad that we need to log this multiple times. We need static chunks without timelines.
                (
                    entity_path.clone(),
                    SeriesPoints::descriptor_marker_sizes(),
                    Arc::new(Float32Array::from(vec![5.0; 10])),
                ),
                (
                    entity_path.clone(),
                    SeriesLines::descriptor_widths(),
                    Arc::new(Float32Array::from(vec![3.0; 10])),
                ),
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
