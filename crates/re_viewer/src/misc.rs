#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
pub(crate) mod image_cache;
pub(crate) mod log_db;
pub(crate) mod mesh_loader;
#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub(crate) mod profiler;
pub(crate) mod time_axis;
pub(crate) mod time_control;
pub(crate) mod time_control_ui;
mod time_range;
mod time_real;
mod viewer_context;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

use image_cache::ImageCache;
pub(crate) use log_db::LogDb;
pub(crate) use time_control::{TimeControl, TimeView};
pub(crate) use time_range::{TimeRange, TimeRangeF};
pub(crate) use time_real::TimeReal;
pub(crate) use viewer_context::*;

// ----------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use egui::emath;

use re_log_types::{CameraSpaceConvention, TimeInt, TimePoint, TimeSource};

/// An aggregate of [`TimePoint`]:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<TimeSource, BTreeSet<TimeInt>>);

impl TimePoints {
    pub fn insert(&mut self, time_point: &TimePoint) {
        for (time_source, value) in &time_point.0 {
            self.0
                .entry(*time_source)
                .or_default()
                .insert(value.as_int());
        }
    }
}

pub fn help_hover_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Label::new("❓").sense(egui::Sense::click()), // sensing clicks also gives hover effect
    )
}

pub fn random_rgb(seed: u64) -> [u8; 3] {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    let mut small_rng = SmallRng::seed_from_u64(seed);

    loop {
        // TODO(emilk): OKLab
        let hsva = egui::color::Hsva {
            h: small_rng.gen(),
            s: small_rng.gen_range(0.35..=0.55_f32).sqrt(),
            v: small_rng.gen_range(0.55..=0.80_f32).cbrt(),
            a: 1.0,
        };

        let rgba = egui::Rgba::from(hsva);

        let intensity = 0.2126 * rgba.r() + 0.7152 * rgba.g() + 0.0722 * rgba.b();

        if intensity > 0.7 {
            let color = egui::Color32::from(rgba);
            return [color.r(), color.g(), color.b()];
        }
    }
}

// ----------------------------------------------------------------------------

pub fn calc_bbox_2d(objects: &re_data_store::Objects<'_>) -> emath::Rect {
    crate::profile_function!();

    let mut bbox = emath::Rect::NOTHING;

    for (_, obj) in objects.image.iter() {
        if obj.tensor.shape.len() >= 2 {
            let [h, w] = [obj.tensor.shape[0], obj.tensor.shape[1]];
            bbox.extend_with(emath::Pos2::ZERO);
            bbox.extend_with(emath::pos2(w as _, h as _));
        }
    }

    for (_, obj) in objects.point2d.iter() {
        bbox.extend_with(obj.pos.into());
    }

    for (_, obj) in objects.bbox2d.iter() {
        bbox.extend_with(obj.bbox.min.into());
        bbox.extend_with(obj.bbox.max.into());
    }

    for (_, obj) in objects.line_segments2d.iter() {
        for point in obj.points {
            bbox.extend_with(point.into());
        }
    }

    bbox
}

pub fn calc_bbox_3d(objects: &re_data_store::Objects<'_>) -> macaw::BoundingBox {
    crate::profile_function!();

    let mut bbox = macaw::BoundingBox::nothing();

    for (_, obj) in objects.point3d.iter() {
        bbox.extend((*obj.pos).into());
    }

    for (_, obj) in objects.box3d.iter() {
        let re_log_types::Box3 {
            rotation,
            translation,
            half_size,
        } = obj.obb;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = glam::Vec3::from(*translation);
        let half_size = glam::Vec3::from(*half_size);
        let transform =
            glam::Affine3A::from_scale_rotation_translation(half_size, rotation, translation);
        use glam::vec3;
        let corners = [
            transform
                .transform_point3(vec3(-0.5, -0.5, -0.5))
                .to_array(),
            transform.transform_point3(vec3(-0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, 0.5)).to_array(),
        ];
        for p in corners {
            bbox.extend(p.into());
        }
    }

    for (_, obj) in objects.path3d.iter() {
        for &p in obj.points {
            bbox.extend(p.into());
        }
    }

    for (_, obj) in objects.line_segments3d.iter() {
        for &point in obj.points {
            bbox.extend(point.into());
        }
    }

    for (_, obj) in objects.mesh3d.iter() {
        match &obj.mesh {
            re_log_types::Mesh3D::Encoded(_) => {
                // TODO(emilk): how to we get the bbox of an encoded mesh here?
            }
            re_log_types::Mesh3D::Raw(mesh) => {
                for &pos in &mesh.positions {
                    bbox.extend(pos.into());
                }
            }
        }
    }

    for (_, obj) in objects.camera.iter() {
        bbox.extend(obj.camera.position.into());
    }

    bbox
}

// ----------------------------------------------------------------------------

/// Rerun uses a RHS view-space with +X=right, +Y=up, -Z=fwd.
/// This creates a transform from the Rerun view-space
/// to the parent space of the camera.
pub fn world_from_view_from_cam(cam: &re_log_types::Camera) -> macaw::IsoTransform {
    use glam::*;

    let rotation = Quat::from_slice(&cam.rotation);
    let translation = Vec3::from_slice(&cam.position);

    let rotation = match cam.camera_space_convention {
        CameraSpaceConvention::XRightYUpZBack => {
            rotation // same as the Rerun convention
        }
        CameraSpaceConvention::XRightYDownZFwd => {
            rotation * Quat::from_rotation_x(std::f32::consts::TAU / 2.0)
        }
    };

    macaw::IsoTransform::from_rotation_translation(rotation, translation)
}
