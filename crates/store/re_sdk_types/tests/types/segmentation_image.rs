use re_sdk_types::archetypes::SegmentationImage;
use re_sdk_types::components::{ImageBuffer, ImageFormat};
use re_sdk_types::datatypes::{self, ChannelDatatype};
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn segmentation_image_roundtrip() {
    let format_expected = ImageFormat(datatypes::ImageFormat {
        width: 3,
        height: 2,
        pixel_format: None,
        channel_datatype: Some(ChannelDatatype::U8),
        color_model: None,
    });

    let all_expected = [SegmentationImage {
        buffer: ImageBuffer::from(vec![1, 2, 3, 4, 5, 6])
            .serialized(SegmentationImage::descriptor_buffer()),
        format: format_expected.serialized(SegmentationImage::descriptor_format()),
        draw_order: None,
        opacity: None,
    }];

    let all_arch_serialized = [SegmentationImage::try_from(ndarray::array![
        [1u8, 2, 3],
        [4, 5, 6]
    ])
    .unwrap()
    .to_arrow()
    .unwrap()];

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());
        }

        let deserialized = SegmentationImage::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
