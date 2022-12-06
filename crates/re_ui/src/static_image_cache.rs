use egui_extras::RetainedImage;

#[derive(Default)]
pub struct StaticImageCache {
    images: std::collections::HashMap<&'static str, RetainedImage>,
}

impl StaticImageCache {
    pub fn get(&mut self, id: &'static str, image_bytes: &'static [u8]) -> &RetainedImage {
        self.images.entry(id).or_insert_with(|| {
            RetainedImage::from_color_image(
                id,
                load_image_bytes(image_bytes)
                    .unwrap_or_else(|err| panic!("Failed to load image {id:?}: {err:?}")),
            )
        })
    }

    pub fn rerun_logo(&mut self, visuals: &egui::Visuals) -> &RetainedImage {
        if visuals.dark_mode {
            self.get(
                "logo_dark_mode",
                include_bytes!("../data/logo_dark_mode.png"),
            )
        } else {
            self.get(
                "logo_light_mode",
                include_bytes!("../data/logo_light_mode.png"),
            )
        }
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
