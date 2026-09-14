//! Helper to poll, and then save a texture from [`re_renderer::poll_read_texture`].

use image::{ExtendedColorType, ImageEncoder as _};
use re_renderer::{external::wgpu::TextureFormat, texture_readback::TextureReadbackId};
use re_viewer_context::{CommandSender, DownloadAction};

/// Keeps track of textures we're reading back, and prompts the user to save them
/// when they're done.
#[derive(Default)]
pub struct TextureReadbacks {
    active_readbacks: Vec<(TextureReadbackId, DownloadAction)>,
}

impl TextureReadbacks {
    /// Push a new texture readback to keep track of.
    pub fn push(&mut self, id: TextureReadbackId, action: DownloadAction) {
        self.active_readbacks.push((id, action));
    }

    /// Polls if there are any readback textures done, and if so prompt the user to save them.
    pub fn poll_and_save_texture_readbacks(
        &mut self,
        render_ctx: &re_renderer::RenderContext,
        ui: &egui::Ui,
        command_sender: &CommandSender,
        notifications: &mut re_ui::notifications::NotificationUi,
    ) {
        self.active_readbacks.retain(|(id, action)| {
            if let Some(readback) = re_renderer::poll_read_texture(render_ctx, *id) {
                let Some(color_type) = texture_format_to_color_type(readback.format) else {
                    re_log::warn!("Can't download texture with format {:?}", readback.format);
                    return false;
                };
                match action {
                    DownloadAction::CopyToClipboard => {
                        let size = [
                            readback.extent.width as usize,
                            readback.extent.height as usize,
                        ];
                        let data = &readback.data;

                        let Some(image) = to_color_image(color_type, size, data) else {
                            return false;
                        };

                        ui.copy_image(image);
                        notifications.success("Copied image to clipboard");
                    }
                    DownloadAction::Save => {
                        let mut png_bytes = Vec::new();
                        if let Err(err) = image::codecs::png::PngEncoder::new(&mut png_bytes)
                            .write_image(
                                &readback.data,
                                readback.extent.width,
                                readback.extent.height,
                                color_type,
                            )
                        {
                            re_log::error!("Failed to encode preview image as PNG: {err}");
                        } else {
                            command_sender.save_file_dialog(
                                re_capabilities::MainThreadToken::from_egui_ui(ui),
                                "preview.png",
                                "Preview Image".to_owned(),
                                png_bytes,
                            );
                        }
                    }
                }

                false
            } else {
                true
            }
        });

        // If we're waiting for more readbacks, make sure this gets ran again.
        if !self.active_readbacks.is_empty() {
            ui.request_repaint();
        }
    }
}

