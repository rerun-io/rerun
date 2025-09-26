use std::sync::Arc;

use arrow::{
    array::{Array, Float32Array, Float64Array, ListArray, StructArray},
    datatypes::{DataType, Field},
};
use rerun::{
    ComponentDescriptor, DynamicArchetype, RecordingStream, Scalars, TextDocument, TimeCell,
    dataframe::{EntityPathFilter, ResolvedEntityPathFilter},
    external::re_log,
    log::{Chunk, ChunkId, LogMsg},
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

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

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

    let gello_a_transform = PerChunkTransform {
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

    let transform = PerChunkPiplineTransform {
        transforms: vec![instruction_transform, gello_a_transform],
    }
    .to_sink(GrpcSink::default());

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
    for x in 0..10 {
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
