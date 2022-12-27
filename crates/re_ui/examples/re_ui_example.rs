use egui::{Align2, Key, KeyboardShortcut, Modifiers};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1200.0, 800.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: re_ui::FULLSIZE_CONTENT,

        ..Default::default()
    };

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            let re_ui = re_ui::ReUi::load_and_apply(&cc.egui_ctx);
            Box::new(ExampleApp {
                re_ui,

                left_panel: true,
                right_panel: true,
                bottom_panel: true,

                dummy_bool: true,

                cmd_palette: CommandPalette::default(),
            })
        }),
    )
}

pub struct ExampleApp {
    re_ui: re_ui::ReUi,

    left_panel: bool,
    right_panel: bool,
    bottom_panel: bool,

    dummy_bool: bool,

    cmd_palette: CommandPalette,
}

impl eframe::App for ExampleApp {
    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::gui_zoom::zoom_with_keyboard_shortcuts(
            egui_ctx,
            frame.info().native_pixels_per_point,
        );

        self.top_bar(egui_ctx, frame);

        egui::SidePanel::left("left_panel").show_animated(egui_ctx, self.left_panel, |ui| {
            ui.label("Left panel");
            ui.horizontal(|ui| {
                ui.label("Toggle switch:");
                ui.add(re_ui::toggle_switch(&mut self.dummy_bool));
            });
        });
        egui::SidePanel::right("right_panel").show_animated(egui_ctx, self.right_panel, |ui| {
            ui.label("Right panel");
            selection_buttons(ui);
        });
        egui::TopBottomPanel::bottom("bottom_panel").show_animated(
            egui_ctx,
            self.bottom_panel,
            |ui| {
                ui.label("Bottom panel");
            },
        );

        egui::CentralPanel::default().show(egui_ctx, |ui| {
            egui::warn_if_debug_build(ui);
            ui.label("Hover me for a tooltip")
                .on_hover_text("This is a tooltip");
        });

        self.cmd_palette.show(egui_ctx);
    }
}

impl ExampleApp {
    fn top_bar(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let panel_frame = {
            egui::Frame {
                inner_margin: egui::style::Margin::symmetric(8.0, 2.0),
                fill: self.re_ui.design_tokens.top_bar_color,
                ..Default::default()
            }
        };

        let native_pixels_per_point = frame.info().native_pixels_per_point;
        let fullscreen = {
            #[cfg(target_os = "macos")]
            {
                frame.info().window_info.fullscreen
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        };
        let top_bar_style = self
            .re_ui
            .top_bar_style(native_pixels_per_point, fullscreen);

        egui::TopBottomPanel::top("top_bar")
            .frame(panel_frame)
            .exact_height(top_bar_style.height)
            .show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.set_height(top_bar_style.height);
                    ui.add_space(top_bar_style.indent);

                    self.re_ui.medium_icon_toggle_button(
                        ui,
                        &re_ui::icons::LEFT_PANEL_TOGGLE,
                        &mut self.left_panel,
                    );
                    self.re_ui.medium_icon_toggle_button(
                        ui,
                        &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                        &mut self.bottom_panel,
                    );
                    self.re_ui.medium_icon_toggle_button(
                        ui,
                        &re_ui::icons::RIGHT_PANEL_TOGGLE,
                        &mut self.right_panel,
                    );

                    ui.centered_and_justified(|ui| {
                        ui.strong("re_ui example app");
                    })
                });
            });
    }
}

fn selection_buttons(ui: &mut egui::Ui) {
    use egui_extras::{Size, StripBuilder};

    const BUTTON_SIZE: f32 = 20.0;
    const MIN_COMBOBOX_SIZE: f32 = 100.0;

    ui.horizontal(|ui| {
        StripBuilder::new(ui)
            .cell_layout(egui::Layout::centered_and_justified(
                egui::Direction::TopDown, // whatever
            ))
            .size(Size::exact(BUTTON_SIZE)) // prev
            .size(Size::remainder().at_least(MIN_COMBOBOX_SIZE)) // browser
            .size(Size::exact(BUTTON_SIZE)) // next
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    let _ = ui.small_button("⏴");
                });

                strip.cell(|ui| {
                    egui::ComboBox::from_id_source("foo")
                        .width(ui.available_width())
                        .selected_text("ComboBox")
                        .show_ui(ui, |ui| {
                            ui.label("contents");
                        });
                });

                strip.cell(|ui| {
                    let _ = ui.small_button("⏵");
                });
            });
    });
}

// ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
enum Command {
    /// In the order they show up in the command palette by default!
    Save,
    SaveSelection,
    Open,
    Quit,
    ResetViewer,
    ShowProfiler,

    ToggleMemoryPanel,
    ToggleBlueprintPanel,
    ToggleSelectionPanel,
    ToggleTimePanel,
    ToggleFullscreen,

    SelectionPrevious,
    SelectionNext,

    ToggleCommandPalette,

    Test,
}

