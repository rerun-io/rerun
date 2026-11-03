use egui::class::HasClasses as _;
use egui::{
    Align, Atom, Button, Color32, Image, Layout, Popup, RichText, Sense, TextStyle, UiBuilder,
    include_image,
};
use emath::{Rect, RectAlign, Vec2};
use re_format::format_uint;
use re_renderer::WgpuResourcePoolStatistics;
use re_ui::ButtonExt as _;
use re_ui::{ContextExt as _, Size, UICommand, UICommandSender as _, UiExt as _, icons};
use re_viewer_context::{
    ActiveStoreContext, RecordingCommandKind, StoreHub, SystemCommand, SystemCommandSender as _,
};

use crate::AppCore;
use crate::app_blueprint::AppBlueprint;
use crate::dev_panel::{DevPanelTab, MAX_PLAUSIBLE_LATENCY_SEC, latency_text};
use crate::latency_tracker::{LatencyResult, ServerLatencyTrackers};

const NAVIGATION_BUTTON_SIZE: f32 = 24.0;

#[derive(Clone, Copy)]
enum NavigationButtonsState {
    #[cfg(target_arch = "wasm32")]
    Hidden,
    Visible {
        can_go_back: bool,
        can_go_forward: bool,
    },
}

impl NavigationButtonsState {
    fn is_visible(self) -> bool {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::Hidden => false,
            Self::Visible { .. } => true,
        }
    }
}

