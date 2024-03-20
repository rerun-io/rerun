use re_ui::list_item::ListItem;
use re_ui::{toasts, CommandPalette, ReUi, UICommand, UICommandSender};

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

fn main() -> eframe::Result<()> {
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

        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        ..Default::default()
    };

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            let re_ui = re_ui::ReUi::load_and_apply(&cc.egui_ctx);
            Box::new(ExampleApp::new(re_ui))
        }),
    )
}

pub struct ExampleApp {
    re_ui: re_ui::ReUi,
    toasts: toasts::Toasts,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    tree: egui_tiles::Tree<Tab>,

    /// regular modal
    modal_handler: re_ui::modal::ModalHandler,

    /// modal with full span mode
    full_span_modal_handler: re_ui::modal::ModalHandler,

    left_panel: bool,
    right_panel: bool,
    bottom_panel: bool,

    selected_list_item: Option<usize>,

    dummy_bool: bool,

    cmd_palette: CommandPalette,

    /// Commands to run at the end of the frame.
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
    latest_cmd: String,

    show_hierarchical_demo: bool,
    drag_and_drop: drag_and_drop::ExampleDragAndDrop,
    hierarchical_drag_and_drop: hierarchical_drag_and_drop::HierarchicalDragAndDrop,
}

impl ExampleApp {
    fn new(re_ui: re_ui::ReUi) -> Self {
        let (logger, text_log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Info);
        re_log::add_boxed_logger(Box::new(logger)).unwrap();

        let tree = egui_tiles::Tree::new_tabs("my_tree", vec![1, 2, 3]);

        let (command_sender, command_receiver) = command_channel();

        Self {
            re_ui,
            toasts: Default::default(),
            text_log_rx,

            tree,
            modal_handler: Default::default(),
            full_span_modal_handler: Default::default(),

            left_panel: true,
            right_panel: true,
            bottom_panel: true,

            selected_list_item: None,

            dummy_bool: true,

            cmd_palette: CommandPalette::default(),
            command_sender,
            command_receiver,
            latest_cmd: Default::default(),

            show_hierarchical_demo: true,
            drag_and_drop: Default::default(),
            hierarchical_drag_and_drop: Default::default(),
        }
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        while let Ok(re_log::LogMsg {
            level,
            target: _,
            msg,
        }) = self.text_log_rx.try_recv()
        {
            let kind = match level {
                re_log::Level::Error => toasts::ToastKind::Error,
                re_log::Level::Warn => toasts::ToastKind::Warning,
                re_log::Level::Info => toasts::ToastKind::Info,
                re_log::Level::Debug | re_log::Level::Trace => {
                    continue; // too spammy
                }
            };

            self.toasts.add(toasts::Toast {
                kind,
                text: msg,
                options: toasts::ToastOptions::with_ttl_in_seconds(4.0),
            });
        }
    }
}

