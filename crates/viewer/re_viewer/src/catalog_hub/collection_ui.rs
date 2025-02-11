use re_viewer_context::ViewerContext;

use super::hub::{Command, RecordingCollection};

pub fn collection_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _collection: &RecordingCollection,
) -> Vec<Command> {
    let mut commands = vec![];
    if ui.button("Close").clicked() {
        commands.push(Command::DeselectCollection);
    }

    //TODO: display some data here

    commands
}