pub fn top_panel(
    frame: &eframe::Frame,
    app: &mut AppCore,
    app_blueprint: &AppBlueprint<'_>,
    store_context: Option<&ActiveStoreContext<'_>>,
    store_hub: &StoreHub,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
    ui: &mut egui::Ui,
) {
    re_tracing::profile_function!();

    let style_like_web = app.is_screenshotting() || app.app_env().is_test();
    let native_window_bar = !re_ui::fullsize_content(ui.os()) && !app.custom_window_decorations();
    let top_bar_style = ui.top_bar_style(frame, style_like_web);
    let window_frame = app.window_frame_config(ui.ctx());
    let mut top_panel_frame = ui.tokens().top_panel_frame(window_frame);

    if app.custom_window_decorations() {
        // Keep the custom window buttons flush with the right edge. `custom_window_frame` is false
        // on Windows, but we still draw custom caption buttons there.
        top_panel_frame.inner_margin.right = 0;
    }

    let mut content = |ui: &mut egui::Ui, show_content: bool| {
        ui.name_panel("Top bar");

        // React to dragging and double-clicking the top bar:
        #[cfg(not(target_arch = "wasm32"))]
        if !native_window_bar {
            // Interact with background first, so that buttons in the top bar gets input priority
            // (last added widget has priority for input).
            let title_bar_response = ui.interact(
                ui.max_rect(),
                ui.make_persistent_id("background"),
                egui::Sense::click_and_drag(),
            );
            if title_bar_response.double_clicked() {
                let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                ui.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            } else if title_bar_response.drag_started_by(egui::PointerButton::Primary) {
                ui.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }

        egui::MenuBar::new().ui(ui, |ui| {
            ui.set_height(top_bar_style.height);
            ui.add_space(top_bar_style.indent);

            if show_content {
                // The title bar is roomier than a list item, so everything in it is one size up.
                // The widgets inherit this, and only need to say so if they want something else.
                ui.scope_builder(
                    UiBuilder::new().with_class(Size::Small.class_name()),
                    |ui| {
                        // The buttons carry their own padding at this size, so the gap between
                        // them is already wide enough without the full item spacing on top.
                        ui.spacing_mut().item_spacing.x = 4.0;

                        top_bar_ui(
                            frame,
                            app,
                            app_blueprint,
                            store_context,
                            store_hub,
                            ui,
                            gpu_resource_stats,
                        );
                    },
                );
            }
        });
    };

    let panel = egui::Panel::top("top_bar")
        .frame(top_panel_frame)
        .exact_size(top_bar_style.height);
    let is_expanded = app_blueprint.top_panel_state().is_expanded();

    // On native MacOS, we show the close/minimize/maximize buttons in the top panel.
    // We _always_ want to show the top panel in that case, and only hide its content.
    // The web viewer has no window buttons, even when `ui.os()` is MacOS.
    let has_window_buttons = !cfg!(target_arch = "wasm32") && !native_window_bar;
    if has_window_buttons {
        panel.show(ui, |ui| content(ui, is_expanded));
    } else {
        let mut panel_expanded = is_expanded; // Note: can't resize top panel, or drag-to-close it.
        panel.show_collapsible(ui, &mut panel_expanded, |ui| content(ui, is_expanded));
    }
}

fn top_bar_ui(
    frame: &eframe::Frame,
    app: &mut AppCore,
    app_blueprint: &AppBlueprint<'_>,
    store_context: Option<&ActiveStoreContext<'_>>,
    store_hub: &StoreHub,
    ui: &mut egui::Ui,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
) {
    let top_bar_rect = ui.max_rect();

    if !app.uses_native_menu() {
        app.rerun_menu_button_ui(store_hub, store_context, ui);
    }

    let show_top_bar_diagnostics = app.app_options().show_metrics;

    if !app.is_screenshotting() && !app.app_env().is_test() {
        show_warnings(frame, ui, app.app_env(), show_top_bar_diagnostics); // Fixed width: put first

        // Latency is only meaningful for data that could have been logged just now:
        let latency_snapshot = store_context
            .filter(|store_context| {
                store_context
                    .recording
                    .data_sources()
                    .iter()
                    .any(|source| source.may_be_live())
            })
            .map(|store_context| store_context.recording.ingestion_stats().latency_snapshot());

        let mut show_latency_tab = false;
        let mut show_memory_tab = false;

        if show_top_bar_diagnostics {
            ui.separator();

            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                show_memory_tab |=
                    fixed_width_diagnostic_ui(ui, "9999.9 GiB", &TextStyle::Monospace, |ui| {
                        memory_use_label_ui(ui, gpu_resource_stats, &app.external_memory_users)
                    });
                fixed_width_diagnostic_ui(ui, "9999.9 ms", &TextStyle::Monospace, |ui| {
                    frame_time_label_ui(ui, app);
                });
                fixed_width_diagnostic_ui(ui, "9999 FPS", &TextStyle::Monospace, |ui| {
                    fps_ui(ui, app);
                });

                if let Some(latency_snapshot) = latency_snapshot {
                    show_latency_tab |=
                        fixed_width_diagnostic_ui(ui, "Latency: 59.9 s", &TextStyle::Body, |ui| {
                            latency_snapshot_button_ui(ui, &latency_snapshot)
                        });
                }
            });
        } else {
            // Show latency metrics only if high enough to be "interesting":
            if let Some(latency_snapshot) = latency_snapshot {
                // Should we show the e2e latency?

                // High enough to be concerning; low enough to be believable (and almost realtime).
                let is_latency_interesting = latency_snapshot.e2e().is_some_and(|e2e| {
                    app.app_options().warn_e2e_latency < e2e && e2e < MAX_PLAUSIBLE_LATENCY_SEC
                });

                // Avoid flicker by showing the latency for 1 second since it was last deemed interesting:
                let now = ui.input(|i| i.time);
                let latest_interest_id = egui::Id::unique("latest_latency_interest");
                let latest_interest = ui.data_mut(|d| {
                    if is_latency_interesting {
                        d.insert_temp(latest_interest_id, now);
                    }
                    d.get_temp::<f64>(latest_interest_id)
                });

                if latest_interest.is_some_and(|time| now - time < 1.0) {
                    ui.separator();
                    show_latency_tab |= latency_snapshot_button_ui(ui, &latency_snapshot);
                }
            }
        }

        if show_latency_tab {
            app.show_dev_panel_tab(DevPanelTab::Latency);
        }
        if show_memory_tab {
            app.show_dev_panel_tab(DevPanelTab::MemoryFlamegraph);
        }

        #[cfg(debug_assertions)]
        if !app.app_env().is_test() {
            multi_pass_warning_dot_ui(ui);
        }
    }

    let left_content_right = ui.cursor().left();
    let right_content_left = ui
        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.custom_window_decorations() && !cfg!(target_arch = "wasm32") {
                #[cfg(not(target_arch = "wasm32"))]
                ui.native_window_buttons_ui();
                ui.separator();
            } else {
                // Make the first button the same distance form the side as from the top,
                // no matter how high the top bar is. The button's height comes from the theme, so
                // ask it rather than assuming one.
                let extra_margin = (ui.available_height() - Size::inherited(ui).height()) / 2.0;
                ui.add_space(extra_margin);
            }

            panel_buttons_r2l(app, app_blueprint, ui, store_hub);

            if show_top_bar_diagnostics && !app.is_screenshotting() && !app.app_env().is_test() {
                connection_status_ui(
                    ui,
                    &mut app.connections.latency_trackers,
                    app.state.navigation.current(),
                    store_hub,
                );
            }

            ui.cursor().right()
        })
        .inner;

    command_palette_trigger_ui(
        ui,
        top_bar_rect,
        left_content_right,
        right_content_left,
        app,
    );
}

