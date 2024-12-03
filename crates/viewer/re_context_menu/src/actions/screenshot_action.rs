use re_viewer_context::{Item, ScreenshotTarget, SpaceViewId, SpaceViewRectPublisher};

use crate::{ContextMenuAction, ContextMenuContext};

/// Space view screenshot action.
#[cfg(not(target_arch = "wasm32"))] // TODO(#8264): screenshotting on web
pub enum ScreenshotAction {
    /// Scrteenshot the space view, and copy the results to clipboard.
    CopyScreenshot,

    /// Scrteenshot the space view, and save the results to disk.
    SaveScreenshot,
}

impl ContextMenuAction for ScreenshotAction {
    /// Do we have a context menu for this selection?
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // Allow if there is a single space view selected.
        ctx.selection.len() == 1
            && ctx
                .selection
                .iter()
                .all(|(item, _)| self.supports_item(ctx, item))
    }

    /// Do we have a context menu for this item?
    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        let Item::SpaceView(space_view_id) = item else {
            return false;
        };

        ctx.egui_context.memory_mut(|mem| {
            mem.caches
                .cache::<SpaceViewRectPublisher>()
                .get(space_view_id)
                .is_some()
        })
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            Self::CopyScreenshot => "Copy screenshot".to_owned(),
            Self::SaveScreenshot => "Save screenshot…".to_owned(),
        }
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        let Some(space_view_rect) = ctx.egui_context.memory_mut(|mem| {
            mem.caches
                .cache::<SpaceViewRectPublisher>()
                .get(space_view_id)
                .copied()
        }) else {
            return;
        };

        let target = match self {
            Self::CopyScreenshot => ScreenshotTarget::CopyToClipboard,
            Self::SaveScreenshot => ScreenshotTarget::SaveToDisk,
        };

        ctx.egui_context
            .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                re_viewer_context::ScreenshotInfo {
                    ui_rect: Some(space_view_rect),
                    pixels_per_point: ctx.egui_context.pixels_per_point(),
                    source: re_viewer_context::ScreenshotSource::SpaceView(*space_view_id),
                    target,
                },
            )));
    }
}
