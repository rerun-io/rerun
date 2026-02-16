use re_sdk_types::archetypes::DepthImage;
use re_sdk_types::components::{DepthMeter, ImageBuffer, ImageFormat};
use re_sdk_types::datatypes::{self, ChannelDatatype};
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn depth_image_roundtrip() {
    let format_expected = ImageFormat(datatypes::ImageFormat {
        width: 3,
        height: 2,
        pixel_format: None,
        channel_datatype: Some(ChannelDatatype::U8),
        color_model: None,
    });

    let all_expected = [DepthImage {
        buffer: ImageBuffer::from(vec![1, 2, 3, 4, 5, 6])
            .serialized(DepthImage::descriptor_buffer()),
        format: format_expected.serialized(DepthImage::descriptor_format()),
        meter: DepthMeter::from(1000.0).serialized(DepthImage::descriptor_meter()),
        draw_order: None,
        colormap: None,
        point_fill_ratio: None,
        depth_range: None,
        magnification_filter: None,
    }];

    let all_arch_serialized = [
        DepthImage::try_from(ndarray::array![[1u8, 2, 3], [4, 5, 6]])
            .unwrap()
            .with_meter(1000.0)
            .to_arrow()
            .unwrap(),
    ];

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());
        }

        let deserialized = DepthImage::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

#[test]
fn depth_image_from_gray16() {
    let image_buffer = ndarray::array![[1u16, 2, 3], [4, 5, 6]];
    let depth_image1 = DepthImage::try_from(image_buffer.clone()).unwrap();
    let depth_image2 = DepthImage::from_gray16(
        image_buffer
            .into_iter()
            .flat_map(|num| vec![num as u8, (num >> 8) as u8])
            .collect::<Vec<_>>(),
        [3, 2],
    );
    similar_asserts::assert_eq!(depth_image1, depth_image2);
}
