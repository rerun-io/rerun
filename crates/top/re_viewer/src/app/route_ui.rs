//! The UI of the current route: the central panel, and the side panels around it.

use std::borrow::Cow;

use egui::text_edit::TextEditState;
use egui::text_selection::LabelSelectionState;
use re_log_channel::RecordingOpenBehavior;
use re_redap_browser::RedapServers;
use re_sdk_types::blueprint::components::{PanelState, PlayState};
use re_ui::{ContextExt as _, UiExt as _, WindowFrameConfig};
use re_viewer_context::viewer_url::{self, ViewerUrl};
use re_viewer_context::{
    ActiveStoreContext, AppBlueprintCtx, AppContext, ApplicationSelectionState,
    BlueprintContext as _, BlueprintUndoState, CommandSender, DragAndDropManager, Item,
    ItemCollection, Route, SelectionChange, StorageContext, StoreHub, SystemCommand,
    SystemCommandSender as _, TableReference, TableStore, TimeControlCommand, ViewerContext,
};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewportBlueprint;
use re_viewport_blueprint::ui::add_view_or_container_modal_ui;

use super::AppCore;
use crate::AppState;
use crate::app_blueprint::AppBlueprint;
use crate::store_states::StoreStates;
use crate::ui::{CloudState, LoginState, WelcomeScreenState, settings_screen_ui};
use crate::viewer_url_description::ViewerUrlDescription;

const WATERMARK: bool = false; // Nice for recording media material

impl AppCore {
    /// Shows the UI for the current route.
    ///
    /// Returns where the central panel was shown, if the route has one.
    pub(super) fn route_ui(
        &mut self,
        ui: &mut egui::Ui,
        app_blueprint: &AppBlueprint<'_>,
        render_ctx: &re_renderer::RenderContext,
        active_store_context: Option<&ActiveStoreContext<'_>>,
        storage_context: &StorageContext<'_>,
        welcome_screen_state: &WelcomeScreenState,
    ) -> Option<egui::Rect> {
        re_tracing::profile_function!();

        let custom_window_frame = self.custom_window_frame();

        let Self {
            startup,
            state,
            stores,
            view_states,
            navigation,
            selection_state,
            focused_item,
            app_caches,
            #[cfg(feature = "audio")]
            audio_output,
            #[cfg(feature = "testing")]
            test_hooks,
            table_blueprints,
            registries,
            rx_log,
            loading_errors,
            command_sender,
            modals,
            event_dispatcher,
            connections,
            async_runtime: runtime,
            ..
        } = self;

        let app_env = &startup.app_env;
        let startup_options = &startup.options;
        let reflection = registries.reflection.read();
        let reflection = &*reflection;
        let component_ui_registry = &registries.component_uis;
        let component_fallback_registry = &registries.component_fallbacks;
        let view_class_registry = &registries.view_classes;
        let event_dispatcher = event_dispatcher.as_ref();

        let egui_ctx = ui.ctx().clone();

        // Record undo points for the blueprint of the shown recording or table.
        let shown_blueprint_id = if let Some(store_context) = active_store_context {
            Some(store_context.blueprint.store_id().clone())
        } else {
            navigation
                .routes
                .current()
                .table_reference()
                .and_then(|table_ref| table_blueprints.active_id(&table_ref).cloned())
        };
        if let Some(blueprint_id) = shown_blueprint_id
            && let Some(blueprint) = storage_context.bundle.get(&blueprint_id)
        {
            stores
                .blueprint_undo_state
                .entry(blueprint_id)
                .or_default()
                .update(&egui_ctx, blueprint);
        }

        re_dataframe_ui::write_pending_upserts(&egui_ctx, &connections.registry, runtime);

        let blueprint_query = stores.blueprint_query_for_viewer(
            &state.app_options,
            active_store_context.map(|ctx| ctx.blueprint),
        );

        let viewport_ui = active_store_context.map(|store_context| {
            ViewportUi::new(ViewportBlueprint::from_db(
                store_context.blueprint,
                &blueprint_query,
            ))
        });
        let drag_and_drop_manager = if let Some(viewport_ui) = &viewport_ui {
            // The root container cannot be dragged.
            DragAndDropManager::new(Item::Container(viewport_ui.blueprint.root_container))
        } else {
            DragAndDropManager::new(ItemCollection::default())
        };

        let active_route = navigation.routes.current().clone();

        // Don't mess with the selection state while we're on the loading screen,
        // this way we can go back.
        if !matches!(active_route, Route::Loading(_)) {
            selection_on_frame_start(
                selection_state,
                &mut state.recording_panel,
                storage_context,
                event_dispatcher,
                active_store_context,
                &active_route,
                viewport_ui.as_ref(),
            );
        }

        let mut registering_files: ahash::HashMap<_, Vec<_>> = Default::default();
        for registration in connections.registrations_in_flight.snapshot().into_values() {
            registering_files
                .entry(registration.origin)
                .or_default()
                .push(registration.name);
        }

        // App-level context, available for all routes (also those without an active recording).
        let app_ctx = AppContext {
            is_test: app_env.is_test(),

            app_options: &state.app_options,
            reflection,

            egui_ctx: &egui_ctx,
            render_ctx,
            command_sender,

            connection_registry: &connections.registry,
            registering_files: &registering_files,
            last_loading_error: loading_errors.by_source(),
            storage_context,
            blueprint_undo_states: &stores.blueprint_undo_state,
            active_store_context,
            app_caches,
            audio_output: cfg_select! {
                feature = "audio" => Some(audio_output),
                _ => None,
            },

            component_ui_registry,
            view_class_registry,
            component_fallback_registry,
            route: navigation.routes.current(),
            selection_state,
            focused_item,
            drag_and_drop_manager: &drag_and_drop_manager,
            connected_receivers: rx_log,
            auth_context: connections.auth.as_ref(),
            login_enabled: startup_options.login_enabled(),
            login_signed_in_url: startup_options
                .login
                .as_ref()
                .map(|l| l.signed_in_url.as_str()),
        };

        // check state early, before the UI has a chance to close these popups
        let is_any_popup_open = egui::Popup::is_any_open(ui.ctx());

        let viewport_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            ..Default::default()
        };

