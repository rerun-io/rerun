mod drag_and_drop;
mod hierarchical_drag_and_drop;
mod right_panel;

use re_ui::notifications;
use re_ui::{
    list_item, CommandPalette, ContextExt as _, DesignTokens, UICommand, UICommandSender,
    UiExt as _,
};

/// Sender that queues up the execution of a command.
pub struct CommandSender(std::sync::mpsc::Sender<UICommand>);

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.0.send(command).ok();
    }
}

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver(std::sync::mpsc::Receiver<UICommand>);

impl CommandReceiver {
    /// Receive a command to be executed if any is queued.
    pub fn recv(&self) -> Option<UICommand> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.0.try_recv().ok()
    }
}

/// Creates a new command channel.
fn command_channel() -> (CommandSender, CommandReceiver) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (CommandSender(sender), CommandReceiver(receiver))
}

fn main() -> eframe::Result {
    re_log::setup_logging();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("re_ui_example")
            .with_decorations(!re_ui::CUSTOM_WINDOW_DECORATIONS) // Maybe hide the OS-specific "chrome" around the window
            .with_fullsize_content_view(re_ui::FULLSIZE_CONTENT)
            .with_inner_size([1200.0, 800.0])
            .with_title_shown(!re_ui::FULLSIZE_CONTENT)
            .with_titlebar_buttons_shown(!re_ui::CUSTOM_WINDOW_DECORATIONS)
            .with_titlebar_shown(!re_ui::FULLSIZE_CONTENT)
            .with_transparent(re_ui::CUSTOM_WINDOW_DECORATIONS), // To have rounded corners without decorations we need transparency

        ..Default::default()
    };

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            re_ui::apply_style_and_install_loaders(&cc.egui_ctx);
            Ok(Box::new(ExampleApp::new()))
        }),
    )
}

pub struct ExampleApp {
    notifications: notifications::NotificationUi,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    tree: egui_tiles::Tree<Tab>,

    /// regular modal
    modal_handler: re_ui::modal::ModalHandler,

    /// modal with full span mode
    full_span_modal_handler: re_ui::modal::ModalHandler,

    show_left_panel: bool,
    show_right_panel: bool,
    show_bottom_panel: bool,

    right_panel: right_panel::RightPanel,

    dummy_bool: bool,

    cmd_palette: CommandPalette,

    /// Commands to run at the end of the frame.
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
    latest_cmd: String,
}

impl ExampleApp {
    fn new() -> Self {
        let (logger, text_log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Info);
        re_log::add_boxed_logger(Box::new(logger)).expect("Failed to add logger");

        let tree = egui_tiles::Tree::new_tabs("my_tree", vec![1, 2, 3]);

        let (command_sender, command_receiver) = command_channel();

        Self {
            notifications: Default::default(),
            text_log_rx,

            tree,
            modal_handler: Default::default(),
            full_span_modal_handler: Default::default(),

            show_left_panel: true,
            show_right_panel: true,
            show_bottom_panel: true,

            right_panel: right_panel::RightPanel::default(),

            dummy_bool: true,

            cmd_palette: CommandPalette::default(),
            command_sender,
            command_receiver,
            latest_cmd: Default::default(),
        }
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        while let Ok(message) = self.text_log_rx.try_recv() {
            self.notifications.add_log(message);
        }
    }
}