/// Convert a raw image to a [`egui::ColorImage`].
///
/// As [`egui::ColorImage`] is always 8 bit channels this sometimes looses
/// precision and warns in those cases.
fn to_color_image(
    color_type: ExtendedColorType,
    size: [usize; 2],
    data: &[u8],
) -> Option<egui::ColorImage> {
    Some(match color_type {
        ExtendedColorType::A8 | ExtendedColorType::L8 => egui::ColorImage::from_gray(size, data),

        ExtendedColorType::L16 => {
            re_log::warn!("16 bit image copied as 8 bit image, some precision was lost.");

            egui::ColorImage::from_gray_iter(
                size,
                data.chunks_exact(2).map(|slice| {
                    let pixel = u16::from_ne_bytes(slice.try_into().expect("we use chunks_exact"));

                    // Divide by 2 ^ 8 to convert 16 bit to 8 bit.
                    //
                    // Which means we lose some detail when copying.
                    (pixel >> 8) as u8
                }),
            )
        }

        ExtendedColorType::Rgb8 => egui::ColorImage::from_rgb(size, data),
        ExtendedColorType::Rgba8 => egui::ColorImage::from_rgba_unmultiplied(size, data),

        ExtendedColorType::Bgr8 => egui::ColorImage::from_rgb(
            size,
            &data
                .chunks_exact(3)
                .flat_map(|slice| [slice[2], slice[1], slice[0]])
                .collect::<Vec<_>>(),
        ),

        ExtendedColorType::Bgra8 => egui::ColorImage::from_rgb(
            size,
            &data
                .chunks_exact(4)
                .flat_map(|slice| [slice[2], slice[1], slice[0], slice[3]])
                .collect::<Vec<_>>(),
        ),

        ExtendedColorType::Rgb16 => {
            re_log::warn!("16 bit image copied as 8 bit image, some precision was lost.");

            egui::ColorImage::from_rgb(
                size,
                &data
                    .chunks_exact(6)
                    .flat_map(|slice| {
                        let r = u16::from_ne_bytes(
                            slice[0..2].try_into().expect("we use chunks_exact"),
                        );
                        let g = u16::from_ne_bytes(
                            slice[2..4].try_into().expect("we use chunks_exact"),
                        );
                        let b = u16::from_ne_bytes(
                            slice[4..6].try_into().expect("we use chunks_exact"),
                        );

                        // Divide by 2 ^ 8 to convert 16 bit to 8 bit.
                        //
                        // Which means we lose some detail when copying.
                        [r, g, b].map(|e| (e >> 8) as u8)
                    })
                    .collect::<Vec<_>>(),
            )
        }

        ExtendedColorType::Rgba16 => {
            re_log::warn!("16 bit image copied as 8 bit image, some precision was lost.");

            egui::ColorImage::from_rgb(
                size,
                &data
                    .chunks_exact(8)
                    .flat_map(|slice| {
                        let r = u16::from_ne_bytes(
                            slice[0..2].try_into().expect("we use chunks_exact"),
                        );
                        let g = u16::from_ne_bytes(
                            slice[2..4].try_into().expect("we use chunks_exact"),
                        );
                        let b = u16::from_ne_bytes(
                            slice[4..6].try_into().expect("we use chunks_exact"),
                        );
                        let a = u16::from_ne_bytes(
                            slice[6..8].try_into().expect("we use chunks_exact"),
                        );

                        // Divide by 2 ^ 8 to convert 16 bit to 8 bit.
                        //
                        // Which means we lose some detail when copying.
                        [r, g, b, a].map(|e| (e >> 8) as u8)
                    })
                    .collect::<Vec<_>>(),
            )
        }

        _ => {
            re_log::error!("Can't copy textures with color type `{color_type:?}`");
            return None;
        }
    })
}

fn texture_format_to_color_type(
    format: re_renderer::external::wgpu::TextureFormat,
) -> Option<image::ExtendedColorType> {
    match format {
        // 8-bit per channel
        TextureFormat::R8Unorm
        | TextureFormat::R8Snorm
        | TextureFormat::R8Uint
        | TextureFormat::R8Sint => Some(ExtendedColorType::L8),
        TextureFormat::Rg8Unorm
        | TextureFormat::Rg8Snorm
        | TextureFormat::Rg8Uint
        | TextureFormat::Rg8Sint => Some(ExtendedColorType::La8),
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Rgba8Snorm
        | TextureFormat::Rgba8Uint
        | TextureFormat::Rgba8Sint => Some(ExtendedColorType::Rgba8),
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => Some(ExtendedColorType::Bgra8),

        // 16-bit per channel
        TextureFormat::R16Uint
        | TextureFormat::R16Sint
        | TextureFormat::R16Unorm
        | TextureFormat::R16Snorm
        | TextureFormat::R16Float => Some(ExtendedColorType::L16),
        TextureFormat::Rg16Uint
        | TextureFormat::Rg16Sint
        | TextureFormat::Rg16Unorm
        | TextureFormat::Rg16Snorm
        | TextureFormat::Rg16Float => Some(ExtendedColorType::La16),
        TextureFormat::Rgba16Uint
        | TextureFormat::Rgba16Sint
        | TextureFormat::Rgba16Unorm
        | TextureFormat::Rgba16Snorm
        | TextureFormat::Rgba16Float => Some(ExtendedColorType::Rgba16),
        // lossy: f16 → u16
        // 32-bit float
        TextureFormat::Rgba32Float => Some(ExtendedColorType::Rgba32F),

        // 32-bit int (image has no direct equivalent — map to float or return None)
        // Packed / compressed / depth-stencil — not representable
        _ => None,
    }
}
