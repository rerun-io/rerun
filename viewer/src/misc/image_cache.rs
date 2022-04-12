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
