use re_viewer_context::{
    Item, ScreenshotTarget, SystemCommand, SystemCommandSender as _, ViewId, ViewRectPublisher,
};

use crate::{ContextMenuAction, ContextMenuContext};

/// View screenshot action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenshotAction {
    /// Screenshot the view, and copy the results to clipboard.
    CopyScreenshot,

    /// Screenshot the view, and save the results to disk.
    SaveScreenshot,
}

impl ContextMenuAction for ScreenshotAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        match self {
            Self::CopyScreenshot => false,
            Self::SaveScreenshot => true,
        }
    }

    /// Do we have a context menu for this item?
    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        if *self == Self::CopyScreenshot && ctx.viewer_context.is_safari_browser() {
            // Safari only allows access to clipboard on user action (e.g. on-click).
            // However, the screenshot capture results arrives a frame later.
            re_log::debug_once!("Copying screenshots not supported on Safari");
            return false;
        }

        let Item::View(view_id) = item else {
            return false;
        };

        ctx.egui_context().memory_mut(|mem| {
            mem.caches
                .cache::<ViewRectPublisher>()
                .get(view_id)
                .is_some()
        })
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            Self::CopyScreenshot => "Copy screenshot".to_owned(),
            Self::SaveScreenshot => "Save screenshot…".to_owned(),
        }
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        let target = match self {
            Self::CopyScreenshot => ScreenshotTarget::CopyToClipboard,
            Self::SaveScreenshot => ScreenshotTarget::SaveToPathFromFileDialog,
        };
        ctx.viewer_context
            .command_sender()
            .send_system(SystemCommand::SaveScreenshot {
                target,
                view_id: Some(*view_id),
            });
    }
}
