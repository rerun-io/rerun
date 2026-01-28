use std::sync::Arc;

use arrow::array::{
    Array, ArrayBuilder, Float32Array, Float64Array, Int64Builder, ListArray, ListBuilder,
    StringArray, StringBuilder, StructArray,
};
use arrow::datatypes::{DataType, Field};
use rerun::external::re_log;
use rerun::lenses::{Lens, LensesSink, Op};
use rerun::sink::GrpcSink;
use rerun::{
    ComponentDescriptor, DynamicArchetype, RecordingStream, Scalars, SerializedComponentColumn,
    SeriesLines, SeriesPoints, TextDocument, TimeCell,
};

fn lens_flag() -> anyhow::Result<Lens> {
    let step_fn = |list_array: &ListArray| {
        let (_, offsets, values, nulls) = list_array.clone().into_parts();
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

        Ok(ListArray::new(
            Arc::new(Field::new_list_field(
                scalar_array.data_type().clone(),
                true,
            )),
            offsets,
            Arc::new(scalar_array),
            nulls,
        ))
    };

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

    let lens = Lens::for_input_column("/flag".parse()?, "example:Flag:flag")
        .output_columns(|out| out.component(Scalars::descriptor_scalars(), [Op::func(step_fn)]))?
        .output_static_columns_at("/flag", |out| {
            out.component(
                series_points.descriptor,
                [Op::constant(series_points.list_array)],
            )
            .component(
                series_lines.descriptor,
                [Op::constant(series_lines.list_array)],
            )
        })?
        .build();

    Ok(lens)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let instruction = Lens::for_input_column("/instructions".parse()?, "example:Instruction:text")
        .output_columns(|out| out.component(TextDocument::descriptor_text(), []))?
        .build();

    let destructure = Lens::for_input_column("/nested".parse()?, "example:Nested:payload")
        .output_columns_at("nested/a", |out| {
            out.component(
                Scalars::descriptor_scalars(),
                [Op::selector(".a"), Op::cast(DataType::Float64)],
            )
        })?
        .output_columns_at("nested/b", |out| {
            out.component(Scalars::descriptor_scalars(), [Op::selector(".b")])
        })?
        .build();

    let time = Lens::for_input_column("/timestamped".parse()?, "my_timestamp")
        .output_columns(|out| {
            out.time("my_timeline", rerun::time::TimeType::Sequence, [])
                .component(ComponentDescriptor::partial("value"), [])
        })?
        .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(instruction)
        .with_lens(destructure)
        .with_lens(lens_flag()?)
        .with_lens(time);

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_lenses").spawn()?;
    rec.set_sink(Box::new(lenses_sink));

    log_instructions(&rec)?;
    log_structs_with_scalars(&rec)?;
    log_flag(&rec)?;
    log_timestamps(&rec)?;

    Ok(())
}

// Logging helpers

fn log_flag(rec: &RecordingStream) -> anyhow::Result<()> {
    let flags = ["ACTIVE", "ACTIVE", "INACTIVE", "UNKNOWN"];
    for x in 0..10i64 {
        let flag = StringArray::from(vec![flags[x as usize % flags.len()]]);
        rec.set_time("tick", TimeCell::from_sequence(x));
        rec.log(
            "flag",
            &DynamicArchetype::new("example:Flag").with_component_from_data("flag", Arc::new(flag)),
        )?
    }

    Ok(())
}

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