impl eframe::App for ExampleApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn update(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_text_logs_as_notifications();
        self.toasts.show(egui_ctx);

        self.top_bar(egui_ctx);

        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(self.re_ui.bottom_panel_frame())
            .show_animated(egui_ctx, self.bottom_panel, |ui| {
                ui.strong("Bottom panel");
            });

        // LEFT PANEL

        egui::SidePanel::left("left_panel")
            .default_width(500.0)
            .frame(egui::Frame {
                fill: egui_ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show_animated(egui_ctx, self.left_panel, |ui| {
                // no need to extend `ui.max_rect()` as the enclosing frame doesn't have margins
                ui.set_clip_rect(ui.max_rect());

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

                        if ui.button("Log info").clicked() {
                            re_log::info!(
                                "A lot of text on info level.\nA lot of text in fact. So \
                             much that we should ideally be auto-wrapping it at some point, much \
                             earlier than this."
                            );
                        }
                        if ui.button("Log warn").clicked() {
                            re_log::warn!(
                                "A lot of text on warn level.\nA lot of text in fact. So \
                            much that we should ideally be auto-wrapping it at some point, much \
                            earlier than this."
                            );
                        }
                        if ui.button("Log error").clicked() {
                            re_log::error!(
                                "A lot of text on error level.\nA lot of text in fact. \
                            So much that we should ideally be auto-wrapping it at some point, much \
                            earlier than this."
                            );
                        }
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

                            // ---

                            if ui.button("Open modal").clicked() {
                                self.modal_handler.open();
                            }

                            self.modal_handler.ui(
                                &self.re_ui,
                                ui.ctx(),
                                || re_ui::modal::Modal::new("Modal window"),
                                |_, ui, _| ui.label("This is a modal window."),
                            );

                            // ---

                            if ui.button("Open full span modal").clicked() {
                                self.full_span_modal_handler.open();
                            }

                            self.full_span_modal_handler.ui(
                                &self.re_ui,
                                ui.ctx(),
                                || re_ui::modal::Modal::new("Modal window").full_span_content(true),
                                |_, ui, _| {
                                    for idx in 0..10 {
                                        ListItem::new(&self.re_ui, format!("Item {idx}"))
                                            .show_flat(ui);
                                    }
                                },
                            );

                            // ---

                            self.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                                ui.label("Some data here");
                            });
                            self.re_ui
                                .large_collapsing_header(ui, "Blueprint", true, |ui| {
                                    ui.style_mut().wrap = Some(false);
                                    ui.label("Some blueprint stuff here, that might be wide.");
                                    self.re_ui.checkbox(ui, &mut self.dummy_bool, "Checkbox");

                                    self.re_ui.collapsing_header(
                                        ui,
                                        "Collapsing header",
                                        true,
                                        |ui| {
                                            ui.label("Some data here");
                                            self.re_ui.checkbox(
                                                ui,
                                                &mut self.dummy_bool,
                                                "Checkbox",
                                            );
                                        },
                                    );
                                });
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
        // clip rectangle for full-span behavior, without interference from the scroll areas.

        let panel_frame = egui::Frame {
            fill: egui_ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::SidePanel::right("right_panel")
            .frame(panel_frame)
            .show_animated(egui_ctx, self.right_panel, |ui| {
                ui.set_clip_rect(ui.max_rect());

                //
                // First section - Drag and drop demos
                //

                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    self.re_ui.panel_content(ui, |re_ui, ui| {
                        re_ui.panel_title_bar_with_buttons(ui, "Drag-and-drop demo", None, |ui| {
                            ui.add(re_ui::toggle_switch(&mut self.show_hierarchical_demo));
                            ui.label("Hierarchical:");
                        });

                        if self.show_hierarchical_demo {
                            self.hierarchical_drag_and_drop.ui(re_ui, ui);
                        } else {
                            self.drag_and_drop.ui(re_ui, ui);
                        }
                    });

                    ReUi::full_span_separator(ui);
                    ui.add_space(20.0);
                });

                //
                // Second section - no scroll area, so a single outer "panel_content" can be used.
                //

                self.re_ui.panel_content(ui, |re_ui, ui| {
                    re_ui.large_collapsing_header(ui, "Full-Span UI examples", true, |ui| {
                        ui.label("Some data here");
                        ui.label("Some data there");

                        selection_buttons(ui);
                    });
                });

                // From now on, it's only `list_items`, so we need to remove the default
                // spacing.
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    //
                    // Nested scroll area demo. Multiple `panel_content` must be used.
                    //

                    self.re_ui.panel_content(ui, |re_ui, ui| {
                        re_ui.panel_title_bar(ui, "Another section", None);
                    });

                    egui::ScrollArea::both()
                        .id_source("example_right_panel")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            self.re_ui.panel_content(ui, |re_ui, ui| {
                                for i in 0..10 {
                                    let label = if i == 4 {
                                        "That's one heck of a loooooooong label!".to_owned()
                                    } else {
                                        format!("Some item {i}")
                                    };

                                    let mut item = re_ui
                                        .list_item(label)
                                        .selected(Some(i) == self.selected_list_item)
                                        .active(i != 3)
                                        .with_buttons(|re_ui, ui| {
                                            re_ui.small_icon_button(ui, &re_ui::icons::ADD)
                                                | re_ui.small_icon_button(ui, &re_ui::icons::REMOVE)
                                        });

                                    // demo custom icon
                                    item = if i == 6 {
                                        item.with_icon_fn(|_re_ui, ui, rect, visuals| {
                                            ui.painter().circle(
                                                rect.center(),
                                                rect.width() / 2.0,
                                                visuals.fg_stroke.color,
                                                egui::Stroke::NONE,
                                            );
                                        })
                                    } else {
                                        item.with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                                    };

                                    if item.show_flat(ui).clicked() {
                                        self.selected_list_item = Some(i);
                                    }
                                }
                            });
                        });

                    //
                    // Demo of `ListItem` features.
                    //

                    self.re_ui.panel_content(ui, |re_ui, ui| {
                        re_ui.panel_title_bar(ui, "Another section", None);

                        self.re_ui
                            .list_item("Collapsing list item with icon")
                            .with_icon(&re_ui::icons::SPACE_VIEW_2D)
                            .show_hierarchical_with_content(
                                ui,
                                "collapsing example",
                                true,
                                |_re_ui, ui| {
                                    self.re_ui.list_item("Sub-item").show_hierarchical(ui);
                                    self.re_ui.list_item("Sub-item").show_hierarchical(ui);
                                    self.re_ui
                                        .list_item("Sub-item with icon")
                                        .with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                                        .show_hierarchical(ui);
                                    self.re_ui
                                        .list_item("Sub-item")
                                        .show_hierarchical_with_content(
                                            ui,
                                            "sub-collapsing",
                                            true,
                                            |_re_ui, ui| {
                                                self.re_ui
                                                    .list_item("Sub-sub-item")
                                                    .show_hierarchical(ui)
                                            },
                                        );
                                },
                            );
                    });
                });
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
        let top_bar_style = self.re_ui.top_bar_style(false);

        egui::TopBottomPanel::top("top_bar")
            .frame(self.re_ui.top_panel_frame())
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
                re_ui::native_window_buttons_ui(ui);
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

