use std::sync::Arc;

use arrow::{
    array::{
        Array, BinaryArray, FixedSizeListBuilder, Float32Builder, Float64Array, ListArray,
        ListBuilder, StructArray, UInt8Array,
    },
    buffer::OffsetBuffer,
    datatypes::{DataType, Field, Fields},
};
use rerun::{
    EncodedImage, InstancePoses3D, Points3D,
    dataframe::EntityPathFilter,
    external::re_log,
    lenses::{Error, Lens, LensBuilder, LensesSink, Op},
    sink::GrpcSink,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The path to the MCAP file.
    filepath: std::path::PathBuf,
}

// TODO: This should be builtin.
fn list_binary_to_list_uint8(input: ListArray) -> Result<ListArray, Error> {
    // Extract the values array and cast to BinaryArray
    let binary_values = input
        .values()
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: input.value_type(),
            expected: DataType::Binary,
        })?;

    // Build new nested structure: List<List<Uint8>>
    let mut list_offsets = Vec::with_capacity(input.len() + 1);
    let mut inner_offsets = Vec::with_capacity(binary_values.len() + 1);
    let mut byte_values = Vec::new();

    list_offsets.push(0i32);
    inner_offsets.push(0i32);

    for i in 0..input.len() {
        if input.is_null(i) {
            list_offsets.push(*list_offsets.last().unwrap());
            continue;
        }

        let start = input.value_offsets()[i] as usize;
        let end = input.value_offsets()[i + 1] as usize;

        for j in start..end {
            let bytes = binary_values.value(j);
            byte_values.extend_from_slice(bytes);
            inner_offsets.push(byte_values.len() as i32);
        }

        list_offsets.push(inner_offsets.len() as i32 - 1);
    }

    let uint8_array = UInt8Array::from(byte_values);
    let inner_list = ListArray::new(
        Arc::new(Field::new("item", DataType::UInt8, false)),
        OffsetBuffer::new(inner_offsets.into()),
        Arc::new(uint8_array),
        input.nulls().cloned(),
    );

    let outer_list = ListArray::new(
        Arc::new(Field::new("item", inner_list.data_type().clone(), true)),
        input.offsets().clone(),
        Arc::new(inner_list),
        input.nulls().cloned(),
    );

    Ok(outer_list)
}

fn convert_list_struct_to_list_fixed(list_array: ListArray) -> Result<ListArray, Error> {
    let (_, offsets, values, nulls) = list_array.into_parts();
    let struct_array = values
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: values.data_type().clone(),
            expected: DataType::Struct(
                vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                    Field::new("z", DataType::Float64, false),
                ]
                .into(),
            ),
        })?;

    // Assumes struct has exactly 3 fields in order: x, y, z
    let x = struct_array
        .column(0)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(0).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let y = struct_array
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(1).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let z = struct_array
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(2).data_type().clone(),
            expected: DataType::Float64,
        })?;

    let value_builder = Float32Builder::new();
    let mut fixed_builder = FixedSizeListBuilder::new(value_builder, 3);

    for (x, y, z) in itertools::izip!(x.iter(), y.iter(), z.iter()) {
        let (Some(x), Some(y), Some(z)) = (x, y, z) else {
            re_log::warn_once!("Skipping unexpected Vector3 with missing component");
            continue;
        };
        fixed_builder
            .values()
            .append_slice(&[x as f32, y as f32, z as f32]);
        fixed_builder.append(true);
    }

    let fixed_list_array = fixed_builder.finish();

    Ok(ListArray::new(
        Arc::new(Field::new_list_field(
            fixed_list_array.data_type().clone(),
            true,
        )),
        offsets,
        Arc::new(fixed_list_array),
        nulls,
    ))
}

