use re_renderer::ScheduledScreenshot;

use crate::ui::SpaceViewId;

/// A previously scheduled GPU readback, waiting for getting the result.
pub enum ScheduledGpuReadback {
    SpaceViewScreenshot {
        screenshot: ScheduledScreenshot,
        space_view_id: SpaceViewId,
    },
    // Picking {
    //     picking: ScheduledPicking,
    //     space_view_id: SpaceViewId,
    // },
}
