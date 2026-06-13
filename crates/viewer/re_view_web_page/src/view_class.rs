use re_log_types::EntityPath;
use re_sdk_types::blueprint::{
    archetypes::WebPageViewConfig,
    components::{ShowNavigationControls, WebPageUrl},
};
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _, icons};
use re_viewer_context::{
    ViewClass, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewSystemExecutionError, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::backend::WebViewBounds;
use crate::lifecycle::{WebViewLifecycle, WebViewLifecycleStatus};
use crate::url_policy::{UrlPolicyResult, validate_url};

#[derive(Default)]
struct WebPageViewState {
    lifecycle: WebViewLifecycle,
    address_bar_url: String,
    address_bar_home_url: Option<String>,
    address_bar_error: Option<String>,
    pending_navigation_command: Option<NavigationCommand>,
}

impl ViewState for WebPageViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct WebPageView;

type ViewType = re_sdk_types::blueprint::views::WebPageView;

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebPageConfig {
    url: Option<String>,
    show_navigation_controls: bool,
}

impl WebPageConfig {
    fn from_blueprint(
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<Self, re_sdk_types::DeserializationError> {
        let config = ViewProperty::from_archetype::<WebPageViewConfig>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let url = config
            .component_or_empty::<WebPageUrl>(WebPageViewConfig::descriptor_url().component)?
            .map(|url| url.0.0.as_str().to_owned());

        let show_navigation_controls = config
            .component_or_empty::<ShowNavigationControls>(
                WebPageViewConfig::descriptor_show_navigation_controls().component,
            )?
            .is_none_or(|show_navigation_controls| show_navigation_controls.0.0);

        Ok(Self {
            url,
            show_navigation_controls,
        })
    }
}

impl ViewClass for WebPageView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Web Page"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &icons::VIEW_GENERIC
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Web Page view")
            .markdown("Displays a configured webpage inline in the native viewer.")
    }

    fn on_register(
        &self,
        _system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<WebPageViewState>::default()
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::empty()
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let config = WebPageConfig::from_blueprint(ctx, query)?;
        let Some(url) = config.url else {
            ui.centered_and_justified(|ui| {
                ui.label("No URL configured");
            });
            return Ok(());
        };

        let url = match validate_url(&url) {
            UrlPolicyResult::Accepted(url) => url,
            UrlPolicyResult::Invalid => {
                ui.centered_and_justified(|ui| {
                    ui.label("Invalid URL");
                });
                return Ok(());
            }
            UrlPolicyResult::UnsupportedScheme(scheme) => {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Unsupported URL scheme: {scheme}"));
                });
                return Ok(());
            }
        };

        let state = state
            .as_any_mut()
            .downcast_mut::<WebPageViewState>()
            .ok_or(ViewSystemExecutionError::StateCastError("WebPageViewState"))?;

        if state.address_bar_home_url.as_deref() != Some(url.as_str()) {
            state.address_bar_url.clone_from(&url);
            state.address_bar_home_url = Some(url.clone());
            state.address_bar_error = None;
        }

        if config.show_navigation_controls {
            let mut navigation_command = None;
            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    navigation_command = Some(NavigationCommand::Back);
                }
                if ui.button("Forward").clicked() {
                    navigation_command = Some(NavigationCommand::Forward);
                }
                if ui.button("Reload").clicked() {
                    navigation_command = Some(NavigationCommand::Reload);
                }
                if ui.button("Home").clicked() {
                    navigation_command = Some(NavigationCommand::Home);
                }
                let label = ui.label("Address");
                let go_button_width =
                    ui.spacing().interact_size.x + ui.spacing().button_padding.x * 2.0;
                let address_width =
                    (ui.available_width() - go_button_width - ui.spacing().item_spacing.x)
                        .max(80.0);
                let response = ui
                    .add(
                        egui::TextEdit::singleline(&mut state.address_bar_url)
                            .desired_width(address_width)
                            .hint_text("Address"),
                    )
                    .labelled_by(label.id);
                let go_clicked = ui.button("Go").clicked();
                if go_clicked
                    || response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
                {
                    match validate_url(&state.address_bar_url) {
                        UrlPolicyResult::Accepted(address_bar_url) => {
                            state.address_bar_url.clone_from(&address_bar_url);
                            state.address_bar_error = None;
                            navigation_command =
                                Some(NavigationCommand::NavigateTo(address_bar_url));
                        }
                        UrlPolicyResult::Invalid => {
                            state.address_bar_error = Some("Invalid URL".to_owned());
                        }
                        UrlPolicyResult::UnsupportedScheme(scheme) => {
                            state.address_bar_error =
                                Some(format!("Unsupported URL scheme: {scheme}"));
                        }
                    }
                }
            });
            if let Some(error) = &state.address_bar_error {
                ui.error_label(error);
            }
            ui.separator();

            if matches!(navigation_command, Some(NavigationCommand::Home)) {
                state.address_bar_url.clone_from(&url);
                state.address_bar_error = None;
            }

            state.pending_navigation_command = navigation_command;
        } else {
            state.pending_navigation_command = None;
        }

        let webview_rect = ui.available_rect_before_wrap();
        let webview_bounds =
            WebViewBounds::from_egui_rect(webview_rect, ui.ctx().pixels_per_point());
        let lifecycle_status =
            state
                .lifecycle
                .ensure_webview(ctx, query.view_id, &url, webview_bounds);

        match lifecycle_status {
            WebViewLifecycleStatus::Ready => {
                state.lifecycle.update_bounds(query.view_id, webview_bounds);
                state.lifecycle.set_visible(true);
                match state.pending_navigation_command.take() {
                    Some(NavigationCommand::Back) => state.lifecycle.go_back(),
                    Some(NavigationCommand::Forward) => state.lifecycle.go_forward(),
                    Some(NavigationCommand::Reload) => state.lifecycle.reload(),
                    Some(NavigationCommand::Home) => {
                        state.lifecycle.navigate_to(&url);
                    }
                    Some(NavigationCommand::NavigateTo(address_bar_url)) => {
                        state.lifecycle.navigate_to(&address_bar_url);
                    }
                    None => {}
                }
                ui.allocate_rect(webview_rect, egui::Sense::hover());
            }
            WebViewLifecycleStatus::Unavailable => {
                ui.centered_and_justified(|ui| {
                    ui.label("Embedded webview unavailable");
                });
            }
            WebViewLifecycleStatus::CreationFailed(message) => {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label("Failed to create embedded webview");
                        ui.label(message);
                    });
                });
            }
        }

        Ok(())
    }
}

enum NavigationCommand {
    Back,
    Forward,
    Reload,
    Home,
    NavigateTo(String),
}
