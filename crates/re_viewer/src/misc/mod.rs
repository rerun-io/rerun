#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
pub(crate) mod color_map;
pub(crate) mod mesh_loader;
#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub(crate) mod profiler;
pub(crate) mod space_info;
pub(crate) mod tensor_image_cache;
pub(crate) mod time_axis;
pub(crate) mod time_control;
pub(crate) mod time_control_ui;
mod time_range;
mod time_real;
mod viewer_context;

use tensor_image_cache::ImageCache;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

pub(crate) use time_control::{TimeControl, TimeView};
pub(crate) use time_range::{TimeRange, TimeRangeF};
pub(crate) use time_real::TimeReal;
pub(crate) use viewer_context::*;

// ----------------------------------------------------------------------------

use egui::emath;

// ----------------------------------------------------------------------------

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
            let [h, w] = [obj.tensor.shape[0].size, obj.tensor.shape[1].size];
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
