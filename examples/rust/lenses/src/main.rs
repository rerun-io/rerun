use std::sync::Arc;

use arrow::{
    array::{Array, Float32Array, Float64Array, ListArray, StringArray, StructArray},
    datatypes::{DataType, Field},
};
use rerun::{
    DynamicArchetype, RecordingStream, Scalars, SeriesLines, SeriesPoints, TextDocument, TimeCell,
    external::re_log,
    lenses::{Lens, LensesSink, Op},
    sink::GrpcSink,
};

fn lens_flag() -> anyhow::Result<Lens> {
    let step_fn = |list_array: ListArray| {
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

    let lens = Lens::input_column("/flag".parse()?, "com.Example.Flag:flag")
        .output_column("/flag", Scalars::descriptor_scalars(), [Op::func(step_fn)])
        .static_output_column(
            "/flag",
            series_points.descriptor,
            [Op::constant(series_points.list_array)],
        )
        .static_output_column(
            "/flag",
            series_lines.descriptor,
            [Op::constant(series_lines.list_array)],
        )
        .build();

    Ok(lens)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let instruction = Lens::input_column("/instructions".parse()?, "com.Example.Instruction:text")
        .output_column("instructions", TextDocument::descriptor_text(), [])
        .build();

    let destructure = Lens::input_column("/nested".parse()?, "com.Example.Nested:payload")
        .output_column(
            "nested/a",
            Scalars::descriptor_scalars(),
            [Op::access_field("a"), Op::cast(DataType::Float64)],
        )
        .output_column(
            "nested/b",
            Scalars::descriptor_scalars(),
            [Op::access_field("b")],
        )
        .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(instruction)
        .with_lens(destructure)
        .with_lens(lens_flag()?);

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_lenses").spawn()?;
    rec.set_sink(Box::new(lenses_sink));

    log_instructions(&rec)?;
    log_structs_with_scalars(&rec)?;
    log_flag(&rec)?;

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
