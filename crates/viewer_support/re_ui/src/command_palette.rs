//! A generic, fuzzy-searchable command palette.
//!
//! The palette itself ([`CommandPalette`]) is agnostic to what a "command" is.
//! A [`CommandPaletteProvider`] supplies the commands, fuzzy-matched against the user's
//! query via [`FuzzyQuery`], and grouped so that different command sources can be visually
//! separated (e.g. UI commands, entity paths in the open recording, and URLs to open).
//!
//! Example UI:
//!
//! ```text
//! query: open
//!
//! - Open File…
//! - Open Catalog…
//!
//! /car/open_roof/
//! /car/open_door/
//! ```

use std::collections::BTreeSet;

use crate::{ButtonExt as _, UiExt as _};

use crate::fuzzy::{FuzzyMatch, FuzzyQuery};
use egui::text::LayoutJob;
use egui::{Key, NumExt as _};

fn command_palette_modal_id() -> egui::Id {
    egui::Id::unique("command_palette_modal")
}

/// The visual content of a single command-palette row, as produced by a
/// [`CommandPaletteProvider`].
///
/// The palette itself handles wrapping, layout, the selection background and input;
/// the provider just supplies the (already highlight-styled) text and any decorations.
pub struct CmdRow {
    /// The text to show, already syntax- and/or fuzzy-highlighted and colored.
    pub job: LayoutJob,

    /// Optional right-aligned keyboard shortcut.
    pub kb_shortcut: String,

    /// Optional hover tooltip.
    pub tooltip: Option<String>,
}

/// How a command-palette row should be drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Selectable, but not currently selected.
    Normal,

    /// Selectable and currently selected (via keyboard or click).
    Selected,

    /// Not selectable (grayed out); ignores hover.
    Disabled,
}

/// Minimum height of a command-palette row, in points.
const ROW_MIN_HEIGHT: f32 = 16.0;

/// Lay out and paint one command-palette row.
///
/// The (highlighted) text goes on the left, wrapping to several lines if it is too wide
/// to fit, plus an optional right-aligned keyboard shortcut; the row grows in height to
/// fit the wrapped text. Selectable rows get a hover highlight. Returns the allocated response.
pub fn paint_command_row(ui: &mut egui::Ui, row: CmdRow, state: RowState) -> egui::Response {
    let CmdRow {
        job,
        kb_shortcut,
        tooltip,
    } = row;

    let selected = state == RowState::Selected;
    let enabled = state != RowState::Disabled;

    // Keep the rows apart, so the match-highlighting underline on the selected row stays clear
    // of the row below it:
    ui.spacing_mut().item_spacing.y = 2.0;

    // Wrap the (possibly long) text to the available width, so long entity/component
    // paths flow onto several lines instead of overflowing the palette:
    let mut button = egui::Button::new(job)
        .wrap()
        .min_size(egui::vec2(ui.available_width(), ROW_MIN_HEIGHT));

    if selected {
        button = button.selected(true);
    }

    if !kb_shortcut.is_empty() {
        // On the selected row the weak color clashes with the selection background,
        // so let the shortcut fall back to the selection text color there.
        let shortcut_text = if selected {
            egui::RichText::new(kb_shortcut)
        } else {
            egui::RichText::new(kb_shortcut).weak()
        };
        button = button.right_text(shortcut_text);
    }

    let mut response = ui.add_enabled(enabled, button);

    if let Some(tooltip) = tooltip {
        response = if enabled {
            response.on_hover_text(tooltip)
        } else {
            response.on_disabled_hover_text(tooltip)
        };
    }
    if enabled {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    response
}

/// A specific command that matches some [`FuzzyQuery`].
pub struct MatchedCmd<Cmd> {
    /// The command itself
    pub command: Cmd,

    /// How well the command matches
    pub fuzzy_match: FuzzyMatch,

    /// Is the command currently available?
    ///
    /// Unavailable commands are still shown (grayed out), but can't be selected.
    pub enabled: bool,
}

impl<Cmd> MatchedCmd<Cmd> {
    /// How well did the [`FuzzyQuery`] match the text?
    ///
    /// Higher = better match.
    fn score(&self) -> i64 {
        self.fuzzy_match.score()
    }
}

/// Commands in a group, that somehow belong together.
pub type MatchGroup<Cmd> = Vec<MatchedCmd<Cmd>>;

/// Whether input searches commands or is submitted as an agent prompt.
#[derive(Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Search,
    Agent,
}

