use re_viewer_context::{Item, PublishedViewInfo, ScreenshotTarget, ViewId, ViewRectPublisher};

use crate::{ContextMenuAction, ContextMenuContext};

/// View screenshot action.
pub enum ScreenshotAction {
    /// Screenshot the view, and copy the results to clipboard.
    #[cfg(not(target_arch = "wasm32"))] // TODO(#8264): copy-to-screenshot on web
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
        let Item::View(view_id) = item else {
            return false;
        };

        ctx.egui_context.memory_mut(|mem| {
            mem.caches
                .cache::<ViewRectPublisher>()
                .get(view_id)
                .is_some()
        })
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            #[cfg(not(target_arch = "wasm32"))] // TODO(#8264): copy-to-screenshot on web
            Self::CopyScreenshot => "Copy screenshot".to_owned(),
            Self::SaveScreenshot => "Save screenshot…".to_owned(),
        }
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        let Some(view_info) = ctx.egui_context.memory_mut(|mem| {
            mem.caches
                .cache::<ViewRectPublisher>()
                .get(view_id)
                .cloned()
        }) else {
            return;
        };

        let PublishedViewInfo { name, rect } = view_info;

        let rect = rect.shrink(2.5); // Hacky: Shrink so we don't accidentally include the border of the view.

        if !rect.is_positive() {
            re_log::info!("View too small for a screenshot");
            return;
        }

        let target = match self {
            #[cfg(not(target_arch = "wasm32"))] // TODO(#8264): copy-to-screenshot on web
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
