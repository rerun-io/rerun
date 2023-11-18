//! Screenshotting not implemented on web yet because we
//! haven't implemented "copy image to clipboard" there.

/// Helper for screenshotting the entire app
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct Screenshotter {
    countdown: Option<usize>,
    target_path: Option<std::path::PathBuf>,
    quit: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub struct ScreenshotterOutput {
    /// If true, the screenshotter was told at startup to quit after its donw.
    pub quit: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl Screenshotter {
    /// Used for generating screenshots in dev builds.
    ///
    /// Should only be called at startup.
    pub fn screenshot_to_path_then_quit(&mut self, path: std::path::PathBuf) {
        assert!(self.countdown.is_none(), "screenshotter misused");
        self.request_screenshot();
        self.target_path = Some(path);
    }

    pub fn request_screenshot(&mut self) {
        // Give app time to change the style, and then wait for animations to finish:
        self.countdown = Some(10);
    }

    /// Call once per frame
    pub fn update(&mut self, egui_ctx: &egui::Context) -> ScreenshotterOutput {
        if let Some(countdown) = &mut self.countdown {
            if *countdown == 0 {
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            } else {
                *countdown -= 1;
            }

            egui_ctx.request_repaint(); // Make sure we keep counting down
        }

        ScreenshotterOutput { quit: self.quit }
    }

    /// If true, temporarily re-style the UI to make it suitable for capture!
    ///
    /// We do the re-styling to create consistent screenshots across platforms.
    /// In particular, we style the UI to look like the web viewer.
    pub fn is_screenshotting(&self) -> bool {
        self.countdown.is_some()
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
                    self.quit = true;
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

// ----------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct Screenshotter {}

#[cfg(target_arch = "wasm32")]
impl Screenshotter {
    #[allow(clippy::unused_self)]
    pub fn is_screenshotting(&self) -> bool {
        false
    }
}
