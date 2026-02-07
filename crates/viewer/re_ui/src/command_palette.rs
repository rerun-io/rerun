use std::collections::BTreeSet;

use egui::{Align2, Key, NumExt as _};

use crate::UICommand;

#[derive(Default)]
pub struct CommandPalette {
    visible: bool,
    query: String,
    selected_alternative: usize,
}

/// Either a command, or a URL that we want to open.
///
/// URL opening is the fallback for the command palette and needs some special treatment since
/// ui commands usually don't have arbitrary state.
#[derive(Clone)]
pub enum CommandPaletteAction {
    UiCommand(UICommand),
    OpenUrl(CommandPaletteUrl),
}

#[derive(Clone)]
pub struct CommandPaletteUrl {
    /// The URL that should be opened.
    pub url: String,

    /// Text that describes the command of opening this URL.
    pub command_text: String,
}

impl CommandPaletteAction {
    fn text(&self) -> &str {
        match self {
            Self::UiCommand(command) => command.text(),
            Self::OpenUrl(url) => &url.command_text,
        }
    }

    fn tooltip(&self) -> &'static str {
        match self {
            Self::UiCommand(command) => command.tooltip(),
            Self::OpenUrl(_) => {
                "Try to open this URL in the viewer. If the contents are already loaded, this will select them."
            }
        }
    }

    fn formatted_kb_shortcut(&self, egui_ctx: &egui::Context) -> Option<String> {
        match self {
            Self::UiCommand(command) => command.formatted_kb_shortcut(egui_ctx),
            Self::OpenUrl(_) => None,
        }
    }
}

impl CommandPalette {
    pub fn toggle(&mut self) {
        self.visible ^= true;
    }

    /// Show the command palette, if it is visible.
    #[must_use = "Returns the command that was selected"]
    pub fn show(
        &mut self,
        egui_ctx: &egui::Context,
        parse_url: &dyn Fn(&str) -> Option<CommandPaletteUrl>,
    ) -> Option<CommandPaletteAction> {
        self.visible &= !egui_ctx.input_mut(|i| i.key_pressed(Key::Escape));
        if !self.visible {
            self.query.clear();
            return None;
        }

        let screen_rect = egui_ctx.content_rect();
        let width = 300.0;
        let max_height = 320.0.at_most(screen_rect.height());

        egui::Window::new("Command Palette")
            .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
            .fixed_size([width, max_height])
            .pivot(egui::Align2::CENTER_TOP)
            .resizable(false)
            .scroll(false)
            .title_bar(false)
            .show(egui_ctx, |ui| {
                // We need an extra egui frame here because we set clip_rect_margin to zero.
                egui::Frame {
                    inner_margin: 2.0.into(),
                    ..Default::default()
                }
                .show(ui, |ui| self.window_content_ui(ui, parse_url))
                .inner
            })?
            .inner?
    }

