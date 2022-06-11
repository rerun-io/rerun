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

use log_types::{TimePoint, TimeValue};
use std::collections::{BTreeMap, BTreeSet};

/// An aggregate of `TimePoint`:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<log_types::TimeSource, BTreeSet<TimeValue>>);

impl TimePoints {
    pub fn insert(&mut self, time_point: &TimePoint) {
        for (time_key, value) in &time_point.0 {
            self.0.entry(time_key.clone()).or_default().insert(*value);
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