/// Keep rapidly changing diagnostics from moving the centered command-palette trigger.
fn fixed_width_diagnostic_ui<R>(
    ui: &mut egui::Ui,
    representative_text: &str,
    text_style: &TextStyle,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let font_id = text_style.resolve(ui.style());
    let width = ui
        .painter()
        .layout_no_wrap(representative_text.to_owned(), font_id, Color32::WHITE)
        .size()
        .x;

    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.available_height()),
        Layout::right_to_left(Align::Center),
        contents,
    )
    .inner
}

fn command_palette_trigger_ui(
    ui: &mut egui::Ui,
    top_bar_rect: Rect,
    left_content_right: f32,
    right_content_left: f32,
    app: &AppCore,
) {
    const MAX_WIDTH: f32 = 420.0;
    const MIN_WIDTH: f32 = 180.0;
    const HEIGHT: f32 = 26.0;
    const SIDE_GAP: f32 = 12.0;
    const NAVIGATION_GAP: f32 = 8.0;

    let navigation_buttons_state = navigation_buttons_state(app);
    let agent_enabled = app.app_options().experimental.agent_panel;
    let navigation_width = if navigation_buttons_state.is_visible() {
        2.0 * NAVIGATION_BUTTON_SIZE + NAVIGATION_GAP
    } else {
        0.0
    };
    let center_x = ui.ctx().content_rect().center().x;
    let left_half_width = center_x - left_content_right - SIDE_GAP - navigation_width;
    let right_half_width = right_content_left - SIDE_GAP - center_x;
    let width = (2.0 * left_half_width.min(right_half_width)).min(MAX_WIDTH);

    if width < MIN_WIDTH {
        let available_width = right_content_left - left_content_right - 2.0 * SIDE_GAP;
        if navigation_buttons_state.is_visible() && 2.0 * NAVIGATION_BUTTON_SIZE <= available_width
        {
            let forward_center_x = left_content_right + SIDE_GAP + 1.5 * NAVIGATION_BUTTON_SIZE;
            navigation_buttons_at(
                ui,
                top_bar_rect,
                forward_center_x,
                app,
                navigation_buttons_state,
            );
        }
        return;
    }

    let button_rect = Rect::from_center_size(
        egui::pos2(center_x, top_bar_rect.center().y),
        egui::vec2(width, HEIGHT),
    );
    let shortcut = UICommand::ToggleCommandPalette
        .formatted_kb_shortcut(ui.ctx())
        .unwrap_or_default();
    let subdued_color = ui.tokens().text_subdued;
    let button = Button::image_and_text(
        icons::SEARCH
            .as_image()
            .fit_to_exact_size(ui.tokens().small_icon_size)
            .tint(subdued_color),
        RichText::new(if agent_enabled {
            "Search commands or ask agent…"
        } else {
            "Search commands…"
        })
        .color(subdued_color),
    )
    .truncate()
    .right_text(RichText::new(shortcut).color(subdued_color))
    .outlined()
    // The icon is already tinted like the text; following the variant's text color would darken it.
    .image_tint_follows_text_color(false)
    .min_size(button_rect.size());

    let response = ui.place(button_rect, button);

    if response.clicked() {
        app.command_sender.send_ui(UICommand::ToggleCommandPalette);
    }

    if navigation_buttons_state.is_visible() {
        let forward_center_x = button_rect.left() - NAVIGATION_GAP - NAVIGATION_BUTTON_SIZE * 0.5;
        navigation_buttons_at(
            ui,
            top_bar_rect,
            forward_center_x,
            app,
            navigation_buttons_state,
        );
    }
}

