use egui::NumExt as _;

use re_entity_db::TimesPerTimeline;
use re_log_types::TimeType;
use re_ui::{UICommand, UICommandSender as _, UiExt as _, list_item};

use re_viewer_context::{Looping, PlayState, TimeControl, ViewerContext};

use crate::time_panel::TimeControlExt;

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct TimeControlUi;

impl TimeControlUi {
    #[allow(clippy::unused_self)]
    pub fn timeline_selector_ui(
        &self,
        ctx: &ViewerContext<'_>,
        time_control: &mut impl TimeControlExt,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        time_control
            .get_mut()
            .select_a_valid_timeline(times_per_timeline);

        ui.scope(|ui| {
            ui.spacing_mut().button_padding += egui::Vec2::new(2.0, 0.0);
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.open.expansion = 0.0;

            let response = egui::ComboBox::from_id_salt("timeline")
                .selected_text(time_control.get().timeline().name().as_str())
                .show_ui(ui, |ui| {
                    for timeline_stats in times_per_timeline.timelines_with_stats() {
                        let timeline = &timeline_stats.timeline;
                        if ui
                            .selectable_label(
                                timeline == time_control.get().timeline(),
                                (
                                    timeline.name().as_str(),
                                    egui::Atom::grow(),
                                    egui::RichText::new(format!(
                                        "{} events",
                                        re_format::format_uint(timeline_stats.num_events())
                                    ))
                                    .size(10.0)
                                    .color(ui.tokens().text_subdued),
                                ),
                            )
                            .clicked()
                        {
                            time_control.set_timeline(ctx, *timeline);
                        }
                    }
                })
                .response
                .on_hover_ui(|ui| {
                    list_item::list_item_scope(ui, "tooltip", |ui| {
                        ui.markdown_ui(
                            r"
Select timeline.

Each piece of logged data is associated with one or more timelines.

The logging SDK always creates two timelines for you:
* `log_tick` - a sequence timeline with the sequence number of the log call
* `log_time` - a temporal timeline with the time of the log call

You can also define your own timelines, e.g. for sensor time or camera frame number.
"
                            .trim(),
                        );

                        ui.re_hyperlink(
                            "Full documentation",
                            "https://rerun.io/docs/concepts/timelines",
                            // Always open in a new tab
                            true,
                        );
                    });
                });
            // Sort of an inline of the `egui::Response::context_menu` function.
            // This is required to assign an id to the context menu, which would
            // otherwise conflict with the popup of this `ComboBox`'s popup menu.
            egui::Popup::menu(&response)
                .id(egui::Id::new("timeline select context menu"))
                .open_memory(if response.secondary_clicked() {
                    Some(egui::SetOpenCommand::Bool(true))
                } else if response.clicked() {
                    // Explicitly close the menu if the widget was clicked
                    // Without this, the context menu would stay open if the user clicks the widget
                    Some(egui::SetOpenCommand::Bool(false))
                } else {
                    None
                })
                .at_pointer_fixed()
                .show(|ui| {
                    if ui.button("Copy timeline name").clicked() {
                        let timeline = format!("{}", time_control.get().timeline().name());
                        re_log::info!("Copied timeline: {}", timeline);
                        ui.ctx().copy_text(timeline);
                    }
                })
        });
    }

    #[allow(clippy::unused_self)]
    pub fn fps_ui(&self, time_control: &mut TimeControl, ui: &mut egui::Ui) {
        if time_control.get().time_type() == TimeType::Sequence
            && let Some(mut fps) = time_control.get().fps()
        {
            ui.scope(|ui| {
                ui.spacing_mut().interact_size -= egui::Vec2::new(0., 4.);

                ui.add(
                    egui::DragValue::new(&mut fps)
                        .suffix(" FPS")
                        .speed(1)
                        .range(0.0..=f32::INFINITY),
                )
                .on_hover_text("Frames per second");
            });
            time_control.set_fps(fps);
        }
    }

