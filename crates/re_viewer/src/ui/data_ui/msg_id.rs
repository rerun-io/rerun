use re_log_types::MsgId;

use crate::misc::ViewerContext;

use super::{DataUi, Preview};

impl DataUi for MsgId {
    fn data_ui(&self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, preview: Preview) {
        if let Some(msg) = ctx.log_db.get_log_msg(self) {
            msg.data_ui(ctx, ui, preview);
        }
    }
}
