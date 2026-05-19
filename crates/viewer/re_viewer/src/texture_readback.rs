//! Helper to poll, and then save a texture from [`re_renderer::poll_read_texture`].

use image::{ExtendedColorType, ImageEncoder as _};
use re_renderer::{external::wgpu::TextureFormat, texture_readback::TextureReadbackId};
use re_viewer_context::CommandSender;

/// Keeps track of textures we're reading back, and prompts the user to save them
/// when they're done.
#[derive(Default)]
pub struct TextureReadbacks {
    active_readbacks: Vec<TextureReadbackId>,
}

impl TextureReadbacks {
    /// Push a new texture readback to keep track of.
    pub fn push(&mut self, id: TextureReadbackId) {
        self.active_readbacks.push(id);
    }

    /// Polls if there are any readback textures done, and if so prompt the user to save them.
    pub fn poll_and_save_texture_readbacks(
        &mut self,
        render_ctx: &re_renderer::RenderContext,
        ui: &egui::Ui,
        command_sender: &CommandSender,
    ) {
        self.active_readbacks.retain(|id| {
            if let Some(readback) = re_renderer::poll_read_texture(render_ctx, *id) {
                let Some(color_type) = texture_format_to_color_type(readback.format) else {
                    re_log::warn!("Can't download texture with format {:?}", readback.format);
                    return false;
                };
                let mut png_bytes = Vec::new();
                if let Err(err) = image::codecs::png::PngEncoder::new(&mut png_bytes).write_image(
                    &readback.data,
                    readback.extent.width,
                    readback.extent.height,
                    color_type,
                ) {
                    re_log::error!("Failed to encode preview image as PNG: {err}");
                } else {
                    command_sender.save_file_dialog(
                        re_capabilities::MainThreadToken::from_egui_ui(ui),
                        "preview.png",
                        "Preview Image".to_owned(),
                        png_bytes,
                    );
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
