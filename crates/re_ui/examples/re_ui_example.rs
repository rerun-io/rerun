use re_ui::{Command, CommandPalette};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1200.0, 800.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: re_ui::FULLSIZE_CONTENT,

        // Maybe hide the OS-specific "chrome" around the window:
        decorated: !re_ui::CUSTOM_WINDOW_DECORATIONS,
        // To have rounded corners we need transparency:
        transparent: re_ui::CUSTOM_WINDOW_DECORATIONS,

        ..Default::default()
    };

    let tree = egui_dock::Tree::new(vec![1, 2, 3]);

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            let re_ui = re_ui::ReUi::load_and_apply(&cc.egui_ctx);
            Box::new(ExampleApp {
                re_ui,

                tree,

                left_panel: true,
                right_panel: true,
                bottom_panel: true,

                dummy_bool: true,

                cmd_palette: CommandPalette::default(),
                pending_commands: Default::default(),
                latest_cmd: Default::default(),
            })
        }),
    )
}

pub struct ExampleApp {
    re_ui: re_ui::ReUi,

    tree: egui_dock::Tree<Tab>,

    left_panel: bool,
    right_panel: bool,
    bottom_panel: bool,

    dummy_bool: bool,

    cmd_palette: CommandPalette,

    /// Commands to run at the end of the frame.
    pending_commands: Vec<Command>,
    latest_cmd: String,
}

impl eframe::App for ExampleApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::gui_zoom::zoom_with_keyboard_shortcuts(
            egui_ctx,
            frame.info().native_pixels_per_point,
        );

        self.top_bar(egui_ctx, frame);

        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(self.re_ui.bottom_panel_frame())
            .show_animated(egui_ctx, self.bottom_panel, |ui| {
                ui.strong("Bottom panel");
            });

        egui::SidePanel::left("left_panel")
            .default_width(500.0)
            .frame(egui::Frame {
                fill: egui_ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show_animated(egui_ctx, self.left_panel, |ui| {
                egui::TopBottomPanel::top("left_panel_tio_bar")
                    .exact_height(re_ui::ReUi::title_bar_height())
                    .frame(egui::Frame {
                        inner_margin: egui::Margin::symmetric(re_ui::ReUi::view_padding(), 0.0),
                        ..Default::default()
                    })
                    .show_inside(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.strong("Left bar");
                        });
                    });

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        egui::Frame {
                            inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Toggle switch:");
                                ui.add(re_ui::toggle_switch(&mut self.dummy_bool));
                            });
                            ui.label(format!("Latest command: {}", self.latest_cmd));

                            self.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                                ui.label("Some data here");
                            });
                            self.re_ui
                                .large_collapsing_header(ui, "Blueprint", true, |ui| {
                                    ui.style_mut().wrap = Some(false);
                                    ui.label("Some blueprint stuff here, that might be wide.");
                                });
                        });
                    });
            });

        let panel_frame = egui::Frame {
            fill: egui_ctx.style().visuals.panel_fill,
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..Default::default()
        };

        egui::SidePanel::right("right_panel")
            .frame(panel_frame)
            .show_animated(egui_ctx, self.right_panel, |ui| {
                ui.strong("Right panel");
                selection_buttons(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui_ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show(egui_ctx, |ui| {
                tabs_ui(ui, &mut self.tree);
            });

        if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
            self.pending_commands.push(cmd);
        }
        if let Some(cmd) = re_ui::Command::listen_for_kb_shortcut(egui_ctx) {
            self.pending_commands.push(cmd);
        }

        for cmd in self.pending_commands.drain(..) {
            self.latest_cmd = cmd.text().to_owned();

            #[allow(clippy::single_match)]
            match cmd {
                Command::ToggleCommandPalette => self.cmd_palette.toggle(),
                _ => {}
            }
        }
    }
}

impl ExampleApp {
    fn top_bar(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let native_pixels_per_point = frame.info().native_pixels_per_point;
        let fullscreen = {
            #[cfg(target_arch = "wasm32")]
            {
                false
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                frame.info().window_info.fullscreen
            }
        };
        let top_bar_style = self
            .re_ui
            .top_bar_style(native_pixels_per_point, fullscreen);

        egui::TopBottomPanel::top("top_bar")
            .frame(self.re_ui.top_panel_frame())
            .exact_height(top_bar_style.height)
            .show(egui_ctx, |ui| {
                let _response = egui::menu::bar(ui, |ui| {
                    ui.set_height(top_bar_style.height);
                    ui.add_space(top_bar_style.indent);

                    ui.menu_button("File", |ui| file_menu(ui, &mut self.pending_commands));

                    self.top_bar_ui(ui, frame);
                })
                .response;

                #[cfg(not(target_arch = "wasm32"))]
                if !re_ui::NATIVE_WINDOW_BAR {
                    let title_bar_response = _response.interact(egui::Sense::click());
                    if title_bar_response.double_clicked() {
                        frame.set_maximized(!frame.info().window_info.maximized);
                    } else if title_bar_response.is_pointer_button_down_on() {
                        frame.drag_window();
                    }
                }
            });
    }

    fn top_bar_ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // From right-to-left:

            if re_ui::CUSTOM_WINDOW_DECORATIONS {
                ui.add_space(8.0);
                re_ui::native_window_buttons_ui(frame, ui);
                ui.separator();
            } else {
                ui.add_space(16.0);
            }

            self.re_ui.medium_icon_toggle_button(
                ui,
                &re_ui::icons::RIGHT_PANEL_TOGGLE,
                &mut self.right_panel,
            );
            self.re_ui.medium_icon_toggle_button(
                ui,
                &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                &mut self.bottom_panel,
            );
            self.re_ui.medium_icon_toggle_button(
                ui,
                &re_ui::icons::LEFT_PANEL_TOGGLE,
                &mut self.left_panel,
            );
        });
    }
}

fn file_menu(ui: &mut egui::Ui, pending_commands: &mut Vec<Command>) {
    Command::Save.menu_button_ui(ui, pending_commands);
    Command::SaveSelection.menu_button_ui(ui, pending_commands);
    Command::Open.menu_button_ui(ui, pending_commands);
    Command::Quit.menu_button_ui(ui, pending_commands);
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

fn tabs_ui(ui: &mut egui::Ui, tree: &mut egui_dock::Tree<Tab>) {
    let mut my_tab_viewer = MyTabViewer {};
    egui_dock::DockArea::new(tree)
        .style(re_ui::egui_dock_style(ui.style()))
        .show_inside(ui, &mut my_tab_viewer);
}

pub type Tab = i32;

struct MyTabViewer {}

impl egui_dock::TabViewer for MyTabViewer {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut egui::Ui, _tab: &mut Self::Tab) {
        egui::warn_if_debug_build(ui);
        ui.label("Hover me for a tooltip")
            .on_hover_text("This is a tooltip");
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("This is tab {tab}").into()
    }
}