fn convert_list_struct_to_list_list_fixed(list_array: ListArray) -> Result<ListArray, Error> {
    let (_, offsets, values, nulls) = list_array.into_parts();
    let struct_array = values
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: values.data_type().clone(),
            expected: DataType::Struct(
                vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                    Field::new("z", DataType::Float64, false),
                ]
                .into(),
            ),
        })?;

    // Assumes struct has exactly 3 fields in order: x, y, z
    let x = struct_array
        .column(0)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(0).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let y = struct_array
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(1).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let z = struct_array
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(2).data_type().clone(),
            expected: DataType::Float64,
        })?;

    let value_builder = Float32Builder::new();
    let mut fixed_builder = FixedSizeListBuilder::new(value_builder, 3);

    for (x, y, z) in itertools::izip!(x.iter(), y.iter(), z.iter()) {
        let (Some(x), Some(y), Some(z)) = (x, y, z) else {
            re_log::warn_once!("Skipping unexpected Vector3 with missing component");
            continue;
        };
        fixed_builder
            .values()
            .append_slice(&[x as f32, y as f32, z as f32]);
        fixed_builder.append(true);
    }

    let fixed_list_array = fixed_builder.finish();

    Ok(ListArray::new(
        Arc::new(Field::new_list_field(
            fixed_list_array.data_type().clone(),
            true,
        )),
        offsets,
        Arc::new(fixed_list_array),
        nulls,
    ))
}

// TODO: Use clamping
fn create_dummy_points_for_pose(list_array: ListArray) -> Result<ListArray, Error> {
    let (_, offsets, values, nulls) = list_array.into_parts();
    let struct_array = values
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: values.data_type().clone(),
            expected: DataType::Struct(
                vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                    Field::new("z", DataType::Float64, false),
                ]
                .into(),
            ),
        })?;

    // Assumes struct has exactly 3 fields in order: x, y, z
    let x = struct_array
        .column(0)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(0).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let y = struct_array
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(1).data_type().clone(),
            expected: DataType::Float64,
        })?;
    let z = struct_array
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            actual: struct_array.column(2).data_type().clone(),
            expected: DataType::Float64,
        })?;

    let value_builder = Float32Builder::new();
    let mut fixed_builder = FixedSizeListBuilder::new(value_builder, 3);

    for (x, y, z) in itertools::izip!(x.iter(), y.iter(), z.iter()) {
        let (Some(_), Some(_), Some(_)) = (x, y, z) else {
            re_log::warn_once!("Skipping unexpected Vector3 with missing component");
            continue;
        };
        fixed_builder.values().append_slice(&[0.0, 0.0, 0.0]);
        fixed_builder.append(true);
    }

    let fixed_list_array = fixed_builder.finish();

    Ok(ListArray::new(
        Arc::new(Field::new_list_field(
            fixed_list_array.data_type().clone(),
            true,
        )),
        offsets,
        Arc::new(fixed_list_array),
        nulls,
    ))
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // plural
    let instance_poses_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.PosesInFrame:message")
            .add_output_column(
                InstancePoses3D::descriptor_translations(),
                [
                    Op::access_field("poses"),
                    Op::access_field("position"),
                    Op::func(convert_list_struct_to_list_fixed),
                ],
            )
            .add_output_column(
                // TODO: should probably use `Pinhole`
                Points3D::descriptor_positions(),
                [
                    Op::access_field("poses"),
                    Op::access_field("position"),
                    Op::func(create_dummy_points_for_pose),
                ],
            )
            .build();

    // singular
    let instance_pose_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.PoseInFrame:message")
            .add_output_column(
                InstancePoses3D::descriptor_translations(),
                [
                    Op::access_field("pose"),
                    Op::access_field("position"),
                    Op::func(convert_list_struct_to_list_fixed),
                ],
            )
            .add_output_column(
                Points3D::descriptor_positions(),
                [
                    Op::access_field("pose"),
                    Op::access_field("position"),
                    Op::func(create_dummy_points_for_pose),
                ],
            )
            .build();

    let image_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
            // TODO: We leave out the `format` column because the `png` contents are not a valid MIME type.
            .add_output_column(
                EncodedImage::descriptor_blob(),
                [
                    Op::access_field("data"),
                    Op::func(list_binary_to_list_uint8),
                ],
            )
            .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_japanese_alley")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
