//! Converts MJCF geoms to Rerun archetypes.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ahash::{HashMap, HashMapExt as _};
use mjcf_rs::body::{Geom, GeomType};
use re_chunk::{Chunk, ChunkBuilder, ChunkId, EntityPath, RowId, TimePoint};
use re_sdk_types::AsComponents;
use re_sdk_types::archetypes::{
    Asset3D, Boxes3D, Capsules3D, CoordinateFrame, Cylinders3D, Ellipsoids3D, InstancePoses3D,
    Mesh3D,
};
use re_sdk_types::components::{FillMode, MediaType};
use re_sdk_types::encodings::Rgba32;
use re_sdk_types::external::glam::Vec3;

use super::model;

/// Reads each mesh file once, and collects the failures so they can be reported together.
pub struct MeshLoader<'a> {
    model: &'a mjcf_rs::Model,
    model_dir: &'a Path,
    files: HashMap<PathBuf, Option<(Vec<u8>, MediaType)>>,
    failures: Vec<String>,
}

impl<'a> MeshLoader<'a> {
    pub fn new(model: &'a mjcf_rs::Model, model_dir: &'a Path) -> Self {
        Self {
            model,
            model_dir,
            files: HashMap::new(),
            failures: Vec::new(),
        }
    }

    /// Returns the archetype for the named mesh and its scale, or `None` if it cannot be loaded.
    fn archetype(
        &mut self,
        name: &str,
        color: Rgba32,
        warnings: &mut BTreeSet<String>,
    ) -> anyhow::Result<Option<(Box<dyn AsComponents>, Vec3)>> {
        let Some(mesh) = self.model.assets.mesh(name) else {
            warnings.insert(format!("Geom references unknown mesh {name:?}"));
            return Ok(None);
        };

        let archetype: Box<dyn AsComponents> = if let Some(path) =
            self.model.resolve_mesh_file(mesh, self.model_dir)
        {
            let file = self.files.entry(path.clone()).or_insert_with(|| {
                let Some(media_type) = MediaType::guess_from_path(&path) else {
                    self.failures
                        .push(format!("Unsupported mesh format: {}", path.display()));
                    return None;
                };
                match read_file(&path) {
                    Ok(bytes) => Some((bytes, media_type)),
                    Err(err) => {
                        self.failures.push(format!("{err}: {}", path.display()));
                        None
                    }
                }
            });
            let Some((bytes, media_type)) = file.clone() else {
                return Ok(None);
            };
            Box::new(Asset3D::from_file_contents(bytes, Some(media_type)).with_albedo_factor(color))
        } else if let Some(faces) = &mesh.inline_faces {
            let vertices = mesh.inline_vertices.as_deref().unwrap_or_default();
            if vertices.len() % 3 != 0 || faces.len() % 3 != 0 {
                anyhow::bail!("Mesh {name:?} must have a multiple of 3 `vertex` and `face` values");
            }
            let num_vertices = vertices.len() / 3;
            if let Some(index) = faces.iter().find(|&&i| i as usize >= num_vertices) {
                anyhow::bail!(
                    "Mesh {name:?} refers to vertex {index}, but there are only {num_vertices}"
                );
            }
            Box::new(
                Mesh3D::new(
                    vertices
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|vertex| vertex.map(|v| v as f32)),
                )
                .with_triangle_indices(faces.as_chunks::<3>().0.iter().copied())
                .with_albedo_factor(color),
            )
        } else {
            // Without faces, MuJoCo uses the convex hull of the vertices.
            warnings.insert(format!(
                "Skipped mesh {name:?}: meshes without `file` or `face` are not supported"
            ));
            return Ok(None);
        };

        Ok(Some((
            archetype,
            Vec3::from_array(mesh.scale.map(|v| v as f32)),
        )))
    }

    /// Details are listed on lines starting with `- …` to show up in a details dropdown in the viewer
    /// (see [`re_error::StructuredError::parse`]).
    pub fn log_failures(&self, model_name: &str) {
        if self.failures.is_empty() {
            return;
        }
        let mut message = format!(
            "MJCF model {model_name:?}: failed to load {} mesh file(s)",
            self.failures.len()
        );
        for failure in &self.failures {
            write!(message, "\n- {failure}").ok();
        }
        re_log::error!("{message}");
    }
}