impl AppCore {
    /// Whether navigating back and forward in history would do anything.
    pub fn navigation_availability(&self) -> (bool, bool) {
        match navigation_buttons_state(self) {
            #[cfg(target_arch = "wasm32")]
            NavigationButtonsState::Hidden => (false, false),
            NavigationButtonsState::Visible {
                can_go_back,
                can_go_forward,
            } => (can_go_back, can_go_forward),
        }
    }
}

fn navigation_buttons_state(app: &AppCore) -> NavigationButtonsState {
    #[cfg(target_arch = "wasm32")]
    if app.startup_options().web_history_enabled() {
        return crate::web_history::navigation_availability().map_or(
            NavigationButtonsState::Hidden,
            |(can_go_back, can_go_forward)| NavigationButtonsState::Visible {
                can_go_back,
                can_go_forward,
            },
        );
    }

    NavigationButtonsState::Visible {
        can_go_back: app.state.history.has_back(),
        can_go_forward: app.state.history.has_forward(),
    }
}

fn navigation_buttons_at(
    ui: &mut egui::Ui,
    top_bar_rect: Rect,
    forward_center_x: f32,
    app: &AppCore,
    state: NavigationButtonsState,
) {
    let (can_go_back, can_go_forward) = match state {
        #[cfg(target_arch = "wasm32")]
        NavigationButtonsState::Hidden => return,
        NavigationButtonsState::Visible {
            can_go_back,
            can_go_forward,
        } => (can_go_back, can_go_forward),
    };

    let forward_rect = Rect::from_center_size(
        egui::pos2(forward_center_x, top_bar_rect.center().y),
        egui::Vec2::splat(NAVIGATION_BUTTON_SIZE),
    );
    let back_rect = forward_rect.translate(egui::vec2(-forward_rect.width(), 0.0));
    let mut overlay_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(top_bar_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );

    navigation_button(
        &mut overlay_ui,
        app,
        can_go_back,
        &icons::ARROW_LEFT,
        "go back",
        UICommand::NavigateBackInHistory,
        back_rect,
    );
    navigation_button(
        &mut overlay_ui,
        app,
        can_go_forward,
        &icons::ARROW_RIGHT,
        "go forward",
        UICommand::NavigateForwardInHistory,
        forward_rect,
    );
}

fn navigation_button(
    ui: &mut egui::Ui,
    app: &AppCore,
    enabled: bool,
    icon: &'static icons::Icon,
    alt_text: &'static str,
    command: UICommand,
    rect: Rect,
) {
    // The buttons are placed edge to edge in `rect`, so they must not grow to the top bar's size.
    let button = ui
        .small_icon_button_widget(icon, alt_text)
        .size(Size::custom(rect.height()));
    let response = ui
        .add_enabled_ui(enabled, |ui| ui.place(rect, button))
        .inner;

    if response.on_hover_ui(|ui| command.tooltip_ui(ui)).clicked() {
        app.command_sender.send_ui(command);
    }
}

