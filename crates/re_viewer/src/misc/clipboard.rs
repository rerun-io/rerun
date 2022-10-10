//! Handles interfacing with the OS clipboard.

// TODO(emilk): use egui for this instead once https://github.com/emilk/egui/issues/2108 is done

#[allow(unused)] // only used sometimes
pub fn set_text(text: String) {
    if let Some(mut clipboard) = clipboard() {
        if let Err(err) = clipboard.set_text(text) {
            re_log::error!("Failed to copy image to clipboard: {err}",);
        } else {
            re_log::info!("Image copied to clipboard");
        }
    }
}

pub fn set_image(size: [usize; 2], rgba_unmultiplied: &[u8]) {
    let [width, height] = size;
    assert_eq!(width * height * 4, rgba_unmultiplied.len());

    if let Some(mut clipboard) = clipboard() {
        let image_data = arboard::ImageData {
            width,
            height,
            bytes: rgba_unmultiplied.into(),
        };
        // TODO(emilk): show a quick popup in gui instead of logging
        if let Err(err) = clipboard.set_image(image_data) {
            re_log::error!("Failed to copy image to clipboard: {err}");
        } else {
            re_log::info!("Image copied to clipboard");
        }
    }
}

fn clipboard() -> Option<arboard::Clipboard> {
    match arboard::Clipboard::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            re_log::error!("Failed to initialize clipboard: {err}");
            None
        }
    }
}
