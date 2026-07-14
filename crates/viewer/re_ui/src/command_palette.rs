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

use egui::text::LayoutJob;
use egui::{Key, NumExt as _};

use crate::fuzzy::{FuzzyMatch, FuzzyQuery};

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

    // Pad the row vertically so the selection background extends a bit beyond the text,
    // keeping the match-highlighting underline clearly visible on the selected row:
    ui.spacing_mut().button_padding.y = 2.0;
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

/// One source of commands.
///
/// The type of the command is up to the user to provide.
///
/// For instance: match file names, and return a `Cmd::OpenFile(Path)`.
pub trait CommandPaletteProvider<Cmd> {
    /// Show when the query field is empty (the user hasn't typed anything yet).
    fn initial_hint_ui(&mut self, _ui: &mut egui::Ui) {}

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

    /// Within the selected provider
    selected_alternative: usize,

    /// Groups (by index) whose full contents should be shown,
    /// because the user clicked their "+ N more" button.
    expanded_groups: BTreeSet<usize>,
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

        let response = egui::Window::new("Command Palette")
            .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
            .fixed_size([width, max_height])
            .pivot(egui::Align2::CENTER_TOP)
            .resizable(false)
            .scroll(false)
            .title_bar(false)
            .show(egui_ctx, |ui| self.window_content_ui(ui, provider))?;

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
        // Consume these _before_ we add the `TextEdit`, so it doesn't steal them
        // (e.g. arrow keys would otherwise move the text cursor instead of the selection).
        let (enter_pressed, up, down) = ui.input_mut(|i| {
            (
                i.consume_key(Default::default(), Key::Enter),
                i.count_and_consume_key(Default::default(), Key::ArrowUp),
                i.count_and_consume_key(Default::default(), Key::ArrowDown),
            )
        });

        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.raw_query)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        text_response.request_focus();
        let scroll_to_selected_alternative = if text_response.changed() {
            self.selected_alternative = 0;
            self.expanded_groups.clear();
            true
        } else {
            false
        };

        let selected_command = egui::ScrollArea::vertical()
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
            .inner;

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

        if query.is_empty() {
            provider.initial_hint_ui(ui);
        }

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
