use re_sdk_types::archetypes::TextDocument;
use re_sdk_types::components::MediaType;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    use re_sdk_types::components::Text;

    let expected = TextDocument {
        text: Text::from("This is the contents of the text document.")
            .serialized(TextDocument::descriptor_text()),
        media_type: MediaType::markdown().serialized(TextDocument::descriptor_media_type()),
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
