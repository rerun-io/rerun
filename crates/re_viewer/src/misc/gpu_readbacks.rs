use re_renderer::ScheduledScreenshot;

use crate::ui::SpaceViewId;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ScreenshotMode {
    /// The screenshot will be saved to disc and copied to the clipboard.
    SaveAndCopyToClipboard,

    /// The screenshot will be copied to the clipboard.
    CopyToClipboard,
}

/// A previously scheduled GPU readback, waiting for getting the result.
pub enum ScheduledGpuReadback {
    SpaceViewScreenshot {
        screenshot: ScheduledScreenshot,
        space_view_id: SpaceViewId,
        mode: ScreenshotMode,
    },
    // Picking {
    //     picking: ScheduledPicking,
    //     space_view_id: SpaceViewId,
    // },
}
