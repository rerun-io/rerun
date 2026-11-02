use itertools::Itertools as _;

use re_entity_db::InstancePath;
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::blueprint::archetypes::RawDataViewFormat;
use re_sdk_types::blueprint::components::ViewDensity;
use re_sdk_types::components::Visible;
use re_sdk_types::{ComponentIdentifier, View as _, ViewClassIdentifier};
use re_ui::{ButtonExt as _, Help, UiExt as _};
use re_viewer_context::{
    DragAndDropFeedback, DragAndDropPayload, Item, SystemCommand, SystemCommandSender as _,
    TimeControlCommand, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewContext, ViewId,
    ViewQuery, ViewSpawnHeuristics, ViewState, ViewSystemExecutionError, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::component_list_ui;
use crate::component_time_stepping::{
    latest_time_for_components, next_time_for_components, prev_time_for_components,
};
use crate::properties::{components_section_ui, data_rows, entity_ui};

type ViewType = re_sdk_types::blueprint::views::RawDataView;

/// View padding in points; no bottom inset for scrolling content.
const VIEW_PADDING: egui::Margin = egui::Margin {
    left: 8,
    right: 8,
    top: 12,
    bottom: 0,
};

/// How much closer to the navigation bar the rows sit than the default item spacing puts them.
const NAVIGATION_BAR_SPACING_REDUCTION: f32 = 2.0;

#[derive(Default)]
pub struct RawDataView;

impl ViewClass for RawDataView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn handle_drop(
        &self,
        ctx: &ViewContext<'_>,
        payload: &DragAndDropPayload,
        released: bool,
    ) -> DragAndDropFeedback {
        let (entity, components) = match payload {
            DragAndDropPayload::Entities { entities } => {
                let [entity] = entities.as_slice() else {
                    return DragAndDropFeedback::Reject(Some("Drop exactly one entity"));
                };
                (entity, None)
            }
            DragAndDropPayload::Components { component_paths } => {
                let first = component_paths.first();
                if component_paths
                    .iter()
                    .any(|path| path.entity_path != first.entity_path)
                {
                    return DragAndDropFeedback::Reject(Some(
                        "Drop components from a single entity",
                    ));
                }
                let components = component_paths
                    .iter()
                    .map(|path| path.component)
                    .unique()
                    .collect::<Vec<_>>();
                (&first.entity_path, Some(components))
            }
            DragAndDropPayload::Contents { .. } | DragAndDropPayload::Invalid => {
                return DragAndDropFeedback::Ignore;
            }
        };
        if released {
            if let Some(components) = components.as_deref()
                && displayed_entity(ctx.viewer_ctx, ctx.view_id).as_ref() == Some(entity)
            {
                crate::properties::add_data_rows(ctx, components);
            } else {
                crate::properties::show_entity(ctx, entity, components.as_deref());
            }
        }
        DragAndDropFeedback::Accept
    }

    fn recommendation_order(&self) -> i32 {
        // Prefer specialized views over this catch-all.
        i32::MAX
    }

    fn display_name(&self) -> &'static str {
        "Raw data"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_GENERIC
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Raw data view")
            .docs_link("https://rerun.io/docs/reference/types/views/raw_data_view")
            .markdown(
                "The raw component data of a single entity.
You can pick the entity and the rows to show in the selection panel.

The arrow buttons step the time cursor to the next and previous event of the rows on show,
skipping events of components that are hidden.",
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<re_viewer_context::NoopVisualizer>()
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::empty()
    }

    fn selection_ui<'a>(
        &'a self,
        view_ctx: &re_viewer_context::ViewContext<'_>,
    ) -> re_viewer_context::ViewSelectionUi<'a> {
        let entity_path = displayed_entity(view_ctx.viewer_ctx, view_ctx.view_id);
        let entity_path_for_components = entity_path.clone();

        re_viewer_context::ViewSelectionUi {
            show_entity_filter: false,
            extra_sections: vec![
                re_viewer_context::SelectionSection {
                    title: "Entity".to_owned(),
                    ui: Box::new(move |ui, ctx| {
                        ui.list_item_scope("raw_data_entity", |ui| {
                            entity_ui(ctx, ui, entity_path.as_ref());
                        });
                    }),
                },
                re_viewer_context::SelectionSection {
                    title: "Components".to_owned(),
                    ui: Box::new(move |ui, ctx| {
                        ui.list_item_scope("raw_data_components", |ui| {
                            components_section_ui(ctx, ui, entity_path_for_components.as_ref());
                        });
                    }),
                },
            ],
            blueprint_properties: Some(Box::new(move |ui, ctx| {
                ui.list_item_scope("raw_data_selection_ui", |ui| {
                    re_view::view_property_ui::<RawDataViewFormat>(ctx, ui);
                });

                Ok(())
            })),
            ..Default::default()
        }
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<re_viewer_context::ViewClassUiOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let background = ui.interact(
            ui.max_rect(),
            ui.scope_id().with("raw_data_background"),
            egui::Sense::click(),
        );
        if background.clicked() {
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        let Some(entity_path) = displayed_entity(ctx, query.view_id) else {
            empty_view_ui(ui);
            return Ok(Default::default());
        };

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
        let rows = data_rows(ctx, query.view_id);
        let instance_path = InstancePath::entity_all(entity_path);

        let format = ViewProperty::from_archetype::<RawDataViewFormat>(&view_ctx);
        let show_time_navigation = **format.component_or_fallback::<Visible>(
            &view_ctx,
            RawDataViewFormat::descriptor_show_time_navigation().component,
        )?;
        let style = format.component_or_fallback::<ViewDensity>(
            &view_ctx,
            RawDataViewFormat::descriptor_style().component,
        )?;

        egui::Frame {
            inner_margin: VIEW_PADDING,
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            // Truncate to the tile width; the user can widen it with the splitters.
            ui.resizable_container_scope(|ui| {
                if show_time_navigation {
                    navigation_ui(ctx, ui, &instance_path.entity_path, rows.as_deref());

                    ui.add_space(-NAVIGATION_BAR_SPACING_REDUCTION);
                }

                egui::ScrollArea::both().show(ui, |ui| {
                    component_list_ui::component_list_ui(
                        &ctx.active_recording_store_view_context(),
                        ui,
                        &instance_path,
                        rows.as_deref(),
                        style,
                    );
                });
            });
        });

        Ok(Default::default())
    }
}

