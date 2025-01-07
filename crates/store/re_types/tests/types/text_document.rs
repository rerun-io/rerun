use re_types::{
    archetypes::TextDocument, components::MediaType, Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let expected = TextDocument {
        text: "This is the contents of the text document.".into(),
        media_type: Some(MediaType::markdown()),
    };

    let arch = TextDocument::new("This is the contents of the text document.")
        .with_media_type(MediaType::markdown());
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = TextDocument::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