/// One source of commands.
///
/// The type of the command is up to the user to provide.
///
/// For instance: match file names, and return a `Cmd::OpenFile(Path)`.
pub trait CommandPaletteProvider<Cmd> {
    /// Enables switching between search and agent modes with Tab.
    fn agent_enabled(&self) -> bool {
        false
    }

    /// Action for opening agent setup, if setup is required before submitting prompts.
    fn agent_setup_action(&self) -> Option<Cmd> {
        None
    }

    /// Submit an agent prompt, rejecting blank input.
    /// Return `None` to keep the palette open without clearing the input.
    fn submit_agent_prompt(&mut self, _query: &str) -> Option<Cmd> {
        None
    }

    /// Placeholder shown when the search field is empty.
    fn search_hint(&self) -> &'static str {
        "Search commands"
    }

    /// Find commands that matches this query.
    fn all_matching(&mut self, query: &FuzzyQuery) -> Vec<MatchGroup<Cmd>>;

    /// Build the visual content of a command row.
    ///
    /// The palette handles wrapping, layout, the selection background and input;
    /// this just supplies the (already highlight-styled) text and any decorations.
    ///
    /// Tip: use [`FuzzyMatch::highlight_matching_text`] to underline the matched characters.
    fn cmd_row(&self, ui: &egui::Ui, cmd: &MatchedCmd<Cmd>, selected: bool) -> CmdRow;
}

/// A command palette where you can select a custom command.
#[derive(Default)]
pub struct CommandPalette {
    /// Is the command palette currently open?
    visible: bool,

    /// The raw thing the user entered
    raw_query: String,

    /// How to use the input ([`Mode::Search`], [`Mode::Agent`], …).
    mode: Mode,

    /// Within the selected provider
    selected_alternative: usize,

    /// Groups (by index) whose full contents should be shown,
    /// because the user clicked their "+ N more" button.
    expanded_groups: BTreeSet<usize>,

    animation_started_at: Option<f64>,
}

impl CommandPalette {
    fn reset(&mut self) {
        *self = Default::default();
    }

    pub fn toggle(&mut self) {
        self.visible ^= true;
    }

