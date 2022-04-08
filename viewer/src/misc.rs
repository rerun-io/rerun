use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use egui_extras::RetainedImage;
use log_types::*;

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<LogId, RetainedImage>,
}

impl ImageCache {
    pub fn get(&mut self, log_id: &LogId, image: &Image) -> &RetainedImage {
        self.images
            .entry(*log_id)
            .or_insert_with(|| to_egui_image(image))
    }
}

fn to_egui_image(image: &Image) -> RetainedImage {
    let pixels = image
        .data
        .iter()
        .map(|&l| egui::Color32::from_rgb(l, l, l))
        .collect();
    let color_image = egui::ColorImage {
        size: [image.size[0] as _, image.size[1] as _],
        pixels,
    };
    RetainedImage::from_color_image("image", color_image)
}

// ----------------------------------------------------------------------------

/// An aggregate of `TimePoint`:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<String, BTreeSet<TimeValue>>);

impl TimePoints {
    pub fn insert(&mut self, time_point: &TimePoint) {
        for (time_key, value) in &time_point.0 {
            self.0.entry(time_key.clone()).or_default().insert(*value);
        }
    }
}