        // The catalog, dataset and table pages get their own background, so they don't look like
        // more panel. The viewport keeps the panel fill.
        let page_frame = egui::Frame {
            fill: ui.tokens().page_bg_color,
            ..Default::default()
        };

        // Only the settings screen edits the app options, so we avoid cloning them every frame.
        let mut new_app_options = None;

        let mut central_panel_rect = None;

        match navigation.routes.current() {
            Route::Settings {
                return_route: previous,
            } => {
                let mut show_settings_ui = true;
                let app_options = new_app_options.insert(state.app_options.clone());
                settings_screen_ui(ui, app_options, startup_options, &mut show_settings_ui);
                if !show_settings_ui {
                    command_sender.send_system(SystemCommand::SetRoute((**previous).clone()));
                }
            }

            Route::ChunkStoreBrowser {
                store_id,
                selected_chunk,
                return_route,
            } => {
                state.datastore_ui.ui(
                    ui,
                    storage_context,
                    state.app_options.timestamp_format,
                    store_id.as_ref(),
                    selected_chunk.as_ref(),
                    return_route.as_ref(),
                    command_sender,
                );
            }

            Route::LocalRecording { recording_id } => {
                // `viewport_ui` is `Some` iff `active_store_context` is `Some`.
                let (Some(store_context), Some(viewport_ui)) = (active_store_context, viewport_ui)
                else {
                    re_log::error_once!(
                        "No active store context for route {active_route:?}. This is likely a bug in the viewer state management."
                    );
                    return None;
                };

                let StoreStates {
                    selection_histories,
                    blueprint_time_control,
                    ..
                } = stores;
                let AppState {
                    selection_panel,
                    time_panel,
                    blueprint_time_panel,
                    blueprint_tree,
                    redap_servers,
                    ..
                } = state;

                // If the blueprint is invalid, reset it.
                if viewport_ui.blueprint.is_invalid() {
                    re_log::warn!("Incompatible blueprint detected. Resetting to default.");
                    command_sender
                        .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);

                    // The blueprint isn't valid so nothing past this is going to work properly.
                    // we might as well return and it will get fixed on the next frame.

                    // TODO(jleibs): If we move viewport loading up to a context where the EntityDb is mutable
                    // we can run the clear and re-load.
                    return None;
                }

                let recording = store_context.recording;

                let visualizable_entities_per_visualizer = store_context
                    .caches
                    .visualizable_entities_for_visualizer_systems();
                let indicated_entities_per_visualizer =
                    store_context.caches.indicated_entities_per_visualizer();

                let app_blueprint_ctx = AppBlueprintCtx {
                    command_sender,
                    current_blueprint: store_context.blueprint,
                    default_blueprint: store_context.default_blueprint,
                    blueprint_query: blueprint_query.clone(),
                };
                let time_ctrl = store_context.time_ctrl;

                // Execute the queries for every `View`
                let query_results = {
                    re_tracing::profile_wait!("query_results");

                    for view in viewport_ui.blueprint.views.values() {
                        view_states.ensure_state_exists(
                            recording.store_id(),
                            view.id,
                            view.class(view_class_registry),
                        );
                    }

                    re_viewport_blueprint::query_results_for_views(
                        store_context,
                        app_ctx.app_caches,
                        viewport_ui.blueprint.views.values(),
                        view_class_registry,
                        app_blueprint_ctx.blueprint_query(),
                        view_states,
                        &visualizable_entities_per_visualizer,
                        &indicated_entities_per_visualizer,
                        app_ctx.app_options,
                    )
                };

                let ctx = ViewerContext {
                    app_ctx: app_ctx.clone(),
                    store_context,
                    visualizable_entities_per_visualizer: &visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
                    query_results: &query_results,
                    time_ctrl,
                    blueprint_time_ctrl: blueprint_time_control,
                    blueprint_query: &blueprint_query,
                };

                // enable the heuristics if we must this frame
                if store_context.should_enable_heuristics {
                    viewport_ui.blueprint.set_auto_layout(true, &ctx);
                    viewport_ui.blueprint.set_auto_views(true, &ctx);
                    egui_ctx.request_repaint();
                }

                // Update the viewport. May spawn new views and handle queued requests (like screenshots).
                viewport_ui.on_frame_start(&ctx);

                let window_frame = if custom_window_frame {
                    WindowFrameConfig::custom(ui.ctx())
                } else {
                    WindowFrameConfig::Native
                };

                let was_open = app_ctx.app_options.inspect_blueprint_timeline;
                if was_open {
                    let blueprint_db = ctx.store_context.blueprint;

                    let redo_time = stores
                        .blueprint_undo_state
                        .get(blueprint_db.store_id())
                        .and_then(BlueprintUndoState::redo_time);

                    {
                        // Copy time from undo-state to the blueprint time control struct:
                        if let Some(redo_time) = redo_time {
                            ctx.command_sender()
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: blueprint_db.store_id().clone(),
                                    time_commands: vec![
                                        TimeControlCommand::SetPlayState(PlayState::Paused),
                                        TimeControlCommand::SetTime(redo_time.into()),
                                    ],
                                });
                        } else {
                            ctx.command_sender()
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: blueprint_db.store_id().clone(),
                                    time_commands: vec![TimeControlCommand::SetPlayState(
                                        PlayState::Following,
                                    )],
                                });
                        }
                    }
                }

                // The blueprint inspection panel has no collapsed state: it is either
                // fully expanded or completely hidden. Dragging it closed hides it.
                let mut inspect_blueprint_timeline = was_open;
                blueprint_time_panel.show_panel(
                    &ctx,
                    &ctx.blueprint_store_view_ctx(),
                    &viewport_ui.blueprint,
                    ui,
                    PanelState::Expanded,
                    &mut inspect_blueprint_timeline,
                    // Give the blueprint time panel a distinct color from the normal time panel:
                    ui.tokens()
                        .bottom_panel_frame(window_frame)
                        .fill(ui.tokens().blueprint_time_panel_bg_fill),
                    // No collapsed bar: dragging the panel closed hides it entirely.
                    false,
                );
                #[cfg(debug_assertions)]
                if inspect_blueprint_timeline != was_open {
                    // The user dragged the panel closed:
                    command_sender.send_system(SystemCommand::EnableInspectBlueprintTimeline(
                        inspect_blueprint_timeline,
                    ));
                }

                let time_was_expanded = app_blueprint.time_panel_state().is_expanded();
                let mut time_expanded = time_was_expanded;
                time_panel.show_panel(
                    &ctx,
                    &ctx.active_recording_store_view_context(),
                    &viewport_ui.blueprint,
                    ui,
                    app_blueprint.time_panel_state(),
                    &mut time_expanded,
                    ui.tokens().bottom_panel_frame(window_frame),
                    true,
                );
                if time_expanded != time_was_expanded {
                    // The user dragged the resize handle past the panel's limits to collapse/expand it:
                    app_blueprint.toggle_time_panel(command_sender);
                }

                let selection_was_expanded = app_blueprint.selection_panel_state().is_expanded();
                let mut selection_expanded = selection_was_expanded;
                selection_panel.show_panel(
                    &ctx,
                    &viewport_ui.blueprint,
                    view_states,
                    ui,
                    &mut selection_expanded,
                    selection_histories.entry(recording_id.clone()).or_default(),
                );
                if selection_expanded != selection_was_expanded {
                    // The user dragged the resize handle past the panel's limits to collapse/expand it:
                    app_blueprint.toggle_selection_panel(command_sender);
                }

                // If we are here and the "default" app id is selected,
                // we should instead switch to the welcome screen.
                if ctx.store_context.application_id() == StoreHub::welcome_screen_app_id() {
                    ctx.command_sender()
                        .send_system(SystemCommand::SetRoute(Route::welcome_page()));
                }

                left_panel_ui(
                    &mut state.recording_panel,
                    blueprint_tree,
                    redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &ctx.app_ctx,
                    Some((&viewport_ui, &ctx)),
                );

                let response = egui::CentralPanel::default()
                    .frame(viewport_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Viewport");
                        viewport_ui.viewport_ui(ui, &ctx, view_states);
                    });
                central_panel_rect = Some(response.response.rect);

                add_view_or_container_modal_ui(&ctx, &viewport_ui.blueprint, ui);

                // Process deferred layout operations and apply updates back to blueprint:
                viewport_ui.save_to_blueprint_store(&ctx);

                // NOTE: `redap_servers.modals_ui` is called once for all other routes after the match.
                modals.share.ui(
                    Some(&ctx),
                    ui,
                    startup_options.web_viewer_base_url().as_ref(),
                );

                #[cfg(feature = "testing")]
                if let Some(test_hook) = test_hooks.recording.take() {
                    test_hook(&ctx);
                }
            }

            Route::Loading(log_source) => {
                left_panel_ui(
                    &mut state.recording_panel,
                    &mut state.blueprint_tree,
                    &state.redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &app_ctx,
                    None,
                );

                let source_name = if let Ok(url) = ViewerUrl::from_data_source(log_source) {
                    Cow::Owned(ViewerUrlDescription::from_url(&url).to_string())
                } else {
                    // In practice this shouldn't happen.
                    Cow::Borrowed("<unknown>")
                };

                let response = egui::CentralPanel::default()
                    .frame(viewport_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Loading screen");

                        let error = loading_errors
                            .get(log_source)
                            .map(|error| format!("Failed to load {source_name}: {error}"));
                        if ui.loading_screen(
                            "Loading data source",
                            &*source_name,
                            error.as_deref(),
                            Some("Go Back"),
                        ) {
                            command_sender.send_system(SystemCommand::ReturnFromLoading);
                        }
                    });
                central_panel_rect = Some(response.response.rect);
            }

            Route::LocalTable(table_id) => {
                left_panel_ui(
                    &mut state.recording_panel,
                    &mut state.blueprint_tree,
                    &state.redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &app_ctx,
                    None,
                );

                let response = egui::CentralPanel::default()
                    .frame(page_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Table page");

                        if let Some(store) = app_ctx.table_stores().get(table_id) {
                            re_dataframe_ui::DataFusionTableWidget::new(
                                store.session_context(),
                                TableStore::TABLE_NAME,
                                TableReference::local(table_id.clone()),
                            )
                            .title(table_id.as_str())
                            .show(
                                &app_ctx,
                                runtime,
                                ui,
                                table_blueprints,
                                view_states,
                            );
                        } else {
                            re_log::error_once!(
                                "Could not find batch store for table id {}",
                                table_id
                            );
                        }
                    });
                central_panel_rect = Some(response.response.rect);
            }

            Route::RedapServer(origin) => {
                left_panel_ui(
                    &mut state.recording_panel,
                    &mut state.blueprint_tree,
                    &state.redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &app_ctx,
                    None,
                );

                let response = egui::CentralPanel::default()
                    .frame(page_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Server page");

                        if origin == &*re_redap_browser::EXAMPLES_ORIGIN {
                            let origin = state
                                .redap_servers
                                .iter_servers()
                                .find(|s| !s.origin().is_localhost())
                                .map(|s| s.origin())
                                .cloned();

                            let email = connections.auth.as_ref().map(|auth| auth.email.clone());
                            let origin_token = origin
                                .as_ref()
                                .map(|o| state.redap_servers.is_authenticated(o))
                                .unwrap_or(false);

                            let login_state = if origin_token || email.is_some() {
                                LoginState::Auth { email }
                            } else {
                                LoginState::NoAuth
                            };

                            let login_state = CloudState {
                                login: login_state,
                                has_server: origin,
                            };
                            state.welcome_screen.ui(
                                ui,
                                &app_ctx,
                                welcome_screen_state,
                                &rx_log.sources(),
                                &login_state,
                            );
                        } else {
                            state.redap_servers.server_central_panel_ui(
                                &app_ctx,
                                ui,
                                origin,
                                table_blueprints,
                                view_states,
                            );
                        }
                    });
                central_panel_rect = Some(response.response.rect);
            }

            Route::RedapEntry {
                origin,
                entry_id,
                kind,
            } => {
                left_panel_ui(
                    &mut state.recording_panel,
                    &mut state.blueprint_tree,
                    &state.redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &app_ctx,
                    None,
                );

                let response = egui::CentralPanel::default()
                    .frame(page_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Entry page");

                        state.redap_servers.entry_ui(
                            &app_ctx,
                            ui,
                            origin,
                            *entry_id,
                            *kind,
                            table_blueprints,
                            view_states,
                        );
                    });
                central_panel_rect = Some(response.response.rect);
            }

            Route::RedapFolder {
                origin,
                path: path_prefix,
            } => {
                left_panel_ui(
                    &mut state.recording_panel,
                    &mut state.blueprint_tree,
                    &state.redap_servers,
                    view_states,
                    ui,
                    app_blueprint,
                    welcome_screen_state,
                    &app_ctx,
                    None,
                );

                let response = egui::CentralPanel::default()
                    .frame(page_frame)
                    .show(ui, |ui| {
                        ui.name_panel("Folder page");

                        state.redap_servers.folder_central_panel_ui(
                            &app_ctx,
                            ui,
                            origin,
                            path_prefix,
                        );
                    });
                central_panel_rect = Some(response.response.rect);
            }
        }

        // The `LocalRecording` arm shows the share modal itself with a `ViewerContext`.
        if !matches!(active_route, Route::LocalRecording { .. }) {
            modals
                .share
                .ui(None, ui, startup_options.web_viewer_base_url().as_ref());
        }
        state.redap_servers.modals_ui(&app_ctx, ui);
        modals.open_url.ui(ui);

        drag_and_drop_manager.payload_cursor_ui(&egui_ctx);

        #[cfg(feature = "testing")]
        if let Some(app_test_hook) = test_hooks.app.take() {
            app_test_hook(&app_ctx);
        }

        if let Some(new_app_options) = new_app_options {
            state.app_options = new_app_options;
        }

        if WATERMARK {
            ui.paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(ui.ctx(), command_sender);

        // Deselect on ESC. Must happen after all other UI code to let them capture ESC if needed.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !is_any_popup_open {
            command_sender.send_system(SystemCommand::clear_selection());
        }

        // If there's no text edit or label selected, and the user triggers a copy command, copy a description of the current selection.
        if ui
            .memory(|mem| mem.focused())
            .and_then(|id| TextEditState::load(ui.ctx(), id))
            .is_none()
            && !ui
                .ctx()
                .plugin::<LabelSelectionState>()
                .lock()
                .has_selection()
            && ui.input(|input| input.events.iter().any(|e| e == &egui::Event::Copy))
        {
            selection_state.selected_items().copy_to_clipboard(ui.ctx());
        }

        selection_state.on_frame_end();

        // Reset the focused item.
        *focused_item = None;

        central_panel_rect
    }
}