fn show_warnings(
    frame: &eframe::Frame,
    ui: &mut egui::Ui,
    app_env: &crate::AppEnvironment,
    show_top_bar_diagnostics: bool,
) {
    // We could log these as warning instead and relying on the notification panel to show it.
    // However, there are a few benefits of instead showing it like this:
    // * it's more visible
    // * it will be captured in screenshots in bug reports etc
    // * it let's us customize the message a bit more, with links etc.

    // We want to add a separator if there is any warning. This works.
    let mut has_shown_warning = false;

    fn show_warning(
        ui: &mut egui::Ui,
        has_shown_warning: &mut bool,
        callback: impl FnOnce(&mut egui::Ui),
    ) {
        if !*has_shown_warning {
            #[cfg(target_arch = "wasm32")]
            ui.separator();
            *has_shown_warning = true;
        }

        callback(ui);
    }

    if cfg!(debug_assertions) && show_top_bar_diagnostics {
        show_warning(ui, &mut has_shown_warning, |ui| {
            // Warn if in debug build
            ui.label(
                egui::RichText::new("⚠ Debug build")
                    .small()
                    .color(ui.visuals().warn_fg_color),
            )
            .on_hover_text("Rerun was compiled with debug assertions enabled.");
        });
    }

    if !app_env.is_test()
        && let Some(wgpu) = frame.wgpu_render_state()
        && let info = wgpu.adapter.get_info()
        && info.device_type == wgpu::DeviceType::Cpu
    {
        show_warning(ui, &mut has_shown_warning, |ui| {
            software_rasterizer_warning_ui(ui, &info);
        });
    }

    if crate::docker_detection::is_docker() {
        show_warning(ui, &mut has_shown_warning, |ui| {
            let text = egui::RichText::new("⚠ Docker")
                .small()
                .color(ui.visuals().warn_fg_color);
            let url = "https://github.com/rerun-io/rerun/issues/6835";
            ui.hyperlink_to(text,url).on_hover_ui(|ui| {
                ui.label("It looks like the Rerun Viewer is running inside a Docker container. This is not officially supported, and may lead to subtle bugs. ");
                ui.label("Click for more info.");
            });
        });
    }
}

fn software_rasterizer_warning_ui(ui: &mut egui::Ui, info: &wgpu::AdapterInfo) {
    ui.hyperlink_to(
        egui::RichText::new("⚠ Software rasterizer")
            .small()
            .color(ui.visuals().warn_fg_color),
        "https://www.rerun.io/docs/overview/installing-rerun/troubleshooting#graphics-issues",
    )
    .on_hover_ui(|ui| {
        ui.label("Software rasterizer detected - expect poor performance.");
        ui.label("Rerun requires hardware accelerated graphics (i.e. a GPU) for good performance.");
        ui.label("Click for troubleshooting.");
        ui.add_space(8.0);
        ui.label(format!(
            "wgpu adapter {}",
            re_renderer::adapter_info_summary(info)
        ));
    });
}

/// Show an orange dot to warn about multi-pass layout in egui.
///
/// If it is shown, it means something called `egui::Context::request_discard` the previous pass,
/// causing a multi-pass layout frame in egui.
/// This is used to cover up some visual glitches, but it is also
/// a bit costly and we shouldn't do it too often.
///
/// An infrequent blinking of the dot (e.g. when opening a new panel) is expected,
/// but it should not be sustained.
#[cfg(debug_assertions)]
fn multi_pass_warning_dot_ui(ui: &mut egui::Ui) {
    let is_multi_pass = 0 < ui.current_pass_index();

    // Showing the dot just one frame is not enough (e.g. easily missed at 120Hz),
    // so we blink it up and then fade it out quickly.

    let now = ui.input(|i| i.time);
    let last_multipass_time_id = ui.make_persistent_id("last_multipass_time");
    let last_multipass_time = ui.data_mut(|data| {
        let last_multipass_time =
            data.get_temp_mut_or_insert_with(last_multipass_time_id, || f64::NEG_INFINITY);
        if is_multi_pass {
            *last_multipass_time = now;
        }
        *last_multipass_time
    });
    let time_since_last_multipass = (now - last_multipass_time) as f32;

    let intensity = egui::remap_clamp(time_since_last_multipass, 0.0..=0.5, 1.0..=0.0);

    let radius = 5.0 * egui::emath::ease_in_ease_out(intensity);

    let (response, painter) = ui.allocate_painter(egui::Vec2::splat(12.0), egui::Sense::hover());

    if intensity <= 0.0 {
        // Nothing to show, but we still allocate space so we can show a tooltip to developers
        // who wondered what the hell that blinking orange dot was
    } else {
        // Paint dot:
        painter.circle_filled(response.rect.center(), radius, egui::Color32::ORANGE);

        // Make sure we ask for a repaint so we can animate the dot fading out:
        ui.request_repaint();
    }

    response.on_hover_text(
        "A blinking orange dot appears here in debug builds whenever request_discard is called.\n\
        It is expect that the dot appears occasionally, e.g. when showing a new panel for the first time.\n\
        However, it should not be sustained, as that would indicate a performance bug.",
    );
}

