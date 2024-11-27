use std::collections::HashMap;

use re_types::{
    archetypes::DepthImage,
    components::DepthMeter,
    datatypes::{ChannelDatatype, ImageFormat},
    Archetype as _, AsComponents as _,
};

use crate::util;

#[test]
fn depth_image_roundtrip() {
    let format_expected = ImageFormat {
        width: 3,
        height: 2,
        pixel_format: None,
        channel_datatype: Some(ChannelDatatype::U8),
        color_model: None,
    };

    let all_expected = [DepthImage {
        buffer: vec![1, 2, 3, 4, 5, 6].into(),
        format: format_expected.into(),
        meter: Some(DepthMeter::from(1000.0)),
        draw_order: None,
        colormap: None,
        point_fill_ratio: None,
        depth_range: None,
    }];

    let all_arch_serialized = [
        DepthImage::try_from(ndarray::array![[1u8, 2, 3], [4, 5, 6]])
            .unwrap()
            .with_meter(1000.0)
            .to_arrow2()
            .unwrap(),
    ];

    let expected_extensions: HashMap<_, _> = [("data", vec!["rerun.components.Blob"])].into();

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name);

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = DepthImage::from_arrow2(serialized).unwrap();
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
