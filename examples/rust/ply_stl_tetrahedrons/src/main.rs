//! Demonstrates loading ASCII `.ply` and `.stl` tetrahedrons with the Rust SDK.
//!
//! The example logs:
//! - a 3D PLY point cloud,
//! - a 3D PLY mesh,
//! - an `x/y`-only PLY mesh, which `Mesh3D` flattens onto `z=0`,
//! - an `x/y`-only PLY point cloud,
//! - and an ASCII STL mesh asset.

use std::path::{Path, PathBuf};

use rerun::external::re_log;

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_ply_stl_tetrahedrons").spawn()?;
    run(&rec)
}

fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP())?;

    let ply_points3d = rerun::Points3D::from_file_path(&data_path("tetrahedron_points3d.ply"))?;
    let ply_mesh3d = rerun::Mesh3D::from_file_path(&data_path("tetrahedron_mesh3d.ply"))?;
    let ply_mesh2d = rerun::Mesh3D::from_file_path(&data_path("tetrahedron_mesh2d_xy_only.ply"))?;
    let ply_points2d =
        rerun::Points2D::from_file_path(&data_path("tetrahedron_points2d_xy_only.ply"))?;
    let stl_mesh = rerun::Asset3D::from_file_path(data_path("tetrahedron.stl"))?;

    log_3d_entity(
        rec,
        "world/ply_point_cloud_3d",
        [-4.5, 0.0, 0.0],
        &ply_points3d,
    )?;
    log_3d_entity(rec, "world/ply_mesh_3d", [-1.5, 0.0, 0.0], &ply_mesh3d)?;
    log_3d_entity(
        rec,
        "world/ply_mesh_2d_xy_only",
        [1.5, 0.0, 0.0],
        &ply_mesh2d,
    )?;
    log_3d_entity(rec, "world/stl_mesh", [4.5, 0.0, 0.0], &stl_mesh)?;

    rec.log_static("points_2d/ply_point_cloud_2d_xy_only", &ply_points2d)?;

    Ok(())
}

fn log_3d_entity<AS: ?Sized + rerun::AsComponents>(
    rec: &rerun::RecordingStream,
    entity_path: &str,
    translation: [f32; 3],
    data: &AS,
) -> anyhow::Result<()> {
    rec.log_static(
        entity_path,
        &rerun::Transform3D::from_translation(translation),
    )?;
    rec.log_static(entity_path, data)?;
    Ok(())
}

fn data_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(filename)
}
