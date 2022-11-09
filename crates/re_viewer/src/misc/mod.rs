pub(crate) mod color_map;
pub(crate) mod mesh_loader;
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

#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub(crate) mod profiler;

#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;

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
