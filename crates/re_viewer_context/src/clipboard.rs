/// Handles interfacing with the OS clipboard.
pub struct Clipboard {
    arboard: Option<arboard::Clipboard>,
}

impl Clipboard {
    fn new() -> Self {
        Self {
            arboard: init_arboard(),
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
            if let Err(err) = clipboard.set_image(image_data) {
                re_log::error!("Failed to copy image to clipboard: {err}");
            } else {
                re_log::info!("Image copied to clipboard");
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
            re_log::error!("Failed to initialize clipboard: {err}");
            None
        }
    }
}
