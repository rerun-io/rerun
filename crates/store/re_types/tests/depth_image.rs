use std::collections::HashMap;

use re_types::{
    archetypes::DepthImage,
    components::{ChannelDatatype, DepthMeter, Resolution2D},
    Archetype as _, AsComponents as _,
};

mod util;

#[test]
fn depth_image_roundtrip() {
    let all_expected = [DepthImage {
        data: vec![1, 2, 3, 4, 5, 6].into(),
        resolution: Resolution2D::new(3, 2),
        datatype: ChannelDatatype::U8,
        meter: Some(DepthMeter::from(1000.0)),
        draw_order: None,
        colormap: None,
        point_fill_ratio: None,
    }];

    let all_arch_serialized = [
        DepthImage::try_from(ndarray::array![[1u8, 2, 3], [4, 5, 6]])
            .unwrap()
            .with_meter(1000.0)
            .to_arrow()
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

        let deserialized = DepthImage::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
