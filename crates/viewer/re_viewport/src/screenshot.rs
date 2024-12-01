use re_viewer_context::ScreenshotTarget;
use re_viewport_blueprint::SpaceViewBlueprint;

// TODO: use this
pub fn handle_pending_space_view_screenshots(
    space_view: &SpaceViewBlueprint,
    data: &[u8],
    extent: glam::UVec2,
    target: ScreenshotTarget,
) {
    // Set to clipboard.
    #[cfg(not(target_arch = "wasm32"))]
    re_viewer_context::Clipboard::with(|clipboard| {
        clipboard.set_image([extent.x as _, extent.y as _], data);
    });
    if target == ScreenshotTarget::CopyToClipboard {
        return;
    }

    // Get next available file name.
    fn is_safe_filename_char(c: char) -> bool {
        c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')
    }
    let safe_display_name = space_view
        .display_name_or_default()
        .as_ref()
        .replace(|c: char| !is_safe_filename_char(c), "");
    let mut i = 1;
    let filename = loop {
        let filename = format!("Screenshot {safe_display_name} - {i}.png");
        if !std::path::Path::new(&filename).exists() {
            break filename;
        }
        i += 1;
    };
    let filename = std::path::Path::new(&filename);
    let readable_path = filename.canonicalize().unwrap_or(filename.to_path_buf());

    // TODO: use rfd to pick a path?

    match image::save_buffer(filename, data, extent.x, extent.y, image::ColorType::Rgba8) {
        Ok(()) => {
            re_log::info!("Saved screenshot to {readable_path:?}.",);
        }
        Err(err) => {
            re_log::error!("Failed to save screenshot to {readable_path:?}: {err}");
        }
    }
}
