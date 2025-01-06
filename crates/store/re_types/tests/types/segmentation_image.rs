use std::collections::HashMap;

use re_types::{
    archetypes::SegmentationImage,
    datatypes::{ChannelDatatype, ImageFormat},
    Archetype as _, AsComponents as _,
};

use crate::util;

#[test]
fn segmentation_image_roundtrip() {
    let format_expected = ImageFormat {
        width: 3,
        height: 2,
        pixel_format: None,
        channel_datatype: Some(ChannelDatatype::U8),
        color_model: None,
    };

    let all_expected = [SegmentationImage {
        buffer: vec![1, 2, 3, 4, 5, 6].into(),
        format: format_expected.into(),
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

    let expected_extensions: HashMap<_, _> = [("data", vec!["rerun.components.Blob"])].into();

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name().as_str()].as_slice(),
                );
            }
        }

        let deserialized = SegmentationImage::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