impl Command {
    fn text(&self) -> &'static str {
        match self {
            Command::Save => "Save",
            Command::SaveSelection => "Save selection",
            Command::Open => "Open",
            Command::Quit => "Quit",
            Command::ResetViewer => "Reset viewer",
            Command::ShowProfiler => "Show profiler",
            Command::ToggleMemoryPanel => "Toggle memory panel",
            Command::ToggleBlueprintPanel => "Toggle blueprint panel",
            Command::ToggleSelectionPanel => "Toggle selection panel",
            Command::ToggleTimePanel => "Toggle time panel",
            Command::ToggleFullscreen => "Toggle fullscreen",
            Command::SelectionPrevious => "Selection previous",
            Command::SelectionNext => "Selection next",
            Command::ToggleCommandPalette => "Toggle command palette",
            Command::Test => "Test",
        }
    }

    fn kb_shortcut(&self) -> Option<KeyboardShortcut> {
        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), key)
        }

        match self {
            Command::Save => Some(cmd(Key::S)),
            Command::SaveSelection => Some(cmd_shift(Key::S)),
            Command::Open => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::O)),

            #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
            Command::Quit => Some(KeyboardShortcut::new(Modifiers::ALT, Key::F4)),

            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
            Command::Quit => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Q)),

            Command::ResetViewer => Some(ctrl_shift(Key::R)),
            Command::ShowProfiler => Some(ctrl_shift(Key::P)),
            Command::ToggleMemoryPanel => Some(ctrl_shift(Key::M)),
            Command::ToggleBlueprintPanel => Some(ctrl_shift(Key::B)),
            Command::ToggleSelectionPanel => Some(ctrl_shift(Key::S)),
            Command::ToggleTimePanel => Some(ctrl_shift(Key::T)),
            Command::ToggleFullscreen => Some(ctrl_shift(Key::ArrowLeft)),
            Command::SelectionPrevious => Some(ctrl_shift(Key::ArrowRight)),
            Command::SelectionNext => Some(KeyboardShortcut::new(Modifiers::NONE, Key::F11)),
            Command::ToggleCommandPalette => Some(cmd(Key::P)),

            Command::Test => None,
        }
    }
}

#[derive(Default)]
struct CommandPalette {
    visible: bool,
    text: String,
    selected_alternative: usize,
}

impl CommandPalette {
    #[must_use = "Returns the command that was selected"]
    fn show(&mut self, egui_ctx: &egui::Context) -> Option<Command> {
        let kb_shortcut = Command::ToggleCommandPalette
            .kb_shortcut()
            .expect("We need a keybloard shortcut for the command palette");
        self.visible ^= egui_ctx.input_mut().consume_shortcut(&kb_shortcut);
        self.visible &= !egui_ctx
            .input_mut()
            .consume_key(Default::default(), Key::Escape);
        if !self.visible {
            return None;
        }

        let max_height = 320.0;
        let y = egui_ctx.input().screen_rect().center().y - 0.5 * max_height;

        egui::Window::new("Command Palette")
            .title_bar(false)
            .anchor(Align2::CENTER_TOP, [0.0, y])
            .fixed_size([350.0, max_height])
            .show(egui_ctx, |ui| self.window_content(ui))?
            .inner?
    }

    #[must_use = "Returns the command that was selected"]
    fn window_content(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        // Check _before_ we add the `TextEdit`, so it doesn't steal it.
        let enter_pressed = ui.input_mut().consume_key(Default::default(), Key::Enter);
        if enter_pressed {
            self.visible = false;
        }

        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.text)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        text_response.request_focus();
        if text_response.changed() {
            self.selected_alternative = 0;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| self.alternatives(ui, enter_pressed))
            .inner
    }

    #[must_use = "Returns the command that was selected"]
    fn alternatives(&mut self, ui: &mut egui::Ui, enter_pressed: bool) -> Option<Command> {
        use strum::IntoEnumIterator as _;
        let mut num_alternatives: usize = 0;

        // TODO(emilk): nicer filtering
        let filter = self.text.to_lowercase();

        let item_height = 16.0;
        let font_id = egui::TextStyle::Button.resolve(ui.style());

        let mut selected_command = None;

        for (i, command) in Command::iter()
            .filter(|alt| alt.text().to_lowercase().contains(&filter))
            .enumerate()
        {
            let text = command.text();
            let kb_shortcut = command
                .kb_shortcut()
                .map(|shortcut| ui.ctx().format_shortcut(&shortcut))
                .unwrap_or_default();

            let (rect, response) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), item_height),
                egui::Sense::click(),
            );

            if response.clicked() {
                selected_command = Some(command);
            }

            let selected = i == self.selected_alternative;
            let style = ui.style().interact_selectable(&response, selected);

            if selected {
                ui.painter().rect_filled(
                    rect,
                    re_ui::ReUi::small_rounding(),
                    ui.visuals().selection.bg_fill,
                );

                if enter_pressed {
                    selected_command = Some(command);
                }

                ui.scroll_to_rect(rect, None);
            }

            // TODO(emilk): shorten long text using '…'
            ui.painter().text(
                rect.left_center(),
                Align2::LEFT_CENTER,
                text,
                font_id.clone(),
                style.text_color(),
            );
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                kb_shortcut,
                font_id.clone(),
                if selected {
                    style.text_color()
                } else {
                    ui.visuals().weak_text_color()
                },
            );

            num_alternatives += 1;
        }

        // Move up/down in the list:

        self.selected_alternative = self.selected_alternative.saturating_sub(
            ui.input_mut()
                .count_and_consume_key(Default::default(), Key::ArrowUp),
        );

        self.selected_alternative = self.selected_alternative.saturating_add(
            ui.input_mut()
                .count_and_consume_key(Default::default(), Key::ArrowDown),
        );

        self.selected_alternative = self
            .selected_alternative
            .clamp(0, num_alternatives.saturating_sub(1));

        selected_command
    }
}