    /// Show the command palette, if it is visible.
    #[must_use = "Returns the command that was selected"]
    pub fn show<Cmd>(
        &mut self,
        egui_ctx: &egui::Context,
        provider: &mut dyn CommandPaletteProvider<Cmd>,
    ) -> Option<Cmd> {
        self.visible &= !egui_ctx.input_mut(|i| i.key_pressed(Key::Escape));
        if !self.visible {
            // Reset everything (query, selection, expanded groups) so reopening starts fresh:
            self.reset();
            return None;
        }

        let screen_rect = egui_ctx.content_rect();
        let width = 640.0.at_most(0.9 * screen_rect.width());
        let max_height = 320.0.at_most(screen_rect.height());

        let animation_started_at = *self
            .animation_started_at
            .get_or_insert_with(|| egui_ctx.input(|input| input.time));
        let scale = crate::motion::scale_for_opened_at(egui_ctx, animation_started_at);

        let modal_response = egui::Modal::new(command_palette_modal_id())
            .backdrop_color(egui::Color32::TRANSPARENT)
            .frame(egui::Frame::NONE)
            .show(egui_ctx, |_| {});

        let window_id = egui::Id::unique("Command Palette");
        let window_layer_id = egui::LayerId::new(egui::Order::Foreground, window_id);
        egui_ctx.set_sublayer(modal_response.response.layer_id, window_layer_id);

        let response = egui::Window::new("Command Palette")
            .id(window_id)
            .order(egui::Order::Foreground)
            .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
            .fixed_size([width, max_height])
            .pivot(egui::Align2::CENTER_TOP)
            .resizable(false)
            .scroll(false)
            .title_bar(false)
            .show(egui_ctx, |ui| self.window_content_ui(ui, provider))?;

        if scale < 1.0 {
            egui_ctx.transform_layer_shapes_after_rounding(
                response.response.layer_id,
                crate::motion::scale_transform(response.response.rect.center(), scale),
            );
        }

        if !self.visible {
            return response.inner?;
        }

        // Clicking or dragging anywhere outside the palette closes it.
        // We check the press (not the click) so that starting a drag outside also closes it:
        let pressed_outside = egui_ctx.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| !response.response.rect.contains(pos))
        });
        if pressed_outside {
            self.visible = false; // The `reset` happens on the next frame.
            return None;
        }

        response.inner?
    }

    #[must_use = "Returns the command that was selected"]
    fn window_content_ui<Cmd>(
        &mut self,
        ui: &mut egui::Ui,
        provider: &mut dyn CommandPaletteProvider<Cmd>,
    ) -> Option<Cmd> {
        // Allow switching to agent mode via Tab, if possible.
        let agent_enabled = provider.agent_enabled();
        if !agent_enabled {
            self.mode = Mode::Search;
        }
        let mode_changed = agent_enabled
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Tab));
        if mode_changed {
            self.mode = match self.mode {
                Mode::Search => Mode::Agent,
                Mode::Agent => Mode::Search,
            };
        }

        // Consume these _before_ we add the `TextEdit`, so it doesn't steal them
        // (e.g. arrow keys would otherwise move the text cursor instead of the selection).
        let (enter_pressed, up, down) = ui.input_mut(|i| {
            (
                i.consume_key(Default::default(), Key::Enter),
                if self.mode == Mode::Agent {
                    // No search suggestions in agent mode.
                    0
                } else {
                    i.count_and_consume_key(Default::default(), Key::ArrowUp)
                },
                if self.mode == Mode::Agent {
                    0
                } else {
                    i.count_and_consume_key(Default::default(), Key::ArrowDown)
                },
            )
        });

        let query_id = ui.make_persistent_id("query");
        let (icon, hint, name) = match self.mode {
            Mode::Search => (
                &crate::icons::SEARCH,
                provider.search_hint(),
                "Search commands",
            ),
            Mode::Agent => (
                &crate::icons::AGENT,
                "Ask a question or describe what you’d like to do.",
                "Prompt",
            ),
        };
        let text_edit = egui::TextEdit::singleline(&mut self.raw_query)
            .id(query_id)
            .hint_text(hint)
            .prefix(
                icon.as_image()
                    .fit_to_exact_size(ui.tokens().small_icon_size)
                    .tint(ui.tokens().text_strong),
            )
            .desired_width(f32::INFINITY)
            .lock_focus(true);
        // TextEdit uses the themed weak color for placeholders, overriding RichText colors.
        ui.visuals_mut().weak_text_color = Some(ui.tokens().text_subdued);
        let text_response = ui.add(text_edit).accessible_name(name);
        text_response.request_focus();
        let scroll_to_selected_alternative = mode_changed || text_response.changed();
        if scroll_to_selected_alternative {
            self.selected_alternative = 0;
            self.expanded_groups.clear();
        }

        if agent_enabled {
            let (hint, icon) = match self.mode {
                Mode::Search => ("Press Tab to switch to agent mode", &crate::icons::AGENT),
                Mode::Agent => ("Press Tab to switch to search mode", &crate::icons::SEARCH),
            };
            ui.horizontal_wrapped(|ui| {
                ui.weak(hint);
                ui.add(
                    icon.as_image()
                        .fit_to_exact_size(ui.tokens().small_icon_size)
                        .tint(ui.visuals().weak_text_color()),
                );
            });
        }

        let selected_command = if self.mode == Mode::Agent {
            // Check first if an agent is configured, otherwise guide the user to setup.
            if let Some(setup_action) = provider.agent_setup_action() {
                crate::alert::Alert::info()
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        egui::Sides::new()
                            .show(
                                ui,
                                |ui| ui.label("Set up an agent to send prompts."),
                                |ui| {
                                    ui.add(egui::Button::new("Set up agent").primary().size_small())
                                        .clicked()
                                },
                            )
                            .1
                    })
                    .inner
                    .then_some(setup_action)
            } else {
                enter_pressed
                    .then(|| provider.submit_agent_prompt(&self.raw_query))
                    .flatten()
            }
        } else {
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.alternatives_ui(
                        ui,
                        provider,
                        enter_pressed,
                        up,
                        down,
                        scroll_to_selected_alternative,
                    )
                })
                .inner
        };

        if selected_command.is_some() {
            self.reset();
        }

        selected_command
    }

    #[must_use = "Returns the command that was selected"]
    #[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
    fn alternatives_ui<Cmd>(
        &mut self,
        ui: &mut egui::Ui,
        provider: &mut dyn CommandPaletteProvider<Cmd>,
        enter_pressed: bool,
        up: usize,
        down: usize,
        mut scroll_to_selected_alternative: bool,
    ) -> Option<Cmd> {
        re_tracing::profile_function!();

        let query = FuzzyQuery::new(self.raw_query.clone());

        let mut groups = provider.all_matching(&query);

        // Put best matching group first:
        groups.sort_by_cached_key(|group| {
            std::cmp::Reverse(group.iter().map(|cmd| cmd.score()).max())
        });

        groups.retain(|g| !g.is_empty());

        let num_groups = groups.len();

        if num_groups == 0 {
            ui.weak("No matching results");
            return None;
        }

        // Sort each group (top scorers first), and truncate long groups:
        let mut num_truncated = vec![0; num_groups];
        for (group_idx, group) in groups.iter_mut().enumerate() {
            group.sort_by_key(|cmd| std::cmp::Reverse(cmd.score())); // highest first

            if !self.expanded_groups.contains(&group_idx) {
                let max_per_group = if 1 < num_groups {
                    // If there are many matching groups, then keep each group short.
                    5
                } else {
                    // A single group gets all the space, but is still capped
                    // so we don't spend CPU laying out thousands of rows.
                    50
                };
                num_truncated[group_idx] = group.len().saturating_sub(max_per_group);
                group.truncate(max_per_group);
            }
        }

        // Which alternatives are enabled (selectable), in display order.
        // A truncated group ends with a "+ N more" expand button, which is also selectable:
        let enabled: Vec<bool> = groups
            .iter()
            .enumerate()
            .flat_map(|(group_idx, group)| {
                std::iter::chain(
                    group.iter().map(|cmd| cmd.enabled),
                    (0 < num_truncated[group_idx]).then_some(true),
                )
            })
            .collect();

        // Handle keyboard navigation, skipping disabled alternatives:
        scroll_to_selected_alternative |= (up + down) != 0;

        let enabled_indices: Vec<usize> = enabled
            .iter()
            .enumerate()
            .filter_map(|(idx, &enabled)| enabled.then_some(idx))
            .collect();

        if enabled_indices.is_empty() {
            // Nothing selectable; still paint the (grayed-out) rows below.
            self.selected_alternative = usize::MAX;
        } else {
            // Snap the current selection to the nearest enabled alternative, then move:
            let mut pos = enabled_indices
                .partition_point(|&idx| idx < self.selected_alternative)
                .at_most(enabled_indices.len() - 1);
            pos = pos
                .saturating_add(down)
                .saturating_sub(up)
                .at_most(enabled_indices.len() - 1);
            self.selected_alternative = enabled_indices[pos];
        }

        let mut selected_command = None;

        let mut alternative_idx = 0; // across all groups

        for (group_idx, group) in groups.into_iter().enumerate() {
            re_tracing::profile_scope!("group_ui");

            for matched_cmd in group {
                let selected = alternative_idx == self.selected_alternative;
                let enabled = matched_cmd.enabled;

                let state = if !enabled {
                    RowState::Disabled
                } else if selected {
                    RowState::Selected
                } else {
                    RowState::Normal
                };

                let row = provider.cmd_row(ui, &matched_cmd, selected);
                let response = paint_command_row(ui, row, state);

                if selected && scroll_to_selected_alternative {
                    ui.scroll_to_rect(response.rect, None);
                }

                if enabled && (selected && enter_pressed || response.clicked()) {
                    selected_command = Some(matched_cmd.command);
                }

                alternative_idx += 1;
            }

            // A truncated group ends with a "+ N more" button that expands the group.
            // It takes part in keyboard navigation, just like the commands:
            if 0 < num_truncated[group_idx] {
                let selected = alternative_idx == self.selected_alternative;

                let text_color = if selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().weak_text_color()
                };
                let row = CmdRow {
                    job: LayoutJob::simple(
                        format!(
                            "+ {} more",
                            re_format::format_uint(num_truncated[group_idx])
                        ),
                        egui::TextStyle::Button.resolve(ui.style()),
                        text_color,
                        f32::INFINITY,
                    ),
                    kb_shortcut: String::new(),
                    tooltip: Some("Show all matches in this group".to_owned()),
                };

                let state = if selected {
                    RowState::Selected
                } else {
                    RowState::Normal
                };
                let response = paint_command_row(ui, row, state);

                if selected && scroll_to_selected_alternative {
                    ui.scroll_to_rect(response.rect, None);
                }

                if selected && enter_pressed || response.clicked() {
                    self.expanded_groups.insert(group_idx);
                }

                alternative_idx += 1;
            }

            if group_idx + 1 < num_groups {
                ui.add_space(8.0);
            }
        }

        selected_command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyProvider;

    impl CommandPaletteProvider<()> for EmptyProvider {
        fn all_matching(&mut self, _query: &FuzzyQuery) -> Vec<MatchGroup<()>> {
            Vec::new()
        }

        fn cmd_row(&self, _ui: &egui::Ui, _cmd: &MatchedCmd<()>, _selected: bool) -> CmdRow {
            unreachable!("the empty provider has no command rows")
        }
    }

    /// Returns whether any painted shape is scaled by the opening animation.
    fn render_palette(ctx: &egui::Context, palette: &mut CommandPalette, time: f64) -> bool {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(palette.show(ui.ctx(), &mut EmptyProvider).is_none());
            },
        );
        output.textures_delta.clear();

        output
            .shapes
            .iter()
            .any(|clipped| clipped.transform_after_tessellation.scaling < 1.0)
    }

    #[test]
    fn command_palette_animation_restarts_after_closing() {
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|style| style.animation_time = 0.2);
        let mut palette = CommandPalette::default();

        palette.toggle();
        render_palette(&ctx, &mut palette, 0.0);
        assert!(
            render_palette(&ctx, &mut palette, 0.016),
            "the opening command palette should be scaled"
        );
        assert_eq!(
            ctx.memory(|memory| memory.top_modal_layer()),
            Some(egui::LayerId::new(
                egui::Order::Foreground,
                command_palette_modal_id(),
            )),
            "the command palette should block interaction with lower layers"
        );

        palette.toggle();
        assert!(!render_palette(&ctx, &mut palette, 0.032));

        palette.toggle();
        render_palette(&ctx, &mut palette, 0.048);
        assert!(
            render_palette(&ctx, &mut palette, 0.064),
            "the reopened command palette should restart its animation"
        );

        assert!(!render_palette(&ctx, &mut palette, 1.0));
    }
}
