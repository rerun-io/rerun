#[derive(PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)] // Not used on the web.
pub enum ScreenshotMode {
    /// The screenshot will be saved to disc and copied to the clipboard.
    SaveAndCopyToClipboard,

    /// The screenshot will be copied to the clipboard.
    CopyToClipboard,
}
