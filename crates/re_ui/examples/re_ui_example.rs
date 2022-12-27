use re_ui::CommandPalette;

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

                latest_cmd: Default::default(),
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

    latest_cmd: String,
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

            ui.label(format!("Latest command: {}", self.latest_cmd));
        });

        if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
            self.latest_cmd = cmd.text().to_owned();
        }
        if let Some(cmd) = re_ui::Command::listen_for_kb_shortcut(egui_ctx) {
            self.latest_cmd = cmd.text().to_owned();
        }
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
