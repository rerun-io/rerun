use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use re_capabilities::MainThreadToken;
use re_log_channel::SaveScreenshotError;
use re_viewer_context::{CommandSender, ScreenshotInfo, ScreenshotTarget};

type DeliveredScreenshot = (ScreenshotInfo, Arc<egui::ColorImage>);

/// Screenshots requested from egui, which the renderer delivers on a later frame.
pub struct ScreenshotRequests {
    tx: Sender<DeliveredScreenshot>,
    rx: Receiver<DeliveredScreenshot>,
}

impl Default for ScreenshotRequests {
    fn default() -> Self {
        // Unbounded: the sender is a renderer callback that must never block,
        // and it delivers at most one image per frame.
        #[expect(clippy::disallowed_methods)]
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }
}

impl ScreenshotRequests {
    /// Ask egui for a screenshot of the next frame, to be handled by [`Self::process_delivered`].
    pub fn request(&self, egui_ctx: &egui::Context, info: ScreenshotInfo) {
        let tx = self.tx.clone();
        let ctx = egui_ctx.clone();
        egui_ctx.request_screenshot(move |image| {
            if tx.send((info, image)).is_ok() {
                ctx.request_repaint();
            }
        });
    }

    /// Crop each screenshot the renderer has delivered, and send it to its [`ScreenshotTarget`].
    pub fn process_delivered(
        &self,
        egui_ctx: &egui::Context,
        command_sender: &CommandSender,
        main_thread_token: MainThreadToken,
    ) {
        while let Ok((info, image)) = self.rx.try_recv() {
            process_screenshot(egui_ctx, command_sender, main_thread_token, &image, info);
        }
    }
}

fn process_screenshot(
    egui_ctx: &egui::Context,
    command_sender: &CommandSender,
    main_thread_token: MainThreadToken,
    image: &Arc<egui::ColorImage>,
    info: ScreenshotInfo,
) {
    let ScreenshotInfo {
        ui_rect,
        pixels_per_point,
        name,
        target,
        notify,
    } = info;

    // Only used in the native `SaveToPath` branch below.
    #[cfg(target_arch = "wasm32")]
    let _ = notify;

    let rgba = if let Some(ui_rect) = ui_rect {
        Arc::new(image.region(&ui_rect, Some(pixels_per_point)))
    } else {
        image.clone()
    };

    match target {
        ScreenshotTarget::CopyToClipboard => {
            egui_ctx.copy_image((*rgba).clone());
        }

        ScreenshotTarget::SaveToPathFromFileDialog => {
            use image::ImageEncoder as _;
            let mut png_bytes: Vec<u8> = Vec::new();
            if let Err(err) = image::codecs::png::PngEncoder::new(&mut png_bytes).write_image(
                rgba.as_raw(),
                rgba.width() as u32,
                rgba.height() as u32,
                image::ExtendedColorType::Rgba8,
            ) {
                re_log::error!("Failed to encode screenshot as PNG: {err}");
            } else {
                let file_name = format!("{name}.png");
                command_sender.save_file_dialog(
                    main_thread_token,
                    &file_name,
                    "Save screenshot".to_owned(),
                    png_bytes,
                );
            }
        }

        ScreenshotTarget::SaveToPath {
            path: file_path,
            max_size,
            on_done,
        } => {
            cfg_select! {
                target_arch = "wasm32" => {
                    _ = max_size;
                    let reason = "saving screenshots to a path is not supported on web";
                    if let Some(on_done) = on_done {
                        on_done.call(Err(SaveScreenshotError::SaveToPathFailed {
                            path: file_path.to_string(),
                            reason: reason.to_owned(),
                        }));
                    } else {
                        re_log::error!(%file_path, "Failed to save screenshot: {reason}");
                    }
                }
                _ => {
                    let Some(rgb_image) = scaled_rgb_image(&rgba, max_size) else {
                        re_log::error!("Failed to create image from screenshot data");
                        if let Some(on_done) = on_done {
                            on_done.call(Err(SaveScreenshotError::InvalidImageData));
                        }
                        return;
                    };

                    let result = match rgb_image.save(&file_path) {
                        Ok(()) => {
                            // Only show a user-facing toast for user-initiated screenshots.
                            if notify {
                                re_log::info!("Saved screenshot to {file_path:?}");
                            } else {
                                re_log::debug!("Saved screenshot to {file_path:?}");
                            }
                            Ok(())
                        }
                        Err(err) => {
                            re_log::error!(?file_path, "Failed to save screenshot: {err}");
                            // Image library has the bad habit of creating the file even when it fails e.g. due to unsupported format. Remove it again.
                            std::fs::remove_file(&file_path).ok();
                            Err(SaveScreenshotError::SaveToPathFailed {
                                path: file_path.to_string(),
                                reason: err.to_string(),
                            })
                        }
                    };

                    if let Some(on_done) = on_done {
                        on_done.call(result);
                    }
                }
            }
        }

        ScreenshotTarget::EncodePng { max_size, on_done } => {
            let result = scaled_rgb_image(&rgba, max_size)
                .ok_or(SaveScreenshotError::InvalidImageData)
                .and_then(|rgb_image| {
                    let mut png = Vec::new();
                    rgb_image
                        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                        .map_err(|err| SaveScreenshotError::EncodePngFailed {
                            reason: err.to_string(),
                        })?;
                    Ok(png)
                });
            on_done.call(result);
        }
    }
}

/// The screenshot as an RGB image, scaled down so its longest side is at most `max_size`.
///
/// RGB rather than RGBA, so it works with JPG and other formats that don't support alpha.
/// There's nothing interesting in the alpha channel anyways.
fn scaled_rgb_image(rgba: &egui::ColorImage, max_size: Option<u32>) -> Option<image::RgbImage> {
    let rgba_image = image::RgbaImage::from_vec(
        rgba.width() as _,
        rgba.height() as _,
        bytemuck::pod_collect_to_vec(&rgba.pixels),
    )?;

    let mut image = image::DynamicImage::ImageRgba8(rgba_image);
    if let Some(max_size) = max_size
        && max_size < image.width().max(image.height())
    {
        image = image.resize(max_size, max_size, image::imageops::FilterType::Triangle);
    }
    Some(image.to_rgb8())
}
