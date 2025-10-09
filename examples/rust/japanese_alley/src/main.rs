use std::sync::Arc;

use arrow::{
    array::{Array, BinaryArray, ListArray, UInt8Array},
    buffer::OffsetBuffer,
    datatypes::{DataType, Field},
};
use rerun::{
    EncodedImage,
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

fn image_lens(entity_path: &str) -> Lens {
    LensBuilder::for_input_column(
        EntityPathFilter::parse_forgiving(entity_path),
        "foxglove.CompressedImage:message",
    )
    // TODO: We leave out the `format` column because the `png` contents are not a valid MIME type.
    .add_output_column(
        EncodedImage::descriptor_blob(),
        [
            Op::access_field("data"),
            Op::func(list_binary_to_list_uint8),
        ],
    )
    .build()
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens("/depth_left"))
        .with_lens(image_lens("/depth_right"))
        .with_lens(image_lens("/image_left"))
        .with_lens(image_lens("/image_right"));

    let (rec, _serve_guard) = args.rerun.init("rerun_example_japanese_alley")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