    pub fn play_pause_ui(
        &self,
        ctx: &ViewerContext<'_>,
        time_control: &mut impl TimeControlExt,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0; // from figma
            self.play_button_ui(ctx, time_control.get(), ui);
            self.follow_button_ui(ctx, time_control.get(), ui);
            self.pause_button_ui(ctx, time_control.get(), ui);
            self.step_time_button_ui(ctx, ui);
            self.loop_button_ui(time_control.get_mut(), ui);
        });
    }

    #[allow(clippy::unused_self)]
    fn play_button_ui(
        &self,
        ctx: &ViewerContext<'_>,
        time_control: &TimeControl,
        ui: &mut egui::Ui,
    ) {
        let is_playing = time_control.play_state() == PlayState::Playing;
        if ui
            .large_button_selected(&re_ui::icons::PLAY, is_playing)
            .on_hover_ui(|ui| UICommand::PlaybackTogglePlayPause.tooltip_ui(ui))
            .clicked()
        {
            ctx.command_sender()
                .send_ui(UICommand::PlaybackTogglePlayPause);
        }
    }

    #[allow(clippy::unused_self)]
    fn follow_button_ui(
        &self,
        ctx: &ViewerContext<'_>,
        time_control: &TimeControl,
        ui: &mut egui::Ui,
    ) {
        let is_following = time_control.play_state() == PlayState::Following;
        if ui
            .large_button_selected(&re_ui::icons::FOLLOW, is_following)
            .on_hover_ui(|ui| UICommand::PlaybackFollow.tooltip_ui(ui))
            .clicked()
        {
            ctx.command_sender().send_ui(UICommand::PlaybackFollow);
        }
    }

    #[allow(clippy::unused_self)]
    fn pause_button_ui(
        &self,
        ctx: &ViewerContext<'_>,
        time_control: &TimeControl,
        ui: &mut egui::Ui,
    ) {
        let is_paused = time_control.play_state() == PlayState::Paused;
        if ui
            .large_button_selected(&re_ui::icons::PAUSE, is_paused)
            .on_hover_ui(|ui| UICommand::PlaybackTogglePlayPause.tooltip_ui(ui))
            .clicked()
        {
            ctx.command_sender()
                .send_ui(UICommand::PlaybackTogglePlayPause);
        }
    }

    #[allow(clippy::unused_self)]
    fn step_time_button_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        if ui
            .large_button(&re_ui::icons::ARROW_LEFT)
            .on_hover_ui(|ui| UICommand::PlaybackStepBack.tooltip_ui(ui))
            .clicked()
        {
            ctx.command_sender().send_ui(UICommand::PlaybackStepBack);
        }

        if ui
            .large_button(&re_ui::icons::ARROW_RIGHT)
            .on_hover_ui(|ui| UICommand::PlaybackStepForward.tooltip_ui(ui))
            .clicked()
        {
            ctx.command_sender().send_ui(UICommand::PlaybackStepForward);
        }
    }

    #[allow(clippy::unused_self)]
    fn loop_button_ui(&self, time_control: &mut TimeControl, ui: &mut egui::Ui) {
        let icon = &re_ui::icons::LOOP;

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match time_control.get().looping() {
                Looping::Off => {
                    if ui
                        .large_button_selected(icon, false)
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        time_control.set_looping(Looping::All);
                    }
                }
                Looping::All => {
                    ui.visuals_mut().selection.bg_fill = ui.tokens().loop_everything_color;
                    if ui
                        .large_button_selected(icon, true)
                        .on_hover_text("Looping entire recording")
                        .clicked()
                    {
                        time_control.set_looping(Looping::Selection);
                    }
                }
                Looping::Selection => {
                    // ui.visuals_mut().selection.bg_fill = re_ui::ReUi::loop_selection_color(); // we have one color for the button, and a slightly different shade of it for the actual selection :/
                    #[allow(clippy::collapsible_else_if)]
                    if ui
                        .large_button_selected(icon, true)
                        .on_hover_text("Looping selection")
                        .clicked()
                    {
                        time_control.set_looping(Looping::Off);
                    }
                }
            }
        });
    }

    #[allow(clippy::unused_self)]
    pub fn playback_speed_ui(&self, time_control: &mut TimeControl, ui: &mut egui::Ui) {
        let mut speed = time_control.get().speed();
        let drag_speed = (speed * 0.02).at_least(0.01);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size -= egui::Vec2::new(0., 4.);
            ui.add(
                egui::DragValue::new(&mut speed)
                    .speed(drag_speed)
                    .suffix("x"),
            )
            .on_hover_text("Playback speed");
        });

        time_control.set_speed(speed);
    }
}