fn selection_on_frame_start(
    selection_state: &mut ApplicationSelectionState,
    recording_panel: &mut re_recording_panel::RecordingPanel,
    storage_context: &StorageContext<'_>,
    event_dispatcher: Option<&crate::event::ViewerEventDispatcher>,
    active_store_context: Option<&ActiveStoreContext<'_>>,
    route: &Route,
    viewport_ui: Option<&ViewportUi>,
) {
    let selection_change = selection_state.on_frame_start(
        |item| {
            if let Item::StoreId(store_id) = item
                && store_id.is_empty_recording()
            {
                return None;
            }

            if !item.is_compatible_with_route(route) {
                return None;
            }

            if let Some(viewport_ui) = viewport_ui {
                if viewport_ui.blueprint.is_item_valid(storage_context, item) {
                    return Some(item.clone());
                }

                // A data result whose view still exists but whose entity isn't actually
                // part of that view: fall back to selecting the entity itself rather than
                // dropping the selection entirely.
                if let Item::DataResult(data_result) = item
                    && viewport_ui.blueprint.view(&data_result.view_id).is_some()
                {
                    return Some(Item::InstancePath(data_result.instance_path.clone()));
                }

                None
            } else if item.requires_blueprint() {
                // This item is invalid without an active blueprint
                None
            } else {
                // This item doesn't depend on any blueprint, lets keep it valid
                Some(item.clone())
            }
        },
        route.item(),
    );

    if let SelectionChange::SelectionChanged(selection) = selection_change {
        recording_panel.on_selection_changed(selection);

        if let Some(event_dispatcher) = event_dispatcher
            && let Some(active_store_context) = active_store_context
            && let Some(viewport_ui) = viewport_ui
        {
            event_dispatcher.on_selection_change(
                active_store_context.recording,
                selection,
                &viewport_ui.blueprint,
            );
        }
    }
}

