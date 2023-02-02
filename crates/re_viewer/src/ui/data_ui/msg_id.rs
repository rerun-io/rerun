use re_log_types::MsgId;

use crate::misc::ViewerContext;

use super::{DataUi, UiVerbosity};

impl DataUi for MsgId {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ctx.msg_id_button(ui, *self);
            }
            UiVerbosity::All | UiVerbosity::Reduced => {
                if let Some(msg) = ctx.log_db.get_log_msg(self) {
                    msg.data_ui(ctx, ui, verbosity, query);
                } else {
                    ctx.msg_id_button(ui, *self);
                }
            }
        }
    }
}
