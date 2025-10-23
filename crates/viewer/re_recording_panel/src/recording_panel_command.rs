use crate::data::RecordingPanelData;
use egui::Id;
use itertools::Itertools;
use re_redap_browser::RedapServers;
use re_viewer_context::{DisplayMode, SystemCommand, SystemCommandSender, ViewerContext};

/// Commands that need to be handled in the context of the recording panel UI.
///
/// Why do we need another command kind?
/// So the Next / Previous recording action should be a `UiCommand` (for discoverability).
/// The order of recordings is defined by the recording panel, and the order of the next / previous
/// commands should match that. There is the nice [`RecordingPanelData`] struct that we can use
/// to iterate over the recordings in the same display order as the panel UI.
/// But to construct this, we need a [`ViewerContext`] and the usual `UiCommand` handler doesn't
/// have access to that, so we need to handle these commands "within" the frame, where we have
/// access to the context.
#[derive(Clone, Debug)]
pub enum RecordingPanelCommand {
    /// Switch to the next recording in the recording panel.
    SelectNextRecording,

    /// Switch to the previous recording in the recording panel.
    SelectPreviousRecording,
}

impl RecordingPanelCommand {
    /// Send a command.
    ///
    /// Since the recording panel has no state, commands are stored in egui context.
    pub fn send(self, ctx: &egui::Context) {
        ctx.data_mut(|d| {
            let mut commands: &mut Vec<Self> = d.get_temp_mut_or_default(Id::NULL);
            commands.push(self);
        })
    }

    /// Read and clear all pending commands.
    fn read(ctx: &egui::Context) -> Vec<Self> {
        ctx.data_mut(|d| d.remove_temp(Id::NULL).unwrap_or_default())
    }

    /// Handle any pending recording panel commands.
    pub fn handle_recording_panel_commands(ctx: &ViewerContext<'_>, servers: &'_ RedapServers) {
        let commands = RecordingPanelCommand::read(ctx.egui_ctx());

        let server_data = RecordingPanelData::new(ctx, servers, false);

        for command in commands {
            match command {
                RecordingPanelCommand::SelectNextRecording => {
                    Self::shift_through_recordings(ctx, &server_data, 1);
                }
                RecordingPanelCommand::SelectPreviousRecording => {
                    Self::shift_through_recordings(ctx, &server_data, -1);
                }
            }
        }
    }

    fn shift_through_recordings(
        ctx: &ViewerContext<'_>,
        server_data: &RecordingPanelData,
        direction: isize,
    ) {
        let recordings = server_data
            .iter_items_in_display_order()
            .filter(|item| DisplayMode::from_item(item).is_some())
            .collect_vec();
        let displayed_item = ctx.display_mode().item();

        if let Some(displayed_item) = displayed_item {
            let current_index = recordings.iter().position(|item| item == &displayed_item);

            let previous_index = match current_index {
                Some(idx) => {
                    let len = recordings.len() as isize;
                    ((idx as isize + direction + len) % len) as usize
                }
                None => 0,
            };

            if let Some(previous_item) = recordings.get(previous_index) {
                ctx.command_sender()
                    .send_system(SystemCommand::SetSelection(previous_item.clone().into()));
            }
        }
    }
}
