use re_ui::{ButtonExt as _, Size};
use re_uri::{DATASET_HIERARCHY_SEPARATOR, split_dataset_hierarchy_path};
use re_viewer_context::{CommandSender, Route, SystemCommand, SystemCommandSender as _};

const CRUMB_SIZE: Size = Size::Tiny;

/// Clickable crumbs for the server and every folder above the entry named `path`, excluding its leaf.
///
/// Folder paths are rebuilt from [`split_dataset_hierarchy_path`] segments, so they match the
/// folders of the recording panel tree.
pub fn hierarchy_breadcrumbs_ui(
    ui: &mut egui::Ui,
    origin: &re_uri::Origin,
    path: &str,
    command_sender: &CommandSender,
) {
    let segments: Vec<&str> = split_dataset_hierarchy_path(path).collect();
    let parents = &segments[..segments.len().saturating_sub(1)];

    // Shift so the first crumb's text, not its button frame, aligns with the content below.
    // A detached child keeps the shift from widening the parent's `max_rect`.
    let shift = egui::vec2(CRUMB_SIZE.padding().x, 0.0);
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap().translate(-shift)),
    );

    child.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        crumb_ui(
            ui,
            origin.host.to_string(),
            Route::RedapServer(origin.clone()),
            command_sender,
        );

        let mut folder_path = String::new();
        for segment in parents {
            if !folder_path.is_empty() {
                folder_path.push(DATASET_HIERARCHY_SEPARATOR);
            }
            folder_path.push_str(segment);

            ui.weak("/");
            crumb_ui(
                ui,
                *segment,
                Route::RedapFolder {
                    origin: origin.clone(),
                    path: folder_path.clone(),
                },
                command_sender,
            );
        }

        ui.weak("/");
    });

    ui.advance_cursor_after_rect(child.min_rect().translate(shift));
}

fn crumb_ui(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    route: Route,
    command_sender: &CommandSender,
) {
    if ui
        .add(egui::Button::new(label.into()).ghost().size(CRUMB_SIZE))
        .clicked()
    {
        if let Some(item) = route.item() {
            command_sender.send_system(SystemCommand::set_selection(item));
        }
        command_sender.send_system(SystemCommand::SetRoute(route));
    }
}