/// Left panel (recordings and blueprint)
///
/// Takes the individual fields rather than `&mut self` so that it can be called with an
/// [`AppContext`] (or [`ViewerContext`]) borrowed from `self` alive at the call site.
#[expect(clippy::too_many_arguments)]
fn left_panel_ui(
    recording_panel: &mut re_recording_panel::RecordingPanel,
    blueprint_tree: &mut re_blueprint_tree::BlueprintTree,
    redap_servers: &RedapServers,
    view_states: &re_viewer_context::ViewStates,
    ui: &mut egui::Ui,
    app_blueprint: &AppBlueprint<'_>,
    welcome_screen_state: &WelcomeScreenState,
    app_ctx: &AppContext<'_>,
    // Present only for `Route::LocalRecording`: the blueprint tree is shown below the
    // recordings panel and needs a full `ViewerContext`.
    viewport: Option<(&ViewportUi, &ViewerContext<'_>)>,
) {
    let route = app_ctx.route();
    let left_panel = egui::Panel::left("blueprint_panel")
        .resizable(true)
        .frame(egui::Frame {
            fill: ui.visuals().panel_fill,
            ..Default::default()
        })
        .min_size(120.0)
        .default_size(default_blueprint_panel_width(ui.content_rect().width()));

    let was_expanded = app_blueprint.blueprint_panel_state().is_expanded();
    let mut blueprint_panel_expanded = was_expanded;
    let left_panel_response =
        left_panel.show_collapsible(ui, &mut blueprint_panel_expanded, |ui: &mut egui::Ui| {
            // ListItem don't need vertical spacing so we disable it, but restore it
            // before drawing the blueprint panel.
            ui.spacing_mut().item_spacing.y = 0.0;

            match route {
                Route::LocalRecording { .. }
                | Route::LocalTable(..)
                | Route::RedapEntry { .. }
                | Route::RedapFolder { .. }
                | Route::RedapServer(..)
                | Route::Loading(..) => {
                    let resizable = viewport.is_some();
                    if resizable {
                        // Ensure Blueprint panel has at least 150px minimum height, because now it doesn't autogrow (as it does without resizing=active)
                        let blueprint_min_height = 150.0;
                        let recordings_min_height = 104.0; // Minimum for recordings panel = top panel + 1 opened recording + extra space before bluprint
                        let available_height = ui.available_height();

                        // Calculate the maximum height for recordings panel
                        // Allow full space usage minus the blueprint minimum height, so that the blueprint panel can grow below existing content
                        let max_recordings_height =
                            (available_height - blueprint_min_height).max(recordings_min_height);

                        egui::Panel::top("recording_panel")
                            .frame(egui::Frame::new())
                            .resizable(resizable)
                            .min_size(recordings_min_height)
                            .max_size(max_recordings_height)
                            .default_size(160.0_f32.max(recordings_min_height))
                            .show(ui, |ui| {
                                recording_panel.show_panel(
                                    app_ctx,
                                    ui,
                                    redap_servers,
                                    welcome_screen_state.hide_examples,
                                );
                            });
                    } else {
                        recording_panel.show_panel(
                            app_ctx,
                            ui,
                            redap_servers,
                            welcome_screen_state.hide_examples,
                        );
                    }

                    if let Some((viewport_ui, ctx)) = viewport {
                        blueprint_tree.show(ctx, &viewport_ui.blueprint, ui, view_states);
                    }
                }

                Route::ChunkStoreBrowser { .. } | Route::Settings { .. } => {} // handled above
            }
        });
    if let Some(left_panel_response) = left_panel_response {
        left_panel_response
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Pane, true, "Blueprint panel"));
    }

    // The user dragged the resize handle past the panel's limits to collapse/expand it:
    if blueprint_panel_expanded != was_expanded {
        app_blueprint.toggle_blueprint_panel(app_ctx.command_sender());
    }
}

