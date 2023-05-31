//! Screenshotting not implemented on web yet because we
//! haven't implemented "copy image to clipboard" there.

/// Helper for screenshotting the entire app
#[derive(Default)]
pub struct Screenshotter {
    #[cfg(not(target_arch = "wasm32"))]
    countdown: Option<usize>,

    #[cfg(not(target_arch = "wasm32"))]
    target_path: Option<std::path::PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Screenshotter {
    /// Used for generating screenshots in dev builds.
    pub fn screenshot_to_path_then_quit(&mut self, path: std::path::PathBuf) {
        self.request_screenshot();
        self.target_path = Some(path);
    }

    /// Call once per frame
    pub fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(countdown) = &mut self.countdown {
            if *countdown == 0 {
                frame.request_screenshot();
            } else {
                *countdown -= 1;
            }

            egui_ctx.request_repaint(); // Make sure we keep counting down
        }
    }

    /// If true, temporarily re-style the UI to make it suitable for capture!
    ///
    /// We do the re-styling to create consistent screenshots across platforms.
    /// In particular, we style the UI to look like the web viewer.
    pub fn is_screenshotting(&self) -> bool {
        self.countdown.is_some()
    }

    pub fn request_screenshot(&mut self) {
        self.countdown = Some(1);
    }

    pub fn save(&mut self, image: &egui::ColorImage) {
        self.countdown = None;
        if let Some(path) = self.target_path.take() {
            let w = image.width() as _;
            let h = image.height() as _;
            let image =
                image::RgbaImage::from_raw(w, h, bytemuck::pod_collect_to_vec(&image.pixels))
                    .expect("Failed to create image");
            match image.save(&path) {
                Ok(()) => {
                    re_log::info!("Screenshot saved to {path:?}");
                    std::process::exit(0); // Close nicely
                }
                Err(err) => {
                    panic!("Failed saving screenshot to {path:?}: {err}");
                }
            }
        } else {
            re_viewer_context::Clipboard::with(|cb| {
                cb.set_image(image.size, bytemuck::cast_slice(&image.pixels));
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Screenshotter {
    #[allow(clippy::unused_self)]
    pub fn update(&mut self, _egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {}

    #[allow(clippy::unused_self)]
    pub fn is_screenshotting(&self) -> bool {
        false
    }
}
