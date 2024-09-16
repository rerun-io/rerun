use re_types::{
    archetypes::Asset3D,
    components::{Blob, MediaType},
    datatypes::{Rgba32, Utf8},
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    const BYTES: &[u8] = &[1, 2, 3, 4, 5, 6];

    let texture_format = re_types::components::ImageFormat::rgb8([2, 3]);
    let texture_buffer = re_types::components::ImageBuffer::from(vec![0x42_u8; 2 * 3 * 3]);

    let expected = Asset3D {
        blob: Blob(BYTES.to_vec().into()),
        media_type: Some(MediaType(Utf8(MediaType::GLTF.into()))),
        albedo_factor: Some(Rgba32::from_unmultiplied_rgba(0xEE, 0x11, 0x22, 0x33).into()),
        albedo_texture_format: Some(texture_format),
        albedo_texture_buffer: Some(texture_buffer.clone()),
    };

    let arch = Asset3D::from_file_contents(BYTES.to_vec(), Some(MediaType::gltf()))
        .with_albedo_factor(0xEE112233)
        .with_albedo_texture(texture_format, texture_buffer);
    similar_asserts::assert_eq!(expected, arch);

    // let expected_extensions: HashMap<_, _> = [
    // ]
    // .into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);

        // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
        // has been fully replaced.
        // util::assert_extensions(
        //     &**array,
        //     expected_extensions[field.name.as_str()].as_slice(),
        // );
    }

    let deserialized = Asset3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
