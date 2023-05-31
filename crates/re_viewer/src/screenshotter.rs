//! Screenshotting not implemented on web yet because
//! we haven't implemented "copy image to clipbaord".

/// Helper for screenshotting the entire app
#[derive(Default)]
pub struct Screenshotter {
    #[cfg(not(target_arch = "wasm32"))]
    countdown: Option<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Screenshotter {
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
    pub fn is_screenshotting(&self) -> bool {
        self.countdown.is_some()
    }

    pub fn request_screenshot(&mut self) {
        self.countdown = Some(1);
    }

    pub fn save(&mut self, image: &egui::ColorImage) {
        self.countdown = None;
        re_viewer_context::Clipboard::with(|cb| {
            cb.set_image(image.size, bytemuck::cast_slice(&image.pixels));
        });
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
