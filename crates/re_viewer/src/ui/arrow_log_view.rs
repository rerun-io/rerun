use crate::misc::ViewerContext;

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ArrowLogView {}

impl ArrowLogView {
    pub fn ui(&mut self, _ctx: &mut ViewerContext<'_>, _ui: &mut egui::Ui) {}
}