fn connection_status_ui(
    ui: &mut egui::Ui,
    latency_trackers: &mut ServerLatencyTrackers,
    route: &re_viewer_context::Route,
    store_hub: &StoreHub,
) {
    if let Some(origin) = route.redap_origin(store_hub)
        && origin != *re_redap_browser::EXAMPLES_ORIGIN
    {
        let latency = latency_trackers.origin_latency(&origin);

        let url = origin.format_host();
        match latency {
            LatencyResult::ToBeAssigned => {}
            LatencyResult::NoConnection => {
                ui.label(format!("no connection to {url}"));
            }
            LatencyResult::MostRecent(duration) => {
                let mut layout_job = egui::text::LayoutJob::default();

                let ms = duration.as_millis();

                RichText::new(format!("{} ms", re_format::format_uint(ms)))
                    .strong()
                    .append_to(
                        &mut layout_job,
                        ui.style(),
                        egui::FontSelection::Default,
                        egui::Align::Center,
                    );

                RichText::new(format!(" latency for {url}")).append_to(
                    &mut layout_job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );

                ui.label(layout_job);
            }
        }
    }
}

/// Lay out the panel button right-to-left
fn panel_buttons_r2l(
    app: &mut AppCore,
    app_blueprint: &AppBlueprint<'_>,
    ui: &mut egui::Ui,
    store_hub: &StoreHub,
) {
    // Square icon buttons already leave room around their icons, so the toggles sit closer
    // together than the rest of the bar.
    let item_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 2.0;

    if app.app_options().experimental.agent_panel {
        let mut open = app.agent_panel_open();
        if ui
            .medium_icon_toggle_button(&re_ui::icons::AGENT, "Agent panel toggle", &mut open)
            .on_hover_ui(|ui| UICommand::ToggleAgentPanel.tooltip_ui(ui))
            .clicked()
        {
            app.toggle_agent_panel();
        }
    }

    let route = app.state.navigation.current();

    #[cfg(target_arch = "wasm32")]
    if app.is_fullscreen_allowed() {
        let (icon, label) = if app.is_fullscreen_mode() {
            (&re_ui::icons::MINIMIZE, "Minimize")
        } else {
            (&re_ui::icons::MAXIMIZE, "Maximize")
        };

        if ui
            .medium_icon_toggle_button(icon, label, &mut true)
            .on_hover_text("Toggle fullscreen")
            .clicked()
        {
            app.toggle_fullscreen();
        }
    }

    // selection panel
    ui.add_enabled_ui(
        route.has_selection_panel() && !app_blueprint.selection_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::RIGHT_PANEL_TOGGLE,
                    "Selection panel toggle",
                    &mut app_blueprint.selection_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| UICommand::ToggleSelectionPanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_selection_panel(&app.command_sender);
            }
        },
    );

    // time panel
    ui.add_enabled_ui(
        route.has_time_panel() && !app_blueprint.time_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                    "Time panel toggle",
                    &mut app_blueprint.time_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| RecordingCommandKind::ToggleTimePanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_time_panel(&app.command_sender);
            }
        },
    );

    // blueprint panel
    ui.add_enabled_ui(
        route.has_blueprint_panel() && !app_blueprint.blueprint_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::LEFT_PANEL_TOGGLE,
                    "Blueprint panel toggle",
                    &mut app_blueprint.blueprint_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| UICommand::ToggleBlueprintPanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_blueprint_panel(&app.command_sender);
            }
        },
    );

    app.notifications.notification_toggle_button(ui);

    ui.spacing_mut().item_spacing.x = item_spacing;

    let selection = app.state.selection_state.selected_items();
    let time_ctrl = app
        .active_recording_id()
        .and_then(|id| app.state.time_controls.get(id));
    app.state.share_modal.button_ui(
        ui,
        store_hub,
        app.state.navigation.current(),
        time_ctrl,
        selection,
    );

    if let Some(auth) = &app.state.auth_state
        && !app.is_screenshotting()
    {
        let user_icon_size = 16.0;
        let response = Button::new((
            Atom::paint(Vec2::splat(user_icon_size), |ui, args| {
                user_icon(&auth.email, args.rect, ui, user_icon_size / 2.0, 220);
            }),
            icons::DROPDOWN_ARROW
                .as_image()
                .tint(ui.visuals().text_color()),
        ))
        .atom_ui(ui);

        Popup::menu(&response.response)
            .align(RectAlign::BOTTOM_END)
            .gap(3.0)
            .show(|ui| {
                ui.horizontal(|ui| {
                    let user_icon_size = 32.0;
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::splat(user_icon_size), Sense::hover());
                    user_icon(&auth.email, rect, ui, 8.0, 255);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(&auth.email).color(ui.tokens().text_default));
                        if let Some(org_name) = &auth.org_name {
                            ui.label(RichText::new(org_name).color(ui.tokens().text_subdued));
                        }
                    })
                });

                // Avoid increasing the width of the popup, ignore this button when egui calculates the size.
                if !ui.is_sizing_pass() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new("Log out").size_small().primary())
                            .clicked()
                        {
                            app.command_sender.send_system(SystemCommand::Logout);
                        }
                    });
                }
            });
    }
}

