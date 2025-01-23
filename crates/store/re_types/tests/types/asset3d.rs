use re_types::{
    archetypes::Asset3D,
    components::{AlbedoFactor, Blob, MediaType},
    datatypes::{Rgba32, Utf8},
    Archetype as _, AsComponents as _, ComponentBatch,
};

#[test]
fn roundtrip() {
    const BYTES: &[u8] = &[1, 2, 3, 4, 5, 6];

    let expected = Asset3D {
        blob: Blob(BYTES.to_vec().into())
            .serialized()
            .map(|blob| blob.with_descriptor_override(Asset3D::descriptor_blob())),
        media_type: MediaType(Utf8(MediaType::GLTF.into()))
            .serialized()
            .map(|media_type| {
                media_type.with_descriptor_override(Asset3D::descriptor_media_type())
            }),
        albedo_factor: AlbedoFactor(Rgba32::from_unmultiplied_rgba(0xEE, 0x11, 0x22, 0x33))
            .serialized()
            .map(|albedo_factor| {
                albedo_factor.with_descriptor_override(Asset3D::descriptor_albedo_factor())
            }),
    };

    let arch = Asset3D::from_file_contents(BYTES.to_vec(), Some(MediaType::gltf()))
        .with_albedo_factor(0xEE112233);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Asset3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
