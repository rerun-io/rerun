use crate::{misc::ViewerContext, ui::format_usize};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ArrowLogView {}

impl ArrowLogView {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.label(format!("{} arrow log", format_usize(ctx.log_db.len())));
        ui.separator();
    }
}