/// The default width of the left panel.
fn default_blueprint_panel_width(screen_width: f32) -> f32 {
    (0.35 * screen_width).min(200.0).round()
}

/// Handles all kind of links that can be opened within the viewer.
///
/// Must run after any ui code, or other code that tells egui to open an url.
///
/// See [`re_ui::UiExt::re_hyperlink`] for displaying hyperlinks in the UI.
fn check_for_clicked_hyperlinks(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    egui_ctx.output_mut(|o| {
        o.commands.retain_mut(|command| {
            if let egui::OutputCommand::OpenUrl(open_url) = command {
                if let Ok(url) = viewer_url::ViewerUrl::parse_with_options(
                    &open_url.url,
                    &re_data_source::FromUriOptions {
                        accept_extensionless_http: false,
                    },
                ) {
                    url.open(
                        egui_ctx,
                        &viewer_url::OpenUrlOptions {
                            recording_open_behavior: if open_url.new_tab {
                                RecordingOpenBehavior::Open
                            } else {
                                RecordingOpenBehavior::OpenAndSelect
                            },
                            show_loader: !open_url.new_tab,
                        },
                        command_sender,
                    );

                    // We handled the URL, therefore egui shouldn't do anything anymore with it.
                    return false;
                }
                // Open all links in a new tab (https://github.com/rerun-io/rerun/issues/4105)
                open_url.new_tab = true;
            }
            true
        });
    });
}
