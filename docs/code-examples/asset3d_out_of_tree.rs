//! Log a simple 3D asset with an out-of-tree transform which will not affect its children.

use rerun::{
    archetypes::{Asset3D, Points3D, ViewCoordinates},
    components::OutOfTreeTransform3D,
    datatypes::TranslationRotationScale3D,
    demo_util::grid,
    external::{anyhow, glam},
    RecordingStreamBuilder,
};

fn main() -> Result<(), anyhow::Error> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb]>", args[0]);
    };

    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_asset3d_out_of_tree").memory()?;

    rec.log_timeless("world", &ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis

    rec.set_time_sequence("frame", 0);
    rec.log("world/asset", &Asset3D::from_file(path)?)?;
    // Those points will not be affected by their parent's out-of-tree transform!
    rec.log(
        "world/asset/points",
        &Points3D::new(grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)),
    )?;

    for i in 1..20 {
        rec.set_time_sequence("frame", i);

        // Modify the asset's out-of-tree transform: this will not affect its children (i.e. the points)!
        let translation = TranslationRotationScale3D::translation([0.0, 0.0, i as f32 - 10.0]);
        rec.log_component_batches(
            "world/asset",
            false,
            [&OutOfTreeTransform3D::from(translation) as _],
        )?;
    }

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
