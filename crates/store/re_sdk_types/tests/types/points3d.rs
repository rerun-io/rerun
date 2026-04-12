use std::path::{Path, PathBuf};

use re_sdk_types::archetypes::Points3D;
use re_sdk_types::{components, Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let expected = Points3D {
        positions: vec![
            components::Position3D::new(1.0, 2.0, 3.0), //
            components::Position3D::new(4.0, 5.0, 6.0),
        ]
        .serialized(Points3D::descriptor_positions()),
        radii: vec![
            components::Radius::from(42.0), //
            components::Radius::from(43.0),
        ]
        .serialized(Points3D::descriptor_radii()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Points3D::descriptor_colors()),
        labels: (vec!["hello".into(), "friend".into()] as Vec<components::Text>)
            .serialized(Points3D::descriptor_labels()),
        class_ids: vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]
        .serialized(Points3D::descriptor_class_ids()),
        keypoint_ids: vec![
            components::KeypointId::from(2), //
            components::KeypointId::from(3), //
        ]
        .serialized(Points3D::descriptor_keypoint_ids()),
        show_labels: components::ShowLabels(true.into())
            .serialized(Points3D::descriptor_show_labels()),
    };

    let arch = Points3D::new([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_class_ids([126, 127])
        .with_keypoint_ids([2, 3])
        .with_show_labels(true);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        eprintln!("field = {field:#?}");
        eprintln!("array = {array:#?}");
        // eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Points3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

fn example_ply_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/assets/example.ply")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn ply_from_path_matches_contents_for_example_fixture() {
    let path = example_ply_path();
    let contents = std::fs::read(&path).unwrap();

    let from_path = Points3D::from_file_path(&path).unwrap();
    let from_contents = Points3D::from_file_contents(&contents).unwrap();

    similar_asserts::assert_eq!(from_path, from_contents);
}

#[test]
fn ply_parses_optional_properties_and_ignores_extra_data() {
    let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property float z
property uchar red
property uchar green
property uchar blue
property float radius
property list uchar uchar label
property float temperature
element face 1
property list uchar int vertex_index
end_header
1 2 3 10 20 30 0.5 2 72 105 42
4 5 6 11 21 31 1.5 3 66 121 101 43
3 0 1 1
"#;

    let parsed = Points3D::from_file_contents(contents).unwrap();
    let expected = Points3D::new([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_colors([0x0A141EFF, 0x0B151FFF])
        .with_radii([0.5, 1.5])
        .with_labels(["Hi", "Bye"]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_skips_vertices_missing_required_positions() {
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
4 5 40 50 60
"#;

    let parsed = Points3D::from_file_contents(contents).unwrap();
    let expected = Points3D::new([] as [(f32, f32, f32); 0]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_reports_absolute_payload_line_numbers() {
    let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property float z
end_header
1 2 3
4 5
"#;

    let err = Points3D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("Line 9:"));
}
