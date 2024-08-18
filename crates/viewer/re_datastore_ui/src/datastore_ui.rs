use re_viewer_context::ViewerContext;

#[derive(Default)]
pub struct DatastoreUi;

impl DatastoreUi {
    pub fn ui(&mut self, _ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.label("Datastore UI goes here.");
    }
}