    #[must_use = "Returns the command that was selected"]
    fn window_content_ui(
        &mut self,
        ui: &mut egui::Ui,
        parse_url: &dyn Fn(&str) -> Option<CommandPaletteUrl>,
    ) -> Option<CommandPaletteAction> {
        // Check _before_ we add the `TextEdit`, so it doesn't steal it.
        let enter_pressed = ui.input_mut(|i| i.consume_key(Default::default(), Key::Enter));

        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        text_response.request_focus();
        let scroll_to_selected_alternative = if text_response.changed() {
            self.selected_alternative = 0;
            true
        } else {
            false
        };

        let selected_command = egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.alternatives_ui(ui, enter_pressed, scroll_to_selected_alternative, parse_url)
            })
            .inner;

        if selected_command.is_some() {
            *self = Default::default();
        }

        selected_command
    }

    #[must_use = "Returns the command that was selected"]
    #[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
    fn alternatives_ui(
        &mut self,
        ui: &mut egui::Ui,
        enter_pressed: bool,
        mut scroll_to_selected_alternative: bool,
        parse_url: &dyn Fn(&str) -> Option<CommandPaletteUrl>,
    ) -> Option<CommandPaletteAction> {
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowUp));
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowDown));

        let item_height = 16.0;
        let font_id = egui::TextStyle::Button.resolve(ui.style());

        let mut num_alternatives: usize = 0;
        let mut selected_command = None;

        for (i, fuzzy_match) in commands_that_match(&self.query, parse_url)
            .into_iter()
            .enumerate()
        {
            let command = fuzzy_match.command.clone();
            let kb_shortcut_text = command.formatted_kb_shortcut(ui.ctx()).unwrap_or_default();

            let (rect, response) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), item_height),
                egui::Sense::click(),
            );

            let response = response.on_hover_text(command.tooltip());

            if response.clicked() {
                selected_command = Some(command.clone());
            }

            let selected = i == self.selected_alternative;
            let style = ui.style().interact_selectable(&response, selected);

            if selected {
                ui.painter().rect_filled(
                    rect.expand(1.0),
                    style.corner_radius,
                    ui.visuals().selection.bg_fill,
                );

                if enter_pressed {
                    selected_command = Some(command);
                }

                if scroll_to_selected_alternative {
                    ui.scroll_to_rect(rect, None);
                }
            }

            let text = format_match(&fuzzy_match, &font_id, style.text_color());

            // TODO(emilk): shorten long text using '…'
            let galley = text.into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::FontSelection::default(),
            );
            let text_rect = Align2::LEFT_CENTER
                .anchor_rect(egui::Rect::from_min_size(rect.left_center(), galley.size()));
            ui.painter()
                .galley(text_rect.min, galley, style.text_color());

            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                kb_shortcut_text,
                font_id.clone(),
                if selected {
                    style.text_color()
                } else {
                    ui.visuals().weak_text_color()
                },
            );

            num_alternatives += 1;
        }

        if num_alternatives == 0 {
            ui.weak("No matching results");
        }

        // Move up/down in the list:
        self.selected_alternative = self.selected_alternative.saturating_sub(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp)),
        );
        self.selected_alternative = self.selected_alternative.saturating_add(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown)),
        );

        self.selected_alternative = self
            .selected_alternative
            .clamp(0, num_alternatives.saturating_sub(1));

        selected_command
    }
}

struct FuzzyMatch {
    command: CommandPaletteAction,
    score: isize,
    fuzzy_match: Option<sublime_fuzzy::Match>,
}

fn commands_that_match(
    query: &str,
    parse_url: &dyn Fn(&str) -> Option<CommandPaletteUrl>,
) -> Vec<FuzzyMatch> {
    use strum::IntoEnumIterator as _;

    if query.is_empty() {
        UICommand::iter()
            .map(|command| FuzzyMatch {
                command: CommandPaletteAction::UiCommand(command),
                score: 0,
                fuzzy_match: None,
            })
            .collect()
    } else {
        let query_lowercase = query.to_lowercase();
        let mut matches: Vec<_> = UICommand::iter()
            .filter_map(|command| {
                let target_text = command.text();
                sublime_fuzzy::best_match(&query_lowercase, target_text).map(|fuzzy_match| {
                    FuzzyMatch {
                        command: CommandPaletteAction::UiCommand(command),
                        score: fuzzy_match.score(),
                        fuzzy_match: Some(fuzzy_match),
                    }
                })
            })
            .collect();

        // Add the special open URL command.
        if let Some(url) = parse_url(query) {
            matches.push(FuzzyMatch {
                command: CommandPaletteAction::OpenUrl(url),
                score: -1,
                fuzzy_match: None,
            });
        }

        matches.sort_by_key(|m| -m.score); // highest score first
        matches
    }
}

fn format_match(
    m: &FuzzyMatch,
    font_id: &egui::FontId,
    text_color: egui::Color32,
) -> egui::WidgetText {
    let target_text = m.command.text();

    if let Some(fm) = &m.fuzzy_match {
        let matched_indices: BTreeSet<_> = fm.matched_indices().collect();

        let mut job = egui::text::LayoutJob::default();
        for (i, c) in target_text.chars().enumerate() {
            let mut format = egui::text::TextFormat::simple(font_id.clone(), text_color);
            if matched_indices.contains(&i) {
                format.underline = egui::Stroke::new(1.0, text_color);
            }
            job.append(&c.to_string(), 0.0, format);
        }

        job.into()
    } else {
        egui::RichText::new(target_text).color(text_color).into()
    }
}
