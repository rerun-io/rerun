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
mod viewer_context;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

use image_cache::ImageCache;
pub(crate) use log_db::LogDb;
pub(crate) use time_control::{TimeControl, TimeView};

pub(crate) use viewer_context::{Selection, ViewerContext};

// ----------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use egui::emath;

use log_types::{TimePoint, TimeValue};

/// An aggregate of `TimePoint`:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<log_types::TimeSource, BTreeSet<TimeValue>>);

impl TimePoints {
    pub fn insert(&mut self, time_point: &TimePoint) {
        for (time_key, value) in &time_point.0 {
            self.0.entry(*time_key).or_default().insert(*value);
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

    // TODO: OKLab
    let hsva = egui::color::Hsva {
        h: small_rng.gen(),
        s: small_rng.gen_range(0.35..=0.55_f32).sqrt(),
        v: small_rng.gen_range(0.55..=0.80_f32).cbrt(),
        a: 1.0,
    };

    let color = egui::Color32::from(hsva);
    [color.r(), color.g(), color.b()]
}

// ----------------------------------------------------------------------------

pub fn calc_bbox_2d(objects: &data_store::Objects<'_>) -> emath::Rect {
    crate::profile_function!();

    let mut bbox = emath::Rect::NOTHING;

    for (_, obj) in objects.image.iter() {
        let [w, h] = obj.image.size;
        bbox.extend_with(emath::Pos2::ZERO);
        bbox.extend_with(emath::pos2(w as _, h as _));
    }

    for (_, obj) in objects.point2d.iter() {
        bbox.extend_with(obj.pos.into());
    }

    for (_, obj) in objects.bbox2d.iter() {
        bbox.extend_with(obj.bbox.min.into());
        bbox.extend_with(obj.bbox.max.into());
    }

    for (_, obj) in objects.line_segments2d.iter() {
        for [a, b] in obj.line_segments {
            bbox.extend_with(a.into());
            bbox.extend_with(b.into());
        }
    }

    bbox
}

pub fn calc_bbox_3d(objects: &data_store::Objects<'_>) -> macaw::BoundingBox {
    crate::profile_function!();

    let mut bbox = macaw::BoundingBox::nothing();

    for (_, obj) in objects.point3d.iter() {
        bbox.extend((*obj.pos).into());
    }

    for (_, obj) in objects.box3d.iter() {
        let log_types::Box3 {
            rotation,
            translation,
            half_size,
        } = obj.obb;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = glam::Vec3::from(*translation);
        let half_size = glam::Vec3::from(*half_size);
        let transform =
            glam::Mat4::from_scale_rotation_translation(half_size, rotation, translation);
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
        for &[a, b] in obj.line_segments {
            bbox.extend(a.into());
            bbox.extend(b.into());
        }
    }

    for (_, obj) in objects.mesh3d.iter() {
        match &obj.mesh {
            log_types::Mesh3D::Encoded(_) => {
                // TODO: how to we get the bbox of an encoded mesh here?
            }
            log_types::Mesh3D::Raw(mesh) => {
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
