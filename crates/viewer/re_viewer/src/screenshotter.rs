//! Screenshotting not implemented on web yet because we
//! haven't implemented "copy image to clipboard" there.

/// Helper for screenshotting the entire app
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct Screenshotter {
    countdown: Option<isize>,
    target_path: Option<std::path::PathBuf>,
    quit: bool,
    pre_screenshot_zoom_factor: Option<f32>,
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub struct ScreenshotterOutput {
    /// If true, the screenshotter was told at startup to quit after it's done.
    pub quit: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl Screenshotter {
    /// Used for generating screenshots in dev builds.
    ///
    /// Should only be called at startup.
    pub fn screenshot_to_path_then_quit(
        &mut self,
        egui_ctx: &egui::Context,
        path: std::path::PathBuf,
    ) {
        assert!(self.countdown.is_none(), "screenshotter misused");
        self.request_screenshot(egui_ctx);
        self.target_path = Some(path);
    }

    pub fn request_screenshot(&mut self, egui_ctx: &egui::Context) {
        // Give app time to change the style, and then wait for animations to finish:
        self.countdown = Some(10);

        self.pre_screenshot_zoom_factor = Some(egui_ctx.zoom_factor());

        // Make screenshots high-quality by pretending we have a high-dpi display, whether we do or not:
        let temporary_pixels_per_points = 2.0;

        let scale_factor = temporary_pixels_per_points / egui_ctx.pixels_per_point();
        let temporary_viewport_size = scale_factor * egui_ctx.content_rect().size();
        egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(temporary_viewport_size));
        egui_ctx.set_pixels_per_point(temporary_pixels_per_points);
    }

    /// Call once per frame
    pub fn update(&mut self, egui_ctx: &egui::Context) -> ScreenshotterOutput {
        if let Some(countdown) = &mut self.countdown {
            if *countdown == 0 {
                // From sending the screenshot command to actually taking it (calling `save`),
                // an arbitrary amount of frames may pass since we don't know when the gpu frame copy
                // is done and transferred to ram.
                // Obviously we want to send the command this command only once, so we keep counting down
                // to negatives until we get a call to `save` which then disables the counter.
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            *countdown -= 1;

            egui_ctx.request_repaint(); // Make sure we keep counting down
        } else if let Some(pre_screenshot_zoom_factor) = self.pre_screenshot_zoom_factor.take() {
            // Restore zoom_factor and viewport size.

            let scale_factor = pre_screenshot_zoom_factor / egui_ctx.zoom_factor();
            let old_viewport_size = scale_factor * egui_ctx.content_rect().size();
            egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(old_viewport_size));
            egui_ctx.set_zoom_factor(pre_screenshot_zoom_factor);
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

    pub fn save(&mut self, egui_ctx: &egui::Context, image: &egui::ColorImage) {
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
            egui_ctx.copy_image(image.clone());
            re_log::info!("Screenshot copied to clipboard");
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct Screenshotter {}

#[cfg(target_arch = "wasm32")]
impl Screenshotter {
    #[expect(clippy::unused_self)]
    pub fn is_screenshotting(&self) -> bool {
        false
    }
}
