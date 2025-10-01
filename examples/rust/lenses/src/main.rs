use std::sync::Arc;

use arrow::{
    array::{Array, Float32Array, Float64Array, ListArray, StringArray, StructArray},
    datatypes::{DataType, Field},
};
use rerun::{
    DynamicArchetype, EntityPath, RecordingStream, Scalars, SerializedComponentColumn, SeriesLines,
    SeriesPoints, TextDocument, TimeCell,
    external::re_log,
    lenses::{Lens, LensesSink, TransformedColumn, op},
    sink::GrpcSink,
};

fn lens_instruction() -> anyhow::Result<Lens> {
    Ok(Lens::new(
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
    ))
}

fn lens_destructure() -> anyhow::Result<Lens> {
    Ok(Lens::new(
        "/nested".parse().unwrap(),
        "com.Example.Nested:payload",
        |array, entity_path| {
            let list_array_a = op::extract_field(array.clone(), "a");
            let list_array_a = op::cast_component_batch(list_array_a, &DataType::Float64);

            let list_array_b = op::extract_field(array, "b");

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
    ))
}

fn lens_flag() -> anyhow::Result<Lens> {
    Ok(Lens::new(
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
    ))
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(lens_instruction()?)
        .with_lens(lens_destructure()?)
        .with_lens(lens_flag()?);

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_lenses").spawn()?;
    rec.set_sink(Box::new(lenses_sink));

    log_instructions(&rec)?;
    log_structs_with_scalars(&rec)?;
    log_flag(&rec)?;

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
