use re_viewer_context::ViewerContext;

use super::catalog_hub::{Command, RecordingCollection};

pub fn collection_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    collection: &RecordingCollection,
) -> Vec<Command> {
    let mut commands = vec![];
    if ui.button("Close").clicked() {
        commands.push(Command::DeselectCollection);
    }

    //TODO: display some data here

    commands
}