/// The first matching entity with data, ordered by path for deterministic selection.
fn displayed_entity(ctx: &ViewerContext<'_>, view_id: ViewId) -> Option<EntityPath> {
    let engine = ctx.recording_engine();

    ctx.lookup_query_result(view_id)
        .tree
        .iter_data_results()
        // Ancestors included only to build the tree are not filter matches.
        .filter(|data_result| !data_result.tree_prefix_only)
        .map(|data_result| &data_result.entity_path)
        // Broad filters also match branches with no data of their own.
        .filter(|entity_path| {
            engine
                .store()
                .all_components_on_timeline(Some(ctx.time_ctrl.timeline_name()), entity_path)
                .is_some_and(|components| !components.is_empty())
        })
        .min()
        .cloned()
}

fn empty_view_ui(ui: &mut egui::Ui) {
    egui::Frame {
        inner_margin: ui.tokens().view_padding().into(),
        ..egui::Frame::default()
    }
    .show(ui, |ui| {
        ui.label("No entity to show. Pick one with the entity path filter.");
    });
}

/// Step the time cursor between the events of the rows on show.
///
/// `rows` of `None` means the view shows every component, so every event counts.
fn navigation_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    rows: Option<&[ComponentIdentifier]>,
) {
    let timeline = *ctx.time_ctrl.timeline_name();
    let Some(current) = ctx.time_ctrl.time_int() else {
        return;
    };

    let engine = ctx.recording_engine();
    let store = engine.store();

    let components = match rows {
        Some(rows) => rows.to_vec(),
        None => store
            .all_components_on_timeline(Some(&timeline), entity_path)
            .map(|set| set.iter().copied().collect_vec())
            .unwrap_or_default(),
    };

    let event_time =
        latest_time_for_components(store, &timeline, entity_path, &components, current);
    let prev = event_time.and_then(|time| {
        prev_time_for_components(store, &timeline, entity_path, &components, time)
    });
    let next = next_time_for_components(
        store,
        &timeline,
        entity_path,
        &components,
        event_time.unwrap_or(current),
    );

    // Static data has no timeline events to navigate.
    let is_static = !components.is_empty()
        && components.iter().all(|component| {
            store.num_physical_temporal_events_for_component_on_timeline(
                &timeline,
                entity_path,
                *component,
            ) == 0
        });

    let tokens = ui.tokens();

    // Returns `target` if the button was clicked.
    let step_button = |ui: &mut egui::Ui,
                       icon: &re_ui::Icon,
                       label: &str,
                       direction: &str,
                       target: Option<TimeInt>| {
        let hover = if target.is_some() {
            format!("Jump to {direction} event")
        } else if is_static {
            "Data is static, no events to jump to".to_owned()
        } else {
            format!("No {direction} event")
        };
        let button = egui::Button::new(
            icon.as_image()
                .fit_to_exact_size(egui::Vec2::splat(8.0))
                .alt_text(label),
        )
        .icon_button()
        .ghost()
        .size(re_ui::Size::custom(16.0));
        let response = ui.add_enabled(target.is_some(), button);
        let clicked = response
            .on_hover_text(&hover)
            .on_disabled_hover_text(&hover)
            .clicked();
        target.filter(|_| clicked)
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(tokens.small_icon_size.x + tokens.text_to_icon_padding());

        let prev_clicked = step_button(
            ui,
            &re_ui::icons::ARROW_LEFT,
            "Previous event",
            "previous",
            prev,
        );

        let next_clicked = step_button(ui, &re_ui::icons::ARROW_RIGHT, "Next event", "next", next);

        ui.add_space(tokens.text_to_icon_padding());

        let time_type = ctx.recording().timeline_type(&timeline);
        let formatted_time = if let Some(time) = event_time {
            time_type.format(time, ctx.app_options().timestamp_format)
        } else if is_static {
            "Static".to_owned()
        } else {
            "No event".to_owned()
        };
        ui.label(egui::RichText::new(formatted_time).color(tokens.text_default));

        if let Some(time) = prev_clicked.or(next_clicked) {
            ctx.send_time_commands([
                TimeControlCommand::SetTime(time.into()),
                TimeControlCommand::Pause,
            ]);
        }
    });
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| RawDataView.help(ctx));
}
