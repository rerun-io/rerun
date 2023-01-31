use egui_extras::RetainedImage;
use std::sync::Arc;

#[derive(Default)]
pub struct StaticImageCache {
    images: std::collections::HashMap<&'static str, Arc<RetainedImage>>,
}

impl StaticImageCache {
    pub fn get(&mut self, id: &'static str, image_bytes: &'static [u8]) -> Arc<RetainedImage> {
        self.images
            .entry(id)
            .or_insert_with(|| {
                let color_image = load_image_bytes(image_bytes)
                    .unwrap_or_else(|err| panic!("Failed to load image {id:?}: {err}"));
                let retained_img = RetainedImage::from_color_image(id, color_image);
                Arc::new(retained_img)
            })
            .clone()
    }
}

fn load_image_bytes(image_bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let image = image::load_from_memory(image_bytes).map_err(|err| err.to_string())?;
    let image = image.into_rgba8();
    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}
