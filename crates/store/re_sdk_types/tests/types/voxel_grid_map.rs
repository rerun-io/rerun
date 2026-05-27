use re_sdk_types::archetypes::VoxelGridMap;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _, components, datatypes};

#[test]
fn roundtrip() {
    let expected = VoxelGridMap {
        voxel_indices: vec![
            components::VoxelIndex::from([-1, 0, 2]),
            components::VoxelIndex::from([3, 4, 5]),
        ]
        .serialized(VoxelGridMap::descriptor_voxel_indices()),
        cell_size: components::CellSize::from(0.25)
            .serialized(VoxelGridMap::descriptor_cell_size()),
        values: vec![
            components::VoxelValue::from(0.1),
            components::VoxelValue::from(0.9),
        ]
        .serialized(VoxelGridMap::descriptor_values()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC),
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(VoxelGridMap::descriptor_colors()),
        translation: components::Translation3D::new(1.0, 2.0, 3.0)
            .serialized(VoxelGridMap::descriptor_translation()),
        rotation_axis_angle: vec![components::RotationAxisAngle::new(
            [1.0, 0.0, 0.0],
            datatypes::Angle::from_radians(0.5),
        )]
        .serialized(VoxelGridMap::descriptor_rotation_axis_angle()),
        quaternion: vec![components::RotationQuat::from(
            datatypes::Quaternion::from_xyzw([0.0, 0.0, 0.0, 1.0]),
        )]
        .serialized(VoxelGridMap::descriptor_quaternion()),
        opacity: components::Opacity::from(0.5).serialized(VoxelGridMap::descriptor_opacity()),
        value_range: components::ValueRange::from([0.0, 1.0])
            .serialized(VoxelGridMap::descriptor_value_range()),
        colormap: components::Colormap::Turbo.serialized(VoxelGridMap::descriptor_colormap()),
    };

    let arch = VoxelGridMap::new([(-1, 0, 2), (3, 4, 5)], 0.25)
        .with_values([0.1, 0.9])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_translation([1.0, 2.0, 3.0])
        .with_rotation_axis_angle(datatypes::RotationAxisAngle::new(
            [1.0, 0.0, 0.0],
            datatypes::Angle::from_radians(0.5),
        ))
        .with_quaternion(datatypes::Quaternion::from_xyzw([0.0, 0.0, 0.0, 1.0]))
        .with_opacity(0.5)
        .with_value_range([0.0, 1.0])
        .with_colormap(components::Colormap::Turbo);
    similar_asserts::assert_eq!(expected, arch);

    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = VoxelGridMap::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