pub fn emit_geom(
    emit: &mut dyn FnMut(Chunk),
    entity_path: EntityPath,
    frame: &str,
    geom: &Geom,
    meshes: &mut MeshLoader<'_>,
    warnings: &mut BTreeSet<String>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    // MuJoCo renders colors without converting them from linear space, so they are effectively sRGB.
    let [r, g, b, a] = model::geom_rgba(meshes.model, geom, warnings).map(|v| v as f32);
    let color = Rgba32::from_unmultiplied_rgba_f32(r, g, b, a);
    let (geom_pose, size) = model::geom_pose_and_size(geom);

    let (geometry, scale): (Box<dyn AsComponents>, Option<Vec3>) = match geom.type_ {
        GeomType::Plane => {
            // A zero half-size makes the plane infinite along that axis.
            let [x, y, _] = size;
            if x <= 0.0 || y <= 0.0 {
                warnings.insert(
                    "Skipped infinite planes: give the plane a non-zero `size` to show it"
                        .to_owned(),
                );
                return Ok(());
            }
            (
                Box::new(
                    Boxes3D::from_half_sizes([[x, y, 0.0]])
                        .with_colors([color])
                        .with_fill_mode(FillMode::Solid),
                ),
                None,
            )
        }
        GeomType::Sphere => (
            Box::new(
                Ellipsoids3D::from_radii([size[0]])
                    .with_colors([color])
                    .with_fill_mode(FillMode::Solid),
            ),
            None,
        ),
        GeomType::Ellipsoid => (
            Box::new(
                Ellipsoids3D::from_half_sizes([size])
                    .with_colors([color])
                    .with_fill_mode(FillMode::Solid),
            ),
            None,
        ),
        GeomType::Box => (
            Box::new(
                Boxes3D::from_half_sizes([size])
                    .with_colors([color])
                    .with_fill_mode(FillMode::Solid),
            ),
            None,
        ),
        GeomType::Capsule => (
            // MJCF capsules are centered on the geom frame, Rerun's start at it.
            Box::new(
                Capsules3D::from_lengths_and_radii([2.0 * size[1]], [size[0]])
                    .with_translations([[0.0, 0.0, -size[1]]])
                    .with_colors([color])
                    .with_fill_mode(FillMode::Solid),
            ),
            None,
        ),
        GeomType::Cylinder => (
            Box::new(
                Cylinders3D::from_lengths_and_radii([2.0 * size[1]], [size[0]])
                    .with_colors([color])
                    .with_fill_mode(FillMode::Solid),
            ),
            None,
        ),
        GeomType::Mesh => {
            let Some(mesh) = &geom.mesh else {
                return Ok(());
            };
            let Some((archetype, scale)) = meshes.archetype(mesh, color, warnings)? else {
                return Ok(());
            };
            (archetype, (scale != Vec3::ONE).then_some(scale))
        }
        GeomType::Hfield | GeomType::Sdf => return Ok(()),
    };

    let mut pose = InstancePoses3D::update_fields()
        .with_translations([geom_pose.translation.as_vec3().to_array()])
        .with_quaternions([geom_pose.rotation.as_quat().to_array()]);
    if let Some(scale) = scale {
        pose = pose.with_scales([scale.to_array()]);
    }

    // Geoms have no frame of their own, so they are attached to their body's frame with a constant pose.
    emit(
        ChunkBuilder::new(ChunkId::new(), entity_path)
            .with_archetype(RowId::new(), timepoint.clone(), &pose)
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &CoordinateFrame::update_fields().with_frame(frame),
            )
            .with_archetype(RowId::new(), timepoint.clone(), geometry.as_ref())
            .build()?,
    );

    Ok(())
}

fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    cfg_select! {
        target_arch = "wasm32" => {
            _ = path;
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Loading files referenced by a model is not supported on the web",
            ))
        }
        _ => std::fs::read(path),
    }
}