impl eframe::App for ExampleApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn update(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_text_logs_as_notifications();

        self.top_bar(egui_ctx);

        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(DesignTokens::bottom_panel_frame())
            .show_animated(egui_ctx, self.show_bottom_panel, |ui| {
                ui.strong("Bottom panel");
            });

        // LEFT PANEL

        // top section closure
        let left_panel_top_section_ui = |ui: &mut egui::Ui| {
            ui.horizontal_centered(|ui| {
                ui.strong("Left bar");

                if ui.button("Log info").clicked() {
                    re_log::info!(
                        "A lot of text on info level.\nA lot of text in fact. So \
                             much that we should ideally be auto-wrapping it at some point, much \
                             earlier than this."
                    );
                }
                if ui.button("Log warn").clicked() {
                    re_log::warn!(
                        "A lot of text on warn level.\nA lot of text in fact."
                    );
                }
                if ui.button("Log error").clicked() {
                    re_log::error!(
                        "A lot of text on error level.\nA lot of text in fact. \
                            So much that we should ideally be auto-wrapping it at some point, much \
                            earlier than this. Lorem ipsum sit dolor amet. Lorem ipsum sit dolor amet. \
                            Lorem ipsum sit dolor amet. Lorem ipsum sit dolor amet. Lorem ipsum sit dolor amet. \
                            Lorem ipsum sit dolor amet."
                    );
                }
            });
        };

        // bottom section closure
        let left_panel_bottom_section_ui = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.label("Toggle switch:");
                ui.toggle_switch(8.0, &mut self.dummy_bool);
            });
            ui.label(format!("Latest command: {}", self.latest_cmd));

            ui.selectable_toggle(|ui| {
                ui.selectable_value(&mut self.dummy_bool, false, "Inactive");
                ui.selectable_value(&mut self.dummy_bool, true, "Active");
            });

            // ---

            if ui.button("Open modal").clicked() {
                self.modal_handler.open();
            }

            self.modal_handler.ui(
                ui.ctx(),
                || re_ui::modal::ModalWrapper::new("Modal window"),
                |ui, _| ui.label("This is a modal window."),
            );

            // ---

            if ui.button("Open full span modal").clicked() {
                self.full_span_modal_handler.open();
            }

            self.full_span_modal_handler.ui(
                ui.ctx(),
                || re_ui::modal::ModalWrapper::new("Modal window").full_span_content(true),
                |ui, _| {
                    list_item::list_item_scope(ui, "modal demo", |ui| {
                        for idx in 0..10 {
                            list_item::ListItem::new()
                                .show_flat(ui, list_item::LabelContent::new(format!("Item {idx}")));
                        }
                    });
                },
            );

            // ---

            ui.section_collapsing_header("Data")
                .button(list_item::ItemMenuButton::new(&re_ui::icons::ADD, |ui| {
                    ui.weak("empty");
                }))
                .show(ui, |ui| {
                    ui.label("Some data here");
                });
            ui.section_collapsing_header("Blueprint").show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label("Some blueprint stuff here, that might be wide.");
                ui.re_checkbox(&mut self.dummy_bool, "Checkbox");

                ui.collapsing_header("Collapsing header", true, |ui| {
                    ui.label("Some data here");
                    ui.re_checkbox(&mut self.dummy_bool, "Checkbox");
                });
            });
        };

        // UI code
        egui::SidePanel::left("left_panel")
            .default_width(500.0)
            .frame(egui::Frame {
                fill: egui_ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show_animated(egui_ctx, self.show_left_panel, |ui| {
                let y_spacing = ui.spacing().item_spacing.y;

                list_item::list_item_scope(ui, "left_panel", |ui| {
                    // revert change by `list_item_scope`
                    ui.spacing_mut().item_spacing.y = y_spacing;
                    egui::TopBottomPanel::top("left_panel_top_bar")
                        .exact_height(re_ui::DesignTokens::title_bar_height())
                        .frame(egui::Frame {
                            inner_margin: egui::Margin::symmetric(
                                re_ui::DesignTokens::view_padding(),
                                0.0,
                            ),
                            ..Default::default()
                        })
                        .show_inside(ui, left_panel_top_section_ui);

                    egui::ScrollArea::both()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            egui::Frame {
                                inner_margin: egui::Margin::same(
                                    re_ui::DesignTokens::view_padding(),
                                ),
                                ..Default::default()
                            }
                            .show(ui, left_panel_bottom_section_ui);
                        });
                });
            });

        // RIGHT PANEL
        //
        // This is the "idiomatic" panel structure for Rerun:
        // - A top-level `SidePanel` without inner margins and which sets the clip rectangle.
        // - Every piece of content (title bar, lists, etc.) are wrapped in a `Frame` with inner
        //   margins set to `ReUi::panel_margins()`. That can be done with `ReUi::panel_content()`.
        // - If/when a scroll area is used, it must be applied without margin and outside of the
        //   `Frame`.
        //
        // This way, the content (titles, etc.) is properly inset and benefits from a properly set
        // full-span scope, without interference from the scroll areas.

        let panel_frame = egui::Frame {
            fill: egui_ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::SidePanel::right("right_panel")
            .frame(panel_frame)
            .min_width(0.0)
            .show_animated(egui_ctx, self.show_right_panel, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.right_panel.ui(ui);
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
            self.command_sender.send_ui(cmd);
        }
        if let Some(cmd) = re_ui::UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }

        while let Some(cmd) = self.command_receiver.recv() {
            self.latest_cmd = cmd.text().to_owned();

            #[allow(clippy::single_match)]
            match cmd {
                UICommand::ToggleCommandPalette => self.cmd_palette.toggle(),
                UICommand::ZoomIn => {
                    let mut zoom_factor = egui_ctx.zoom_factor();
                    zoom_factor += 0.1;
                    egui_ctx.set_zoom_factor(zoom_factor);
                }
                UICommand::ZoomOut => {
                    let mut zoom_factor = egui_ctx.zoom_factor();
                    zoom_factor -= 0.1;
                    egui_ctx.set_zoom_factor(zoom_factor);
                }
                UICommand::ZoomReset => {
                    egui_ctx.set_zoom_factor(1.0);
                }
                _ => {}
            }
        }
    }
}

