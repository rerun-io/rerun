use std::path::{Path, PathBuf};

use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{self, ShowLabels};
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let expected = Points2D {
        positions: vec![
            components::Position2D::new(1.0, 2.0), //
            components::Position2D::new(3.0, 4.0),
        ]
        .serialized(Points2D::descriptor_positions()),
        radii: vec![
            components::Radius::from(42.0), //
            components::Radius::from(43.0),
        ]
        .serialized(Points2D::descriptor_radii()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Points2D::descriptor_colors()),
        labels: vec![
            components::Text::from("hello"),  //
            components::Text::from("friend"), //
        ]
        .serialized(Points2D::descriptor_labels()),
        draw_order: components::DrawOrder::from(300.0)
            .serialized(Points2D::descriptor_draw_order()),
        class_ids: vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]
        .serialized(Points2D::descriptor_class_ids()),
        keypoint_ids: vec![
            components::KeypointId::from(2), //
            components::KeypointId::from(3), //
        ]
        .serialized(Points2D::descriptor_keypoint_ids()),
        show_labels: ShowLabels::from(false).serialized(Points2D::descriptor_show_labels()),
    };

    let arch = Points2D::new([(1.0, 2.0), (3.0, 4.0)])
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
        .with_class_ids([126, 127])
        .with_keypoint_ids([2, 3])
        .with_show_labels(false);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Points2D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

fn example_ply_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/assets/example.ply")
}

#[test]
fn ply_parses_optional_properties_and_ignores_extra_data() {
    let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property uchar red
property uchar green
property uchar blue
property float radius
property list uchar uchar label
property float temperature
element edge 1
property int vertex1
property int vertex2
end_header
1 2 10 20 30 0.5 2 72 105 42
4 5 11 21 31 1.5 3 66 121 101 43
0 1
"#;

    let parsed = Points2D::from_file_contents(contents).unwrap();
    let expected = Points2D::new([(1.0, 2.0), (4.0, 5.0)])
        .with_colors([0x0A141EFF, 0x0B151FFF])
        .with_radii([0.5, 1.5])
        .with_labels(["Hi", "Bye"]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn ply_from_path_matches_contents_for_two_dimensional_file() {
    let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property uchar red
property uchar green
property uchar blue
end_header
1 2 10 20 30
4 5 11 21 31
"#;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut file, contents).unwrap();

    let from_path = Points2D::from_file_path(file.path()).unwrap();
    let from_contents = Points2D::from_file_contents(contents).unwrap();

    similar_asserts::assert_eq!(from_path, from_contents);
}

#[test]
fn ply_rejects_three_dimensional_headers() {
    let path = example_ply_path();
    let contents = std::fs::read(path).unwrap();

    let err = Points2D::from_file_contents(&contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn ply_reports_absolute_payload_line_numbers() {
    let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
end_header
1 2
3
"#;

    let err = Points2D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("Line 8:"));
}
