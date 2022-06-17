/// Handles interfacing with the OS clipboard.
///
/// If the "clipboard" feature is off, or we cannot connect to the OS clipboard,
/// then a fallback clipboard that just works works within the same app is used instead.
pub struct Clipboard {
    arboard: Option<arboard::Clipboard>,
}

impl Clipboard {
    fn new() -> Self {
        Self {
            arboard: init_arboard(),
        }
    }

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))] // only used sometimes
    pub fn set_text(&mut self, text: String) {
        if let Some(clipboard) = &mut self.arboard {
            if let Err(err) = clipboard.set_text(text) {
                tracing::error!("Failed to copy image to clipboard: {}", err);
            } else {
                tracing::info!("Image copied to clipboard");
            }
        }
    }

    pub fn set_image(&mut self, size: [usize; 2], rgba_unmultiplied: &[u8]) {
        let [width, height] = size;
        assert_eq!(width * height * 4, rgba_unmultiplied.len());

        if let Some(clipboard) = &mut self.arboard {
            let image_data = arboard::ImageData {
                width,
                height,
                bytes: rgba_unmultiplied.into(),
            };
            // TODO: show a quick popup in gui instead of logging
            if let Err(err) = clipboard.set_image(image_data) {
                tracing::error!("Failed to copy image to clipboard: {}", err);
            } else {
                tracing::info!("Image copied to clipboard");
            }
        }
    }

    /// Get access to the thread-local [`Clipboard`].
    pub fn with<R>(f: impl FnOnce(&mut Clipboard) -> R) -> R {
        use std::cell::RefCell;
        thread_local! {
            static CLIPBOARD: RefCell<Option<Clipboard>> = RefCell::new(None);
        }

        CLIPBOARD.with(|clipboard| {
            let mut clipboard = clipboard.borrow_mut();
            let clipboard = clipboard.get_or_insert_with(Clipboard::new);
            f(clipboard)
        })
    }
}

fn init_arboard() -> Option<arboard::Clipboard> {
    match arboard::Clipboard::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            tracing::error!("Failed to initialize clipboard: {}", err);
            None
        }
    }
}