fn file_menu(ui: &mut egui::Ui, command_sender: &CommandSender) {
    UICommand::SaveRecording.menu_button_ui(ui, command_sender);
    UICommand::SaveRecordingSelection.menu_button_ui(ui, command_sender);
    UICommand::Open.menu_button_ui(ui, command_sender);
    UICommand::Quit.menu_button_ui(ui, command_sender);
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
        egui::warn_if_debug_build(ui);
        ui.label("Hover me for a tooltip")
            .on_hover_text("This is a tooltip");

        ui.label(
            egui::RichText::new("Welcome to the ReUi example")
                .text_style(ReUi::welcome_screen_h1()),
        );

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
        _active: bool,
    ) -> egui::Stroke {
        egui::Stroke::NONE
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        re_ui::ReUi::title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

// ==============================================================================
// DRAG AND DROP DEMO

mod drag_and_drop {
    use std::collections::HashSet;

    #[derive(Hash, Clone, Copy, PartialEq, Eq)]
    struct ItemId(u32);

    pub struct ExampleDragAndDrop {
        items: Vec<ItemId>,

        /// currently selected items
        selected_items: HashSet<ItemId>,
    }

    impl Default for ExampleDragAndDrop {
        fn default() -> Self {
            Self {
                items: (0..10).map(ItemId).collect(),
                selected_items: HashSet::new(),
            }
        }
    }

    impl ExampleDragAndDrop {
        pub fn ui(&mut self, re_ui: &crate::ReUi, ui: &mut egui::Ui) {
            let mut swap: Option<(usize, usize)> = None;

            for (i, item_id) in self.items.iter().enumerate() {
                //
                // Draw the item
                //

                let label = format!("Item {}", item_id.0);
                let response = re_ui
                    .list_item(label.as_str())
                    .selected(self.selected_items.contains(item_id))
                    .draggable(true)
                    .show_flat(ui);

                //
                // Handle item selection
                //

                // Basic click and cmd/ctr-click
                if response.clicked() {
                    if ui.input(|i| i.modifiers.command) {
                        if self.selected_items.contains(item_id) {
                            self.selected_items.remove(item_id);
                        } else {
                            self.selected_items.insert(*item_id);
                        }
                    } else {
                        self.selected_items.clear();
                        self.selected_items.insert(*item_id);
                    }
                }

                // Drag-and-drop of multiple items not (yet?) supported, so dragging resets selection to single item.
                if response.drag_started() {
                    self.selected_items.clear();
                    self.selected_items.insert(*item_id);

                    response.dnd_set_drag_payload(i);
                }

                //
                // Detect drag situation and run the swap if it ends.
                //

                let source_item_position_index = egui::DragAndDrop::payload(ui.ctx()).map(|i| *i);

                if let Some(source_item_position_index) = source_item_position_index {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

                    let (top, bottom) = response.rect.split_top_bottom_at_fraction(0.5);

                    let (insert_y, target) = if ui.rect_contains_pointer(top) {
                        (Some(top.top()), Some(i))
                    } else if ui.rect_contains_pointer(bottom) {
                        (Some(bottom.bottom()), Some(i + 1))
                    } else {
                        (None, None)
                    };

                    if let (Some(insert_y), Some(target)) = (insert_y, target) {
                        ui.painter().hline(
                            ui.cursor().x_range(),
                            insert_y,
                            (2.0, egui::Color32::WHITE),
                        );

                        // note: can't use `response.drag_released()` because we not the item which
                        // started the drag
                        if ui.input(|i| i.pointer.any_released()) {
                            swap = Some((source_item_position_index, target));

                            egui::DragAndDrop::clear_payload(ui.ctx());
                        }
                    }
                }
            }

            //
            // Handle the swap command (if any)
            //

            if let Some((source, target)) = swap {
                if source != target {
                    let item = self.items.remove(source);

                    if source < target {
                        self.items.insert(target - 1, item);
                    } else {
                        self.items.insert(target, item);
                    }
                }
            }
        }
    }
}

// ==============================================================================
// HIERARCHICAL DRAG AND DROP DEMO

mod hierarchical_drag_and_drop {
    use std::collections::{HashMap, HashSet};

    use egui::NumExt;

    use re_ui::ReUi;

    #[derive(Hash, Clone, Copy, PartialEq, Eq)]
    struct ItemId(u32);

    impl ItemId {
        fn new() -> Self {
            Self(rand::random())
        }
    }

    impl std::fmt::Debug for ItemId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "#{:04x}", self.0)
        }
    }

    impl From<ItemId> for egui::Id {
        fn from(id: ItemId) -> Self {
            Self::new(id)
        }
    }

    enum Item {
        Container(Vec<ItemId>),
        Leaf(String),
    }

    #[derive(Debug)]
    enum Command {
        /// Set the selection to the given item.
        SetSelection(ItemId),

        /// Toggle the selected state of the given item.
        ToggleSelected(ItemId),

        /// Move the currently dragged item to the given container and position.
        MoveItem {
            moved_item_id: ItemId,
            target_container_id: ItemId,
            target_position_index: usize,
        },

        /// Specify the currently identifed target container to be highlighted.
        HighlightTargetContainer(ItemId),
    }

    pub struct HierarchicalDragAndDrop {
        /// All items
        items: HashMap<ItemId, Item>,

        /// Id of the root item (not displayed in the UI)
        root_id: ItemId,

        /// Set of all selected items
        selected_items: HashSet<ItemId>,

        /// If a drag is ongoing, this is the id of the destination container (if any was identified)
        ///
        /// This is used to highlight the target container.
        target_container: Option<ItemId>,

        /// Channel to receive commands from the UI
        command_receiver: std::sync::mpsc::Receiver<Command>,

        /// Channel to send commands from the UI
        command_sender: std::sync::mpsc::Sender<Command>,
    }

    impl Default for HierarchicalDragAndDrop {
        fn default() -> Self {
            let root_item = Item::Container(Vec::new());
            let root_id = ItemId::new();

            let (command_sender, command_receiver) = std::sync::mpsc::channel();

            let mut res = Self {
                items: std::iter::once((root_id, root_item)).collect(),
                root_id,
                selected_items: HashSet::new(),
                target_container: None,
                command_receiver,
                command_sender,
            };

            res.populate();

            res
        }
    }

    //
    // Data stuff
    //
    impl HierarchicalDragAndDrop {
        /// Add a bunch of items in the hierarchy.
        fn populate(&mut self) {
            let c1 = self.add_container(self.root_id);
            let c2 = self.add_container(self.root_id);
            let c3 = self.add_container(self.root_id);
            self.add_leaf(self.root_id);
            self.add_leaf(self.root_id);

            let c11 = self.add_container(c1);
            let c12 = self.add_container(c1);
            self.add_leaf(c11);
            self.add_leaf(c11);
            self.add_leaf(c12);
            self.add_leaf(c12);

            self.add_leaf(c2);
            self.add_leaf(c2);

            self.add_leaf(c3);
        }

        fn container(&self, id: ItemId) -> Option<&Vec<ItemId>> {
            match self.items.get(&id) {
                Some(Item::Container(children)) => Some(children),
                _ => None,
            }
        }

        /// Does some container contain the given item?
        ///
        /// Used to test if a target location is suitable for a given dragged item.
        fn contains(&self, container_id: ItemId, item_id: ItemId) -> bool {
            if let Some(children) = self.container(container_id) {
                if container_id == item_id {
                    return true;
                }

                if children.contains(&item_id) {
                    return true;
                }

                for child_id in children {
                    if self.contains(*child_id, item_id) {
                        return true;
                    }
                }

                return false;
            }

            false
        }

        /// Move item `item_id` to `container_id` at position `pos`.
        fn move_item(&mut self, item_id: ItemId, container_id: ItemId, mut pos: usize) {
            println!("Moving {item_id:?} to {container_id:?} at position {pos:?}");

            // Remove the item from its current location. Note: we must adjust the target position if the item is
            // moved within the same container, as the removal might shift the positions by one.
            if let Some((source_parent_id, source_pos)) = self.parent_and_pos(item_id) {
                if let Some(Item::Container(children)) = self.items.get_mut(&source_parent_id) {
                    children.remove(source_pos);
                }

                if source_parent_id == container_id && source_pos < pos {
                    pos -= 1;
                }
            }

            if let Some(Item::Container(children)) = self.items.get_mut(&container_id) {
                children.insert(pos.at_most(children.len()), item_id);
            }
        }

        /// Find the parent of an item, and the index of that item within the parent's children.
        fn parent_and_pos(&self, id: ItemId) -> Option<(ItemId, usize)> {
            if id == self.root_id {
                None
            } else {
                self.parent_and_pos_impl(id, self.root_id)
            }
        }

        fn parent_and_pos_impl(&self, id: ItemId, container_id: ItemId) -> Option<(ItemId, usize)> {
            if let Some(children) = self.container(container_id) {
                for (idx, child_id) in children.iter().enumerate() {
                    if child_id == &id {
                        return Some((container_id, idx));
                    } else if self.container(*child_id).is_some() {
                        let res = self.parent_and_pos_impl(id, *child_id);
                        if res.is_some() {
                            return res;
                        }
                    }
                }
            }

            None
        }

        fn add_container(&mut self, parent_id: ItemId) -> ItemId {
            let id = ItemId::new();
            let item = Item::Container(Vec::new());

            self.items.insert(id, item);

            if let Some(Item::Container(children)) = self.items.get_mut(&parent_id) {
                children.push(id);
            }

            id
        }

        fn add_leaf(&mut self, parent_id: ItemId) {
            let id = ItemId::new();
            let item = Item::Leaf(format!("Item {id:?}"));

            self.items.insert(id, item);

            if let Some(Item::Container(children)) = self.items.get_mut(&parent_id) {
                children.push(id);
            }
        }

        fn selected(&self, id: ItemId) -> bool {
            self.selected_items.contains(&id)
        }

        fn send_command(&self, command: Command) {
            // The only way this can fail is if the receiver has been dropped.
            self.command_sender.send(command).ok();
        }
    }

    //
    // UI stuff
    //
    impl HierarchicalDragAndDrop {
        pub fn ui(&mut self, re_ui: &crate::ReUi, ui: &mut egui::Ui) {
            if let Some(top_level_items) = self.container(self.root_id) {
                self.container_children_ui(re_ui, ui, top_level_items);
            }

            // always reset the target container
            self.target_container = None;

            while let Ok(command) = self.command_receiver.try_recv() {
                //println!("Received command: {command:?}");
                match command {
                    Command::SetSelection(item_id) => {
                        self.selected_items.clear();
                        self.selected_items.insert(item_id);
                    }
                    Command::ToggleSelected(item_id) => {
                        if self.selected_items.contains(&item_id) {
                            self.selected_items.remove(&item_id);
                        } else {
                            self.selected_items.insert(item_id);
                        }
                    }
                    Command::MoveItem {
                        moved_item_id,
                        target_container_id,
                        target_position_index,
                    } => self.move_item(moved_item_id, target_container_id, target_position_index),
                    Command::HighlightTargetContainer(item_id) => {
                        self.target_container = Some(item_id);
                    }
                }
            }
        }

        fn container_ui(
            &self,
            re_ui: &crate::ReUi,
            ui: &mut egui::Ui,
            item_id: ItemId,
            children: &Vec<ItemId>,
        ) {
            let response = re_ui
                .list_item(format!("Container {item_id:?}"))
                .subdued(true)
                .selected(self.selected(item_id))
                .draggable(true)
                .drop_target_style(self.target_container == Some(item_id))
                .show_hierarchical_with_content(ui, item_id, true, |re_ui, ui| {
                    self.container_children_ui(re_ui, ui, children);
                });

            self.handle_interaction(
                ui,
                item_id,
                true,
                &response.item_response,
                response.body_response.as_ref().map(|r| &r.response),
            );
        }

        fn container_children_ui(
            &self,
            re_ui: &crate::ReUi,
            ui: &mut egui::Ui,
            children: &Vec<ItemId>,
        ) {
            for child_id in children {
                match self.items.get(child_id) {
                    Some(Item::Container(children)) => {
                        self.container_ui(re_ui, ui, *child_id, children);
                    }
                    Some(Item::Leaf(label)) => {
                        self.leaf_ui(re_ui, ui, *child_id, label);
                    }
                    None => {}
                }
            }
        }

        fn leaf_ui(&self, re_ui: &crate::ReUi, ui: &mut egui::Ui, item_id: ItemId, label: &str) {
            let response = re_ui
                .list_item(label)
                .selected(self.selected(item_id))
                .draggable(true)
                .show_hierarchical(ui);

            self.handle_interaction(ui, item_id, false, &response, None);
        }

        fn handle_interaction(
            &self,
            ui: &egui::Ui,
            item_id: ItemId,
            is_container: bool,
            response: &egui::Response,
            body_response: Option<&egui::Response>,
        ) {
            //
            // basic selection management
            //

            if response.clicked() {
                if ui.input(|i| i.modifiers.command) {
                    self.send_command(Command::ToggleSelected(item_id));
                } else {
                    self.send_command(Command::SetSelection(item_id));
                }
            }

            //
            // handle drag
            //

            if response.drag_started() {
                // Here, we support dragging a single item at a time, so we set the selection to the dragged item
                // if/when we're dragging it proper.
                self.send_command(Command::SetSelection(item_id));

                egui::DragAndDrop::set_payload(ui.ctx(), item_id);
            }

            //
            // handle drop
            //

            // find the item being dragged
            let Some(dragged_item_id) =
                egui::DragAndDrop::payload(ui.ctx()).map(|payload| (*payload))
            else {
                // nothing is being dragged, we're done here
                return;
            };

            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

            let Some((parent_id, position_index_in_parent)) = self.parent_and_pos(item_id) else {
                // this shouldn't happen
                return;
            };

            let previous_container_id = if position_index_in_parent > 0 {
                self.container(parent_id)
                    .map(|c| c[position_index_in_parent - 1])
                    .filter(|id| self.container(*id).is_some())
            } else {
                None
            };

            let item_desc = re_ui::drag_and_drop::ItemContext {
                id: item_id,
                item_kind: if is_container {
                    re_ui::drag_and_drop::ItemKind::Container {
                        parent_id,
                        position_index_in_parent,
                    }
                } else {
                    re_ui::drag_and_drop::ItemKind::Leaf {
                        parent_id,
                        position_index_in_parent,
                    }
                },
                previous_container_id,
            };

            let drop_target = re_ui::drag_and_drop::find_drop_target(
                ui,
                &item_desc,
                response.rect,
                body_response.map(|r| r.rect),
                ReUi::list_item_height(),
            );

            if let Some(drop_target) = drop_target {
                // We cannot allow the target location to be "inside" the dragged item, because that would amount moving
                // myself inside of me.

                if self.contains(dragged_item_id, drop_target.target_parent_id) {
                    return;
                }

                ui.painter().hline(
                    drop_target.indicator_span_x,
                    drop_target.indicator_position_y,
                    (2.0, egui::Color32::WHITE),
                );

                // note: can't use `response.drag_released()` because we not the item which
                // started the drag
                if ui.input(|i| i.pointer.any_released()) {
                    self.send_command(Command::MoveItem {
                        moved_item_id: dragged_item_id,
                        target_container_id: drop_target.target_parent_id,
                        target_position_index: drop_target.target_position_index,
                    });

                    egui::DragAndDrop::clear_payload(ui.ctx());
                } else {
                    self.send_command(Command::HighlightTargetContainer(
                        drop_target.target_parent_id,
                    ));
                }
            }
        }
    }
}
