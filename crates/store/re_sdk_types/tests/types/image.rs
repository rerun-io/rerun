use re_sdk_types::{Archetype as _, AsComponents as _, archetypes::Image, datatypes::ColorModel};

#[test]
fn image_roundtrip() {
    let all_expected = [Image::from_l8(vec![1, 2, 3, 4, 5, 6], [3, 2])];

    let all_arch_serialized = [Image::from_color_model_and_tensor(
        ColorModel::L,
        ndarray::array![[1u8, 2, 3], [4, 5, 6]],
    )
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

        let deserialized = Image::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

#[test]
#[cfg(feature = "image")]
fn dynamic_image_roundtrip() {
    use image::{Rgb, RgbImage};

    let all_expected = [Image::from_rgb24(
        vec![
            0, 0, 128, 1, 0, 128, 2, 0, 128, //
            0, 1, 128, 1, 1, 128, 2, 1, 128, //
        ],
        [3, 2],
    )];

    let mut img = RgbImage::new(3, 2);

    for x in 0..3 {
        for y in 0..2 {
            img.put_pixel(x, y, Rgb([x as u8, y as u8, 128]));
        }
    }

    let all_arch_serialized = [Image::from_image(img).unwrap().to_arrow().unwrap()];

    for (expected, serialized) in all_expected.into_iter().zip(all_arch_serialized) {
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());
        }

        let deserialized = Image::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
