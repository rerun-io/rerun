use re_viewer_context::{
    Item, PublishedSpaceViewInfo, ScreenshotTarget, ViewId, SpaceViewRectPublisher,
};

use crate::{ContextMenuAction, ContextMenuContext};

/// Space view screenshot action.
#[cfg(not(target_arch = "wasm32"))]
pub enum ScreenshotAction {
    /// Screenshot the view, and copy the results to clipboard.
    CopyScreenshot,

    /// Screenshot the view, and save the results to disk.
    SaveScreenshot,
}

impl ContextMenuAction for ScreenshotAction {
    /// Do we have a context menu for this selection?
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // Allow if there is a single view selected.
        ctx.selection.len() == 1
            && ctx
                .selection
                .iter()
                .all(|(item, _)| self.supports_item(ctx, item))
    }

    /// Do we have a context menu for this item?
    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        let Item::View(space_view_id) = item else {
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

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &ViewId) {
        let Some(space_view_info) = ctx.egui_context.memory_mut(|mem| {
            mem.caches
                .cache::<SpaceViewRectPublisher>()
                .get(space_view_id)
                .cloned()
        }) else {
            return;
        };

        let PublishedSpaceViewInfo { name, rect } = space_view_info;

        let rect = rect.shrink(1.75); // Hacky: Shrink so we don't accidentally include the border of the space-view.

        let target = match self {
            Self::CopyScreenshot => ScreenshotTarget::CopyToClipboard,
            Self::SaveScreenshot => ScreenshotTarget::SaveToDisk,
        };

        ctx.egui_context
            .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                re_viewer_context::ScreenshotInfo {
                    ui_rect: Some(rect),
                    pixels_per_point: ctx.egui_context.pixels_per_point(),
                    name,
                    target,
                },
            )));
    }
}