impl ExampleApp {
    fn top_bar(&mut self, egui_ctx: &egui::Context) {
        let top_bar_style = egui_ctx.top_bar_style(false);

        egui::TopBottomPanel::top("top_bar")
            .frame(DesignTokens::top_panel_frame())
            .exact_height(top_bar_style.height)
            .show(egui_ctx, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                if !re_ui::NATIVE_WINDOW_BAR {
                    // Interact with background first, so that buttons in the top bar gets input priority
                    // (last added widget has priority for input).
                    let title_bar_response = ui.interact(
                        ui.max_rect(),
                        ui.id().with("background"),
                        egui::Sense::click(),
                    );
                    if title_bar_response.double_clicked() {
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    } else if title_bar_response.is_pointer_button_down_on() {
                        // TODO(emilk): This should probably only run on `title_bar_response.drag_started_by(PointerButton::Primary)`,
                        // see https://github.com/emilk/egui/pull/4656
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                }

                egui::menu::bar(ui, |ui| {
                    ui.set_height(top_bar_style.height);
                    ui.add_space(top_bar_style.indent);

                    ui.menu_button("File", |ui| file_menu(ui, &self.command_sender));

                    self.top_bar_ui(ui);
                });
            });
    }

    fn top_bar_ui(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // From right-to-left:

            if re_ui::CUSTOM_WINDOW_DECORATIONS {
                ui.add_space(8.0);
                ui.native_window_buttons_ui();
                ui.separator();
            } else {
                ui.add_space(16.0);
            }

            ui.medium_icon_toggle_button(
                &re_ui::icons::RIGHT_PANEL_TOGGLE,
                &mut self.show_right_panel,
            );
            ui.medium_icon_toggle_button(
                &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                &mut self.show_bottom_panel,
            );
            ui.medium_icon_toggle_button(
                &re_ui::icons::LEFT_PANEL_TOGGLE,
                &mut self.show_left_panel,
            );

            notifications::notification_toggle_button(ui, &mut self.notifications);
        });
    }
}

fn file_menu(ui: &mut egui::Ui, command_sender: &CommandSender) {
    UICommand::SaveRecording.menu_button_ui(ui, command_sender);
    UICommand::SaveRecordingSelection.menu_button_ui(ui, command_sender);
    UICommand::Open.menu_button_ui(ui, command_sender);
    UICommand::Quit.menu_button_ui(ui, command_sender);
}

fn tabs_ui(ui: &mut egui::Ui, tree: &mut egui_tiles::Tree<Tab>) {
    tree.ui(&mut MyTileTreeBehavior {}, ui);
}

pub type Tab = i32;

struct MyTileTreeBehavior {}

impl egui_tiles::Behavior<Tab> for MyTileTreeBehavior {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        _pane: &mut Tab,
    ) -> egui_tiles::UiResponse {
        egui::Frame::none().inner_margin(4.0).show(ui, |ui| {
            egui::warn_if_debug_build(ui);
            ui.label("Hover me for a tooltip")
                .on_hover_text("This is a tooltip");

            ui.label(
                egui::RichText::new("Welcome to the ReUi example")
                    .text_style(DesignTokens::welcome_screen_h1()),
            );

            ui.error_label("This is an example of a long error label.");
            ui.warning_label("This is an example of a long warning label.");
        });

        Default::default()
    }

    fn tab_title_for_pane(&mut self, pane: &Tab) -> egui::WidgetText {
        format!("This is tab {pane}").into()
    }

    // Styling:

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Tab>,
        _tile_id: egui_tiles::TileId,
        _tab_state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        egui::Stroke::NONE
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        re_ui::DesignTokens::title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}