fn user_icon(email: &str, rect: Rect, ui: &egui::Ui, corner_radius: f32, tint: u8) {
    // The color should not change based on theme, so it's fine to hard-code here
    #[expect(clippy::disallowed_methods)]
    let text_color = Color32::from_gray(tint);
    Image::new(include_image!("../../data/user_image.jpg"))
        .corner_radius(corner_radius)
        .tint(text_color)
        .paint_at(ui, rect);
    let initial = email.chars().next().unwrap_or('?').to_ascii_uppercase();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(rect.height() * 0.6),
        Color32::WHITE,
    );
}

fn frame_time_label_ui(ui: &mut egui::Ui, app: &AppCore) {
    if let Some(frame_time) = app.frame_time_history.average() {
        let ms = frame_time * 1e3;

        let visuals = ui.visuals();
        let color = if ms < 15.0 {
            visuals.weak_text_color()
        } else {
            visuals.warn_fg_color
        };

        let text = format!("{ms:.1} ms");
        ui.label(egui::RichText::new(text).monospace().color(color))
            .on_hover_text("CPU time used by Rerun Viewer each frame. Lower is better.");
    }
}

fn fps_ui(ui: &mut egui::Ui, app: &AppCore) {
    if let Some(fps) = app.frame_time_history.rate() {
        let visuals = ui.visuals();

        // We only warn if we _suspect_ that we're in "continuous repaint mode".
        let low_fps_right_now = fps < 20.0 && ui.has_requested_repaint();

        let now = ui.input(|i| i.time);
        let warn_start_id = ui.make_persistent_id("fps_warning");
        let warn_start_time = ui.data_mut(|d| {
            if low_fps_right_now {
                *d.get_persisted_mut_or::<f64>(warn_start_id, now)
            } else {
                d.remove::<f64>(warn_start_id);
                now
            }
        });

        // Avoid blinking warning
        let low_fps_for_some_time = 0.5 < (now - warn_start_time);

        let color = if low_fps_for_some_time {
            visuals.warn_fg_color
        } else {
            visuals.weak_text_color()
        };

        let text = format!("{fps:.0} FPS");
        ui.label(egui::RichText::new(text).monospace().color(color))
            .on_hover_text("Frames per second. Higher is better.");
    }
}

