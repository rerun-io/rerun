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
    re_log::setup_native_logging();

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
                                ui,
                                || re_ui::modal::Modal::new("Modal window"),
                                |_, ui, _| ui.label("This is a modal window."),
                            );

                            // ---

                            if ui.button("Open full span modal").clicked() {
                                self.full_span_modal_handler.open();
                            }

                            self.full_span_modal_handler.ui(
                                &self.re_ui,
                                ui,
                                || re_ui::modal::Modal::new("Modal window").full_span_content(true),
                                |_, ui, _| {
                                    for idx in 0..10 {
                                        ListItem::new(&self.re_ui, format!("Item {idx}")).show(ui);
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
                // First section - no scroll area, so a single outer "panel_content" can be used.
                //

                self.re_ui.panel_content(ui, |re_ui, ui| {
                    re_ui.panel_title_bar(
                        ui,
                        "Right panel",
                        Some("This is the title of the right panel"),
                    );
                    re_ui.large_collapsing_header(ui, "Large Collapsing Header", true, |ui| {
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
                    // Drag and drop demo
                    //

                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        self.re_ui.panel_content(ui, |re_ui, ui| {
                            re_ui.panel_title_bar_with_buttons(
                                ui,
                                "Drag-and-drop demo",
                                None,
                                |ui| {
                                    ui.add(re_ui::toggle_switch(&mut self.show_hierarchical_demo));
                                    ui.label("Hierarchical:");
                                },
                            );

                            if self.show_hierarchical_demo {
                                self.hierarchical_drag_and_drop.ui(re_ui, ui);
                            } else {
                                self.drag_and_drop.ui(re_ui, ui);
                            }
                        });
                    });

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

                                    if item.show(ui).clicked() {
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
                            .show_collapsing(
                                ui,
                                "collapsing example".into(),
                                true,
                                |_re_ui, ui| {
                                    self.re_ui.list_item("Sub-item").show(ui);
                                    self.re_ui.list_item("Sub-item").show(ui);
                                    self.re_ui
                                        .list_item("Sub-item with icon")
                                        .with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                                        .show(ui);
                                    self.re_ui.list_item("Sub-item").show(ui);
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
                let _response = egui::menu::bar(ui, |ui| {
                    ui.set_height(top_bar_style.height);
                    ui.add_space(top_bar_style.indent);

                    ui.menu_button("File", |ui| file_menu(ui, &self.command_sender));

                    self.top_bar_ui(ui);
                })
                .response;

                #[cfg(not(target_arch = "wasm32"))]
                if !re_ui::NATIVE_WINDOW_BAR {
                    let title_bar_response = _response.interact(egui::Sense::click());
                    if title_bar_response.double_clicked() {
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    } else if title_bar_response.is_pointer_button_down_on() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                }
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
    UICommand::Save.menu_button_ui(ui, command_sender);
    UICommand::SaveSelection.menu_button_ui(ui, command_sender);
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
        items: Vec<(ItemId, String)>,

        /// currently selected items
        selected_items: HashSet<ItemId>,
    }

    impl Default for ExampleDragAndDrop {
        fn default() -> Self {
            Self {
                items: (0..10).map(|i| (ItemId(i), format!("Item {i}"))).collect(),
                selected_items: HashSet::new(),
            }
        }
    }

    impl ExampleDragAndDrop {
        pub fn ui(&mut self, re_ui: &crate::ReUi, ui: &mut egui::Ui) {
            let mut source_item_pos = None;
            let mut target_item_pos = None;

            for (i, (item_id, label)) in self.items.iter_mut().enumerate() {
                //
                // Draw the item
                //

                let id = egui::Id::new("drag_demo").with(*item_id);

                let response = re_ui
                    .list_item(label.as_str())
                    .selected(self.selected_items.contains(item_id))
                    .drag_id(id)
                    .show(ui);

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

                // Multi-selection dragging not (yet?) supported, so dragging resets selection to single item.
                // TODO(emilk/egui#3841): it would be nice to have response.decidedly_dragged()
                if response.dragged() {
                    // Here, we support dragging a single item at a time, so we set the selection to the dragged item
                    // if/when we're dragging it proper.
                    if ui.input(|i| i.pointer.is_decidedly_dragging()) {
                        self.selected_items.clear();
                        self.selected_items.insert(*item_id);
                    }
                }

                //
                // Detect end-of-drag situation and prepare the swap command.
                //

                // TODO(emilk/egui#3841): very tempting to use `response.dragged()` here, but it
                // doesn't work. We must introduce `response.drag_stopped()` and use
                // `response.dragged() || response.drag_stopped()` here.
                if ui.memory(|mem| mem.is_being_dragged(response.id)) {
                    source_item_pos = Some(i);
                }

                // TODO(emilk/egui#3841): this feels like a common enough pattern that is should deserve its own API.
                let anything_being_decidedly_dragged = ui
                    .memory(|mem| mem.is_anything_being_dragged())
                    && ui.input(|i| i.pointer.is_decidedly_dragging());
                if anything_being_decidedly_dragged {
                    let (top, bottom) = response.rect.split_top_bottom_at_fraction(0.5);

                    let (insert_y, target) = if ui.rect_contains_pointer(top) {
                        (Some(top.top()), Some(i))
                    } else if ui.rect_contains_pointer(bottom) {
                        (Some(bottom.bottom()), Some(i + 1))
                    } else {
                        (None, None)
                    };

                    if let Some(insert_y) = insert_y {
                        ui.painter().hline(
                            ui.cursor().x_range(),
                            insert_y,
                            (2.0, egui::Color32::WHITE),
                        );

                        // TODO(emilk/egui#3841): it would be nice to have a drag specific API for that
                        if ui.input(|i| i.pointer.any_released()) {
                            target_item_pos = target;
                        }
                    }
                }
            }

            //
            // Handle the swap command (if any)
            //

            if let (Some(source), Some(target)) = (source_item_pos, target_item_pos) {
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
        MoveDraggedItemTo(ItemId, usize),

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

        /// Move item `item_id` to `container_id`at position `pos`.
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
                    Command::MoveDraggedItemTo(parent_id, pos) => {
                        if let Some(source_id) = self.dragged_id(ui) {
                            self.move_item(source_id, parent_id, pos);
                        }
                    }
                    Command::HighlightTargetContainer(item_id) => {
                        self.target_container = Some(item_id);
                    }
                }
            }
        }

        fn dragged_id(&self, ui: &egui::Ui) -> Option<ItemId> {
            // TODO(emilk/egui#3841): `ui.memory()` should really let us get the `dragged_id` directly.
            ui.memory(|mem| {
                self.items
                    .keys()
                    .find(|item_id| mem.is_being_dragged((**item_id).into()))
                    .copied()
            })
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
                .drag_id(item_id.into())
                .drag_target(self.target_container == Some(item_id))
                .show_collapsing(ui, item_id.into(), true, |re_ui, ui| {
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

        fn leaf_ui(&self, re_ui: &crate::ReUi, ui: &mut egui::Ui, item_id: ItemId, label: &String) {
            let response = re_ui
                .list_item(label)
                .selected(self.selected(item_id))
                .drag_id(item_id.into())
                .show(ui);

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

            if response.dragged() {
                // Here, we support dragging a single item at a time, so we set the selection to the dragged item
                // if/when we're dragging it proper.
                if ui.input(|i| i.pointer.is_decidedly_dragging()) {
                    self.send_command(Command::SetSelection(item_id));
                }
            }

            //
            // handle drop
            //

            let anything_being_decidedly_dragged = ui.memory(|mem| mem.is_anything_being_dragged())
                && ui.input(|i| i.pointer.is_decidedly_dragging());

            if !anything_being_decidedly_dragged {
                // nothing to do
                return;
            }

            let Some(dragged_item_id) = self.dragged_id(ui) else {
                // this shouldn't happen
                return;
            };

            let drag_target =
                self.find_drag_target(ui, item_id, is_container, response, body_response);

            if let Some(drag_target) = drag_target {
                // We cannot allow the target location to be "inside" the dragged item, because that would amount moving
                // myself inside of me.

                if self.contains(dragged_item_id, drag_target.target_parent_id) {
                    return;
                }

                ui.painter().hline(
                    drag_target.indicator_span_x,
                    drag_target.indicator_position_y,
                    (2.0, egui::Color32::WHITE),
                );

                // TODO(emilk/egui#3841): it would be nice to have a drag specific API for that
                if ui.input(|i| i.pointer.any_released()) {
                    self.send_command(Command::MoveDraggedItemTo(
                        drag_target.target_parent_id,
                        drag_target.target_pos,
                    ));
                } else {
                    self.send_command(Command::HighlightTargetContainer(
                        drag_target.target_parent_id,
                    ));
                }
            }
        }

        /// Compute the geometry of the drag cursor and where the dragged item should be inserted.
        ///
        /// This function implements the following logic:
        /// ```text
        ///
        ///                     insert         insert last in container before me            
        ///                   before me           (if any) or insert before me
        ///                       │                             │
        ///                   ╔═══▼═════════════════════════════▼══════════════════╗
        ///                   ║      │                                             ║
        ///      leaf item    ║ ─────┴──────────────────────────────────────────── ║
        ///                   ║                                                    ║
        ///                   ╚═════════════════════▲══════════════════════════════╝
        ///                                         │
        ///                                  insert after me
        ///
        ///
        ///
        ///                     insert         insert last in container before me
        ///                   before me           (if any) or insert before me
        ///                       │                             │
        ///                   ╔═══▼═════════════════════════════▼══════════════════╗
        /// container item    ║      │                                             ║
        ///  (no/collapsed    ║ ─────┼──────────────────────────────────────────── ║
        ///          body)    ║      │                                             ║
        ///                   ╚═══▲═════════════════════════════▲══════════════════╝
        ///                       │                             │
        ///                    insert                   insert inside me
        ///                   after me                     at pos = 0
        ///
        ///
        ///
        ///                     insert         insert last in container before me
        ///                   before me           (if any) or insert before me
        ///                       │                             │
        ///                   ╔═══▼═════════════════════════════▼══════════════════╗
        /// container item    ║      │                                             ║
        ///      with body    ║ ─────┴──────────────────────────────────────────── ║
        ///                   ║                                                    ║
        ///                   ╚══▲═══╦═════════════════════════════════════════════╣ ─┐
        ///                      │   ║                                             ║  │
        ///                  insert  ║                                             ║  │
        ///               inside me  ║                                             ║  │
        ///              at pos = 0  ╠══                                         ══╣  │
        ///                          ║                same logic                   ║  │
        ///                          ║               recursively                   ║  │ body
        ///                  insert  ║               applied here                  ║  │
        ///                after me  ╠══                                         ══╣  │
        ///                      │   ║                                             ║  │
        ///                   ┌──▼── ║                                             ║  │
        ///                   │      ║                                             ║  │
        ///                   └───── ╚═════════════════════════════════════════════╝ ─┘
        ///
        /// ```
        ///
        /// **Note**: press `Alt` to visualize the drag zones while dragging.
        fn find_drag_target(
            &self,
            ui: &egui::Ui,
            item_id: ItemId,
            is_container: bool,
            response: &egui::Response,
            body_response: Option<&egui::Response>,
        ) -> Option<DropTarget> {
            let indent = ui.spacing().indent;

            // For both leaf and containers we have two drag zones on the upper half of the item.
            let (top, mut bottom) = response.rect.split_top_bottom_at_fraction(0.5);
            let (left_top, top) = top.split_left_right_at_x(top.left() + indent);

            // For the lower part of the item, the story is more complicated:
            // - for leaf item, we have a single drag zone on the entire lower half
            // - for container item, we must distinguish between the indent part and the rest, plus check some area in the
            //   body
            let mut left_bottom = egui::Rect::NOTHING;
            if is_container {
                (left_bottom, bottom) = bottom.split_left_right_at_x(bottom.left() + indent);
            }

            let mut content_left_bottom = egui::Rect::NOTHING;
            if let Some(body_response) = body_response {
                content_left_bottom = egui::Rect::from_two_pos(
                    body_response.rect.left_bottom()
                        + egui::vec2(indent, -ReUi::list_item_height() / 2.0),
                    body_response.rect.left_bottom(),
                );
            }

            // Visualize the drag zones
            if ui.input(|i| i.modifiers.alt) {
                ui.ctx()
                    .debug_painter()
                    .debug_rect(top, egui::Color32::RED, "t");
                ui.ctx()
                    .debug_painter()
                    .debug_rect(bottom, egui::Color32::GREEN, "b");

                ui.ctx().debug_painter().debug_rect(
                    left_top,
                    egui::Color32::RED.gamma_multiply(0.5),
                    "lt",
                );
                ui.ctx().debug_painter().debug_rect(
                    left_bottom,
                    egui::Color32::GREEN.gamma_multiply(0.5),
                    "lb",
                );
                ui.ctx().debug_painter().debug_rect(
                    content_left_bottom,
                    egui::Color32::YELLOW,
                    "c",
                );
            }

            let Some((parent_id, pos_in_parent)) = self.parent_and_pos(item_id) else {
                // this shouldn't happen
                return None;
            };

            if ui.rect_contains_pointer(left_top) {
                // insert before me
                Some(DropTarget::new(
                    response.rect.x_range(),
                    top.top(),
                    parent_id,
                    pos_in_parent,
                ))
            } else if ui.rect_contains_pointer(top) {
                // insert last in the previous container if any, else insert before me
                let previous_container_id = if pos_in_parent > 0 {
                    self.container(parent_id)
                        .map(|c| c[pos_in_parent - 1])
                        .filter(|id| self.container(*id).is_some())
                } else {
                    None
                };

                if let Some(previous_container_id) = previous_container_id {
                    Some(DropTarget::new(
                        (response.rect.left() + indent..=response.rect.right()).into(),
                        top.top(),
                        previous_container_id,
                        usize::MAX,
                    ))
                } else {
                    Some(DropTarget::new(
                        response.rect.x_range(),
                        top.top(),
                        parent_id,
                        pos_in_parent,
                    ))
                }
            } else if !is_container {
                if ui.rect_contains_pointer(bottom) {
                    // insert after me
                    Some(DropTarget::new(
                        response.rect.x_range(),
                        bottom.bottom(),
                        parent_id,
                        pos_in_parent + 1,
                    ))
                } else {
                    None
                }
            } else {
                let body_rect = body_response.map(|r| r.rect).filter(|r| r.width() > 0.0);
                if let Some(body_rect) = body_rect {
                    if ui.rect_contains_pointer(left_bottom) || ui.rect_contains_pointer(bottom) {
                        // insert at pos = 0 inside me
                        Some(DropTarget::new(
                            (body_rect.left() + indent..=body_rect.right()).into(),
                            left_bottom.bottom(),
                            item_id,
                            0,
                        ))
                    } else if ui.rect_contains_pointer(content_left_bottom) {
                        // insert after me in my parent
                        Some(DropTarget::new(
                            response.rect.x_range(),
                            content_left_bottom.bottom(),
                            parent_id,
                            pos_in_parent + 1,
                        ))
                    } else {
                        None
                    }
                } else if ui.rect_contains_pointer(left_bottom) {
                    // insert after me in my parent
                    Some(DropTarget::new(
                        response.rect.x_range(),
                        left_bottom.bottom(),
                        parent_id,
                        pos_in_parent + 1,
                    ))
                } else if ui.rect_contains_pointer(bottom) {
                    // insert at pos = 0 inside me
                    Some(DropTarget::new(
                        (response.rect.left() + indent..=response.rect.right()).into(),
                        bottom.bottom(),
                        item_id,
                        0,
                    ))
                } else {
                    None
                }
            }
        }
    }

    struct DropTarget {
        /// Range of X coordinates for the drag target indicator
        indicator_span_x: egui::Rangef,

        /// Y coordinate for drag target indicator
        indicator_position_y: f32,

        /// Destination container ID
        target_parent_id: ItemId,

        /// Destination position within the container
        target_pos: usize,
    }

    impl DropTarget {
        fn new(
            indicator_span_x: egui::Rangef,
            indicator_position_y: f32,
            target_parent_id: ItemId,
            target_pos: usize,
        ) -> Self {
            Self {
                indicator_span_x,
                indicator_position_y,
                target_parent_id,
                target_pos,
            }
        }
    }
}
