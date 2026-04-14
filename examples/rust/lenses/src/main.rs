use std::sync::Arc;

use arrow::array::{
    ArrayBuilder, Float32Array, Float64Array, Int64Builder, ListBuilder, StringBuilder, StructArray,
};
use arrow::datatypes::{DataType, Field};
use rerun::external::re_log;
use rerun::lenses::{Lens, LensesSink, Selector, op};
use rerun::sink::GrpcSink;
use rerun::{
    ComponentDescriptor, DynamicArchetype, RecordingStream, Scalars, SerializedComponentColumn,
    TextDocument, TimeCell,
};

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let instruction = Lens::for_input_column("/instructions".parse()?, "example:Instruction:text")
        .output_columns(|out| {
            out.component(TextDocument::descriptor_text(), Selector::parse(".")?)
        })?
        .build();

    let destructure = Lens::for_input_column("/nested".parse()?, "example:Nested:payload")
        .output_columns(|out| {
            out.at_entity("nested/a").component(
                Scalars::descriptor_scalars(),
                Selector::parse(".a")?.pipe(op::cast(DataType::Float64)),
            )
        })?
        .output_columns(|out| {
            out.at_entity("nested/b")
                .component(Scalars::descriptor_scalars(), Selector::parse(".b")?)
        })?
        .build();

    let time = Lens::for_input_column("/timestamped".parse()?, "my_timestamp")
        .output_columns(|out| {
            out.time(
                "my_timeline",
                rerun::time::TimeType::Sequence,
                Selector::parse(".")?,
            )?
            .component(ComponentDescriptor::partial("value"), Selector::parse(".")?)
        })?
        .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(instruction)
        .with_lens(destructure)
        .with_lens(time);

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_lenses").spawn()?;
    rec.set_sink(Box::new(lenses_sink));

    log_instructions(&rec)?;
    log_structs_with_scalars(&rec)?;
    log_timestamps(&rec)?;

    Ok(())
}

// Logging helpers

fn log_instructions(rec: &RecordingStream) -> anyhow::Result<()> {
    rec.set_time("tick", TimeCell::from_sequence(1));
    rec.log(
        "instructions",
        &DynamicArchetype::new("example:Instruction").with_component_from_data(
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
            &DynamicArchetype::new("example:Nested")
                .with_component_from_data("payload", Arc::new(struct_array)),
        )?
    }

    Ok(())
}

fn log_timestamps(rec: &RecordingStream) -> anyhow::Result<()> {
    let mut timestamp_list_builder = ListBuilder::new(Int64Builder::new());
    let mut string_list_builder = ListBuilder::new(StringBuilder::new());

    for x in 42..53 {
        timestamp_list_builder
            .values()
            .as_any_mut()
            .downcast_mut::<Int64Builder>()
            .unwrap()
            .append_value(x);
        timestamp_list_builder.append(true);

        string_list_builder
            .values()
            .as_any_mut()
            .downcast_mut::<StringBuilder>()
            .unwrap()
            .append_value(format!("value: {x}"));
        string_list_builder.append(true);
    }

    rec.send_columns(
        "timestamped",
        [],
        [
            SerializedComponentColumn {
                descriptor: rerun::ComponentDescriptor::partial("my_timestamp"),
                list_array: timestamp_list_builder.finish(),
            },
            SerializedComponentColumn {
                descriptor: rerun::ComponentDescriptor::partial("value"),
                list_array: string_list_builder.finish(),
            },
        ],
    )?;

    Ok(())
}