/// Shows the memory use of the viewer. Returns `true` if the user clicked it.
#[must_use]
fn memory_use_label_ui(
    ui: &mut egui::Ui,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
    external_usage: &crate::external_memory::ExternalMemoryUsers,
) -> bool {
    const CODE: &str = "use re_memory::AccountingAllocator;\n\
                        #[global_allocator]\n\
                        static GLOBAL: AccountingAllocator<std::alloc::System> =\n    \
                            AccountingAllocator::new(std::alloc::System);";

    fn click_to_copy(
        ui: &mut egui::Ui,
        text: impl Into<String>,
        add_contents_on_hover: impl FnOnce(&mut egui::Ui),
    ) {
        let text = text.into();
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new(text)
                        .monospace()
                        .color(ui.visuals().weak_text_color()),
                )
                .sense(egui::Sense::click()),
            )
            .on_hover_ui(|ui| add_contents_on_hover(ui))
            .clicked()
        {
            ui.copy_text(CODE.to_owned());
        }
    }

    let mem = re_memory::MemoryUse::capture();

    if let Some(count) = re_memory::accounting_allocator::global_allocs() {
        let bytes_used_text = re_format::format_bytes(count.size as _);

        ui.add(
            egui::Label::new(
                egui::RichText::new(&bytes_used_text)
                    .monospace()
                    .color(ui.visuals().weak_text_color()),
            )
            .sense(egui::Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_ui(|ui| {
            egui::Grid::new("memory usage hover")
                .num_columns(2)
                .show(ui, |ui| {
                    let global_mem = count.size;
                    let external_mem = external_usage.total_external_memory();
                    let viewer_mem = global_mem as u64 - external_mem;

                    ui.label("Viewer");
                    ui.monospace(re_format::format_bytes(viewer_mem as _));
                    ui.end_row();

                    if external_mem > 0 {
                        ui.label("External");
                        ui.monospace(re_format::format_bytes(external_mem as _));
                        ui.end_row();
                    }

                    ui.label("Allocations");
                    ui.monospace(format_uint(count.count));
                    ui.end_row();

                    ui.label("GPU");
                    ui.monospace(re_format::format_bytes(
                        gpu_resource_stats.total_bytes() as _
                    ));
                    ui.end_row();

                    ui.label("GPU textures");
                    ui.monospace(format_uint(gpu_resource_stats.num_textures));
                    ui.end_row();

                    ui.label("GPU buffers");
                    ui.monospace(format_uint(gpu_resource_stats.num_buffers));
                    ui.end_row();
                });

            ui.weak("Click for a full memory breakdown");
        })
        .clicked()
    } else if let Some(rss) = mem.resident {
        let bytes_used_text = re_format::format_bytes(rss as _);
        click_to_copy(ui, &bytes_used_text, |ui| {
            ui.label(format!(
                "Rerun Viewer is using {} of Resident memory (RSS),\n\
                plus {} of GPU memory in {} textures and {} buffers.",
                bytes_used_text,
                re_format::format_bytes(gpu_resource_stats.total_bytes() as _),
                format_uint(gpu_resource_stats.num_textures),
                format_uint(gpu_resource_stats.num_buffers),
            ));
            ui.label(
                "To get more accurate memory reportings, consider configuring your Rerun \n\
                 viewer to use an AccountingAllocator by adding the following to your \n\
                 code's main entrypoint:",
            );
            ui.code(CODE);
            ui.label("(click to copy to clipboard)");
        });
        false
    } else {
        click_to_copy(ui, "N/A MiB", |ui| {
            ui.label(
                "The Rerun viewer was not configured to run with an AccountingAllocator,\n\
                consider adding the following to your code's main entrypoint:",
            );
            ui.code(CODE);
            ui.label("(click to copy to clipboard)");
        });
        false
    }
}

/// Shows the e2e latency. Returns `true` if the user clicked it.
#[must_use]
fn latency_snapshot_button_ui(ui: &mut egui::Ui, latency: &re_entity_db::LatencySnapshot) -> bool {
    let Some(e2e) = latency.e2e() else {
        return false; // No e2e latency, nothing to show as a summary
    };

    if MAX_PLAUSIBLE_LATENCY_SEC < e2e {
        // Either the sender's clock is off, or this is not live data.
        return false;
    }

    let text = format!("{} latency", latency_text(ui.visuals(), e2e).text());

    ui.add(
        egui::Label::new(egui::RichText::new(text).color(ui.visuals().weak_text_color()))
            .sense(egui::Sense::click()),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .on_hover_text("End-to-end latency, from the `log` call in the SDK to ingestion in the viewer.\nClick for a breakdown.")
    .clicked()
}
