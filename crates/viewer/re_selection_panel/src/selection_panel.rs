use egui::{NumExt as _, TextBuffer, WidgetInfo, WidgetType};
use egui_tiles::ContainerKind;
use re_context_menu::{SelectionUpdateBehavior, context_menu_ui_for_item};
use re_data_ui::DataUi;
use re_data_ui::item_ui::{
    self, cursor_interact_with_selectable, guess_query_and_db_for_selected_entity,
};
use re_entity_db::{EntityPath, InstancePath};
use re_log_types::{ComponentPath, EntityPathFilter, EntityPathSubs, ResolvedEntityPathFilter};
use re_sdk_types::{ComponentDescriptor, components::TransformFrameId};
use re_ui::list_item::{self, ListItemContentButtonsExt as _, PropertyContent};
use re_ui::text_edit::autocomplete_text_edit;
use re_ui::{SyntaxHighlighting as _, UiExt as _, icons};
use re_viewer_context::{
    ContainerId, Contents, DataQueryResult, DataResult, HoverHighlight, Item, PerVisualizerType,
    SystemCommand, SystemCommandSender as _, TimeControlCommand, UiLayout, ViewContext, ViewId,
    ViewStates, ViewerContext, contents_name_style, icon_for_container_kind,
};
use re_viewport_blueprint::ViewportBlueprint;
use re_viewport_blueprint::ui::show_add_view_or_container_modal;

use crate::defaults_ui::view_components_defaults_section_ui;
use crate::item_heading_no_breadcrumbs::item_title_list_item;
use crate::item_heading_with_breadcrumbs::item_heading_with_breadcrumbs;
use crate::view_entity_picker::ViewEntityPicker;
use crate::visible_time_range_ui::{
    visible_time_range_ui_for_data_result, visible_time_range_ui_for_view,
};
use crate::visualizer_ui::visualizer_ui;

// ---
fn default_selection_panel_width(screen_width: f32) -> f32 {
    (0.45 * screen_width).min(300.0).round()
}

/// The "Selection view" sidebar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SelectionPanel {
    #[serde(skip)]
    /// State for the "Add entity" modal.
    view_entity_modal: ViewEntityPicker,
}

impl SelectionPanel {
    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
        expanded: bool,
    ) {
        let screen_width = ui.ctx().content_rect().width();

        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width(default_selection_panel_width(screen_width))
            .max_width((0.65 * screen_width).round())
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.style().visuals.panel_fill,
                inner_margin: egui::Margin {
                    // TODO(emilk/egui#7749): This is a workaround to prevent flicker between
                    // the time panel resize handle and our scroll bar.
                    bottom: 4,
                    ..Default::default()
                },
                ..Default::default()
            });

        if ctx.time_ctrl.highlighted_range.is_some() {
            // Always reset the VH highlight, and let the UI re-set it if needed.
            ctx.send_time_commands([TimeControlCommand::ClearHighlightedRange]);
        }

        panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
            ui.panel_content(|ui| {
                let hover = "The selection view contains information and options about \
                    the currently selected object(s)";
                ui.panel_title_bar("Selection", Some(hover));
            });

            // move the vertical spacing between the title and the content to _inside_ the scroll
            // area
            ui.add_space(-ui.spacing().item_spacing.y);

            let r = egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(ui.spacing().item_spacing.y);
                    ui.panel_content(|ui| {
                        self.contents(ctx, viewport, view_states, ui);
                    });
                });
            r.state
        });

        // run modals (these are noop if the modals are not active)
        self.view_entity_modal.ui(ui.ctx(), ctx, viewport);
    }

    fn contents(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
    ) {
        re_tracing::profile_function!();

        let selection = ctx.selection();

        if selection.is_empty() {
            return;
        }

        let tokens = ui.tokens();

        // no gap before the first item title
        ui.add_space(-ui.spacing().item_spacing.y);

        if selection.len() == 1 {
            for item in selection.iter_items() {
                let res = list_item::list_item_scope(ui, item, |ui| {
                    item_heading_with_breadcrumbs(ctx, viewport, ui, item);

                    self.item_ui(
                        ctx,
                        viewport,
                        view_states,
                        ui,
                        item,
                        UiLayout::SelectionPanel,
                    );
                });
                res.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Panel, true, "_selection_panel")
                });
            }
        } else {
            let response = list_item::list_item_scope(ui, "selections_panel", |ui| {
                ui.list_item()
                    .with_height(tokens.title_bar_height())
                    .interactive(false)
                    .selected(true)
                    .show_flat(
                        ui,
                        list_item::LabelContent::new(format!(
                            "{} selected items",
                            re_format::format_uint(selection.len())
                        )),
                    );

                for item in selection.iter_items() {
                    ui.add_space(4.0);
                    item_title_list_item(ctx, viewport, ui, item);
                }
            });
            response.response.widget_info(|| {
                WidgetInfo::labeled(egui::WidgetType::Panel, true, "_selection_panel")
            });
        }
    }

    // TODO(emilk): this should probably be `impl DataUi for Item`
    fn item_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
        item: &Item,
        ui_layout: UiLayout,
    ) {
        re_tracing::profile_function!();

        match item {
            Item::ComponentPath(component_path) => {
                let ComponentPath {
                    entity_path,
                    component,
                } = component_path;

                let (query, db) = guess_query_and_db_for_selected_entity(ctx, entity_path);
                let engine = db.storage_engine();
                let component_descriptor = engine
                    .store()
                    .entity_component_descriptor(entity_path, *component)
                    .unwrap_or_else(|| ComponentDescriptor::partial(*component));

                let is_static = engine
                    .store()
                    .entity_has_static_component(entity_path, component_descriptor.component);

                ui.list_item_flat_noninteractive(PropertyContent::new("Parent entity").value_fn(
                    |ui, _| {
                        item_ui::entity_path_parts_buttons(ctx, &query, db, ui, None, entity_path);
                    },
                ));

                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Index type").value_text(if is_static {
                        "Static"
                    } else {
                        "Temporal"
                    }),
                );

                let ComponentDescriptor {
                    archetype: archetype_name,
                    component,
                    component_type,
                } = component_descriptor;

                if let Some(archetype_name) = archetype_name {
                    ui.list_item_flat_noninteractive(PropertyContent::new("Archetype").value_fn(
                        |ui, _| {
                            ui.label(archetype_name.short_name()).on_hover_ui(|ui| {
                                ui.spacing_mut().item_spacing.y = 12.0;

                                ui.strong(archetype_name.full_name());

                                if let Some(doc_url) = archetype_name.doc_url() {
                                    ui.re_hyperlink("Full documentation", doc_url, true);
                                }
                            });
                        },
                    ));
                }

                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Component").value_text(component.as_str()),
                );

                if let Some(component_type) = component_type {
                    ui.list_item_flat_noninteractive(
                        PropertyContent::new("Component type").value_fn(|ui, _| {
                            ui.label(component_type.short_name()).on_hover_ui(|ui| {
                                ui.spacing_mut().item_spacing.y = 12.0;

                                ui.strong(component_type.full_name());

                                // Only show the first line of the docs:
                                if let Some(markdown) = ctx
                                    .reflection()
                                    .components
                                    .get(&component_type)
                                    .map(|info| info.docstring_md)
                                {
                                    ui.markdown_ui(markdown);
                                }

                                if let Some(doc_url) = component_type.doc_url() {
                                    ui.re_hyperlink("Full documentation", doc_url, true);
                                }
                            });
                        }),
                    );
                }

                list_existing_data_blueprints(ctx, viewport, ui, &entity_path.clone().into());
            }

            Item::InstancePath(instance_path) => {
                let (query, db) =
                    guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);

                ui.list_item_flat_noninteractive(PropertyContent::new("Entity path").value_fn(
                    |ui, _| {
                        item_ui::entity_path_parts_buttons(
                            ctx,
                            &query,
                            db,
                            ui,
                            None,
                            &instance_path.entity_path,
                        );
                    },
                ));

                if instance_path.instance.is_specific() {
                    ui.list_item_flat_noninteractive(
                        PropertyContent::new("Instance")
                            .value_text(instance_path.instance.to_string()),
                    );
                }

                list_existing_data_blueprints(ctx, viewport, ui, instance_path);
            }

            Item::Container(container_id) => {
                container_top_level_properties(ctx, viewport, ui, container_id);
                container_children(ctx, viewport, ui, container_id);
            }

            Item::View(view_id) => {
                if let Some(view) = viewport.view(view_id) {
                    view_top_level_properties(ctx, ui, view);
                }
            }

            Item::DataResult(view_id, instance_path) => {
                ui.list_item_flat_noninteractive(PropertyContent::new("Stream entity").value_fn(
                    |ui, _| {
                        let (query, db) =
                            guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);

                        item_ui::entity_path_parts_buttons(
                            ctx,
                            &query,
                            db,
                            ui,
                            None,
                            &instance_path.entity_path,
                        );
                    },
                ));

                if instance_path.instance.is_specific() {
                    ui.list_item_flat_noninteractive(PropertyContent::new("Instance").value_fn(
                        |ui, _| {
                            let response = ui.button(instance_path.instance.to_string());
                            cursor_interact_with_selectable(
                                ctx,
                                response,
                                Item::from(instance_path.clone()),
                            );
                        },
                    ));
                }

                if instance_path.is_all() {
                    let entity_path = &instance_path.entity_path;
                    let query_result = ctx.lookup_query_result(*view_id);
                    let data_result = query_result
                        .tree
                        .lookup_result_by_path(entity_path.hash())
                        .cloned();

                    if let Some(data_result) = &data_result
                        && let Some(view) = viewport.view(view_id)
                    {
                        let view_ctx = view.bundle_context_with_states(ctx, view_states);
                        visible_interactive_toggle_ui(&view_ctx, ui, query_result, data_result);

                        let is_spatial_view =
                            view.class_identifier() == "3D" || view.class_identifier() == "2D";
                        if is_spatial_view {
                            coordinate_frame_ui(ui, &view_ctx, data_result);
                        }
                    }
                }
            }

            _ => {}
        }

        let (query, db) = if let Some(entity_path) = item.entity_path() {
            guess_query_and_db_for_selected_entity(ctx, entity_path)
        } else {
            (ctx.current_query(), ctx.recording())
        };

        if let Some(data_ui_item) = data_section_ui(item) {
            ui.section_collapsing_header("Data").show(ui, |ui| {
                // TODO(#6075): Because `list_item_scope` changes it. Temporary until everything is `ListItem`.
                ui.spacing_mut().item_spacing.y = ui.ctx().style().spacing.item_spacing.y;
                data_ui_item.data_ui(ctx, ui, ui_layout, &query, db);
            });
        }

        match item {
            Item::StoreId(_) => {
                ui.section_collapsing_header("Properties").show(ui, |ui| {
                    show_recording_properties(ctx, db, &query, ui, ui_layout);
                });
            }

            Item::View(view_id) => {
                self.view_selection_ui(ctx, ui, viewport, view_id, view_states);
            }

            Item::DataResult(view_id, instance_path) => {
                if instance_path.is_all() {
                    entity_selection_ui(
                        ctx,
                        ui,
                        &instance_path.entity_path,
                        viewport,
                        view_id,
                        view_states,
                    );
                } else {
                    // NOTE: not implemented when a single instance is selected
                }
            }
            _ => {}
        }
    }

    fn view_selection_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &ViewportBlueprint,
        view_id: &ViewId,
        view_states: &mut ViewStates,
    ) {
        let markdown = r#"
# Entity path query syntax

Entity path queries are described as a list of include/exclude rules that act on paths:

```diff
+ /world/**           # add everything…
- /world/roads/**     # …but remove all roads…
+ /world/roads/main   # …but show main road
```

If there are multiple matching rules, the most specific rule wins.
If there are multiple rules of the same specificity, the last one wins.
If no rules match, the path is excluded.

The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
(`/world/**` matches both `/world` and `/world/car/driver`).
Other uses of `*` are not (yet) supported.

`EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
This means the last matching rule is also the most specific one.
For instance:

```diff
+ /world/**
- /world
- /world/car/**
+ /world/car/driver
```

The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
The last rule matching `/world` is `- /world`, so it is excluded.
The last rule matching `/world/house` is `+ /world/**`, so it is included.
    "#
        .trim();

        clone_view_button_ui(ctx, ui, viewport, *view_id);

        if let Some(view) = viewport.view(view_id) {
            ui.section_collapsing_header("Entity path filter")
                .with_action_button(
                    &re_ui::icons::EDIT,
                    "Modify the entity query using the editor",
                    || {
                        self.view_entity_modal.open(*view_id);
                    },
                )
                .with_help_markdown(markdown)
                .show(ui, |ui| {
                    // TODO(#6075): Because `list_item_scope` changes it. Temporary until everything is `ListItem`.
                    ui.spacing_mut().item_spacing.y = ui.ctx().style().spacing.item_spacing.y;

                    if let Some(new_entity_path_filter) = entity_path_filter_ui(
                        ctx,
                        ui,
                        *view_id,
                        view.contents.entity_path_filter(),
                        &view.space_origin,
                    ) {
                        let path_subs = EntityPathSubs::new_with_origin(&view.space_origin);
                        let query_filter = new_entity_path_filter.resolve_forgiving(&path_subs);
                        view.contents.set_entity_path_filter(ctx, query_filter);
                    }
                })
                .header_response
                .on_hover_text(
                    "The entity path query consists of a list of include/exclude rules \
                that determines what entities are part of this view",
                );
        }

        if let Some(view) = viewport.view(view_id) {
            let view_class = view.class(ctx.view_class_registry());
            let view_state = view_states.get_mut_or_create(view.id, view_class);

            ui.section_collapsing_header("View properties")
                .show(ui, |ui| {
                    // TODO(#6075): Because `list_item_scope` changes it. Temporary until everything is `ListItem`.
                    ui.spacing_mut().item_spacing.y = ui.ctx().style().spacing.item_spacing.y;

                    let cursor = ui.cursor();

                    if let Err(err) =
                        view_class.selection_ui(ctx, ui, view_state, &view.space_origin, view.id)
                    {
                        re_log::error_once!(
                            "Error in view selection UI (class: {}, display name: {}): {err}",
                            view.class_identifier(),
                            view_class.display_name(),
                        );
                    }

                    if cursor == ui.cursor() {
                        ui.weak("(none)");
                    }
                });

            let view_ctx = view.bundle_context_with_state(ctx, view_state);
            view_components_defaults_section_ui(&view_ctx, ui, view);

            visible_time_range_ui_for_view(ctx, ui, view, view_class, view_state);
        }
    }
}

/// Shows the active coordinate frame if it isn't the fallback frame.
///
/// This is not technically a visualizer, but it affects visualization, so we show it alongside.
fn coordinate_frame_ui(ui: &mut egui::Ui, ctx: &ViewContext<'_>, data_result: &DataResult) {
    use re_sdk_types::archetypes;
    use re_view::latest_at_with_blueprint_resolved_data;

    let component_descr = archetypes::CoordinateFrame::descriptor_frame();
    let component = component_descr.component;
    let query_result = latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &ctx.current_query(),
        data_result,
        [component],
        None, // coordinate frames aren't associated with any particular visualizer
    );

    let override_path = data_result.override_base_path();

    let result_override = query_result.overrides.get(component);
    let raw_override = result_override
        .and_then(|c| c.non_empty_component_batch_raw(component))
        .map(|(_, array)| array);

    let Some(frame_id_before) = query_result
        .get_mono::<TransformFrameId>(component)
        .map(|f| f.to_string())
        .or_else(|| {
            if raw_override.is_some() {
                Some(String::new())
            } else {
                None
            }
        })
    else {
        return;
    };

    let mut frame_id = if raw_override.is_some() {
        frame_id_before.clone()
    } else {
        String::new()
    };

    let property_content = list_item::PropertyContent::new("Coordinate frame")
        .value_fn(|ui, _| {
            // Show matching, non-entity-path-derived frame IDs as suggestions when the user edits the frame name.
            let suggestions = {
                let caches = ctx.viewer_ctx.store_context.caches;
                let transform_cache =
                    caches.entry(|c: &mut re_viewer_context::TransformDatabaseStoreCache| {
                        c.read_lock_transform_cache(ctx.viewer_ctx.recording())
                    });

                transform_cache
                    .frame_id_registry()
                    .iter_frame_ids()
                    .filter(|(_, id)| !id.is_entity_path_derived())
                    .map(|(_, id)| id.to_string())
                    .collect::<Vec<String>>()
            };
            autocomplete_text_edit(ui, &mut frame_id, &suggestions, Some(&frame_id_before));
        })
        .with_menu_button(&re_ui::icons::MORE, "More options", |ui: &mut egui::Ui| {
            crate::visualizer_ui::reset_override_button(
                ctx,
                ui,
                component_descr.clone(),
                override_path,
                &raw_override,
            );
        });

    ui.list_item_flat_noninteractive(property_content)
        .on_hover_ui(|ui| {
            ui.markdown_ui(
                "The coordinate frame this entity is associated with.

To learn more about coordinate frames, see the [Spaces & Transforms](https://rerun.io/docs/concepts/spaces-and-transforms) in the manual.",
            );
        });

    if raw_override.is_some() {
        if frame_id.is_empty() {
            ctx.clear_blueprint_component(override_path.clone(), component_descr);
        } else if frame_id_before != frame_id {
            // Save as blueprint override.
            ctx.save_blueprint_component(
                override_path.clone(),
                &component_descr,
                &TransformFrameId::new(&frame_id),
            );
        }
    } else if !frame_id.is_empty() {
        // Save as blueprint override.
        ctx.save_blueprint_component(
            override_path.clone(),
            &component_descr,
            &TransformFrameId::new(&frame_id),
        );
    }
}

fn show_recording_properties(
    ctx: &ViewerContext<'_>,
    db: &re_entity_db::EntityDb,
    query: &re_chunk::LatestAtQuery,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
) {
    re_tracing::profile_function!();

    let mut property_entities = db
        .sorted_entity_paths()
        .filter_map(|entity_path| entity_path.strip_prefix(&EntityPath::properties()))
        .collect::<Vec<_>>();
    property_entities.sort();

    if property_entities.is_empty() {
        ui.label("No properties found for this recording.");
    } else {
        list_item::list_item_scope(ui, "recording_properties", |ui| {
            for suffix_path in property_entities {
                let entity_path = EntityPath::properties() / suffix_path.clone();
                if suffix_path.is_root() {
                    // No header - this is likely `RecordingInfo`.
                } else if suffix_path.len() == 1 {
                    // Single nested - this is what we expect in the normal case.
                    ui.add_space(8.0);
                    ui.label(re_case::to_human_case(suffix_path[0].unescaped_str()))
                        .on_hover_ui(|ui| {
                            ui.label(entity_path.syntax_highlighted(ui.style()));
                        });
                } else {
                    // Deeply nested - show the full path.
                    ui.add_space(8.0);
                    ui.label(suffix_path.to_string()).on_hover_ui(|ui| {
                        ui.label(entity_path.syntax_highlighted(ui.style()));
                    });
                }
                entity_path.data_ui(ctx, ui, ui_layout, query, db);
            }
        });
    }
}

fn entity_selection_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    viewport: &ViewportBlueprint,
    view_id: &ViewId,
    view_states: &mut ViewStates,
) {
    let query_result = ctx.lookup_query_result(*view_id);
    let data_result = query_result
        .tree
        .lookup_result_by_path(entity_path.hash())
        .cloned();

    if let Some(view) = viewport.view(view_id) {
        let class = view.class(ctx.view_class_registry());
        view_states.ensure_state_exists(*view_id, class);
        let view_state = view_states
            .get(*view_id)
            .expect("State got created just now"); // Convince borrow checker we're not mutating `view_states` anymore.
        let view_ctx = view.bundle_context_with_state(ctx, view_state);

        let empty_errors = PerVisualizerType::default();
        let visualizer_errors = view_states
            .visualizer_errors(*view_id)
            .unwrap_or(&empty_errors);

        visualizer_ui(&view_ctx, view, visualizer_errors, entity_path, ui);
    }

    if let Some(data_result) = &data_result {
        visible_time_range_ui_for_data_result(ctx, ui, data_result);
    }
}

fn clone_view_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    viewport: &ViewportBlueprint,
    view_id: ViewId,
) {
    ui.list_item_flat_noninteractive(
        list_item::ButtonContent::new("Clone this view")
            .on_click(|| {
                if let Some(new_view_id) = viewport.duplicate_view(&view_id, ctx) {
                    ctx.command_sender()
                        .send_system(SystemCommand::set_selection(Item::View(new_view_id)));
                    viewport.mark_user_interaction(ctx);
                }
            })
            .hover_text("Create an exact duplicate of this view including all blueprint settings"),
    );
}

/// Returns a new filter when the editing is done, and there has been a change.
fn entity_path_filter_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    filter: &ResolvedEntityPathFilter,
    origin: &EntityPath,
) -> Option<EntityPathFilter> {
    fn syntax_highlight_entity_path_filter(
        tokens: &re_ui::DesignTokens,
        style: &egui::Style,
        mut string: &str,
    ) -> egui::text::LayoutJob {
        let font_id = egui::TextStyle::Body.resolve(style);

        let mut job = egui::text::LayoutJob::default();

        while !string.is_empty() {
            let newline = string.find('\n').unwrap_or(string.len() - 1);
            let line = &string[..=newline];
            string = &string[newline + 1..];
            let is_exclusion = line.trim_start().starts_with('-');

            let color = if is_exclusion {
                style.visuals.error_fg_color
            } else {
                tokens.info_log_text_color
            };

            let text_format = egui::TextFormat {
                font_id: font_id.clone(),
                color,
                ..Default::default()
            };

            job.append(line, 0.0, text_format);
        }

        job
    }

    fn text_layouter(
        ui: &egui::Ui,
        text: &dyn TextBuffer,
        wrap_width: f32,
    ) -> std::sync::Arc<egui::Galley> {
        let mut layout_job =
            syntax_highlight_entity_path_filter(ui.tokens(), ui.style(), text.as_str());
        layout_job.wrap.max_width = wrap_width;
        ui.fonts_mut(|f| f.layout_job(layout_job))
    }

    // We store the string we are temporarily editing in the `Ui`'s temporary data storage.
    // This is so it can contain invalid rules while the user edits it, and it's only normalized
    // when they press enter, or stops editing.
    let filter_text_id = ui.id().with("filter_text");

    let mut filter_string = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with::<String>(filter_text_id, || {
            // We hide the properties filter by default.
            filter.formatted_without_properties()
        })
        .clone()
    });

    let response = ui.add(
        egui::TextEdit::multiline(&mut filter_string)
            .desired_width(ui.spacing().text_edit_width.at_least(ui.available_width()))
            .layouter(&mut text_layouter),
    );

    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp::<String>(filter_text_id, filter_string.clone()));
    } else {
        // Reconstruct it from the filter next frame
        ui.data_mut(|data| data.remove::<String>(filter_text_id));
    }

    // Show some statistics about the query, print a warning text if something seems off.
    let query = ctx.lookup_query_result(view_id);
    if query.num_matching_entities == 0 {
        ui.warning_label("Does not match any entity");
    } else if query.num_matching_entities == 1 {
        ui.label("Matches 1 entity");
    } else {
        ui.label(format!("Matches {} entities", query.num_matching_entities));
    }
    if query.num_matching_entities != 0 && query.num_visualized_entities == 0 {
        // TODO(andreas): Talk about this root bit only if it's a spatial view.
        ui.warning_label(
            format!("This view is not able to visualize any of the matched entities using the current root \"{origin:?}\"."),
        );
    }

    // Apply the edit.
    let new_filter = EntityPathFilter::parse_forgiving(&filter_string);
    if response.changed() && new_filter != filter.unresolved() {
        Some(new_filter)
    } else {
        None // no change
    }
}

fn container_children(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    container_id: &ContainerId,
) {
    let Some(container) = viewport.container(container_id) else {
        return;
    };

    let show_content = |ui: &mut egui::Ui| {
        let mut has_child = false;
        for child_contents in &container.contents {
            has_child |= show_list_item_for_container_child(ctx, viewport, ui, child_contents);
        }

        if !has_child {
            ui.list_item_flat_noninteractive(
                list_item::LabelContent::new("empty — use the + button to add content")
                    .weak(true)
                    .italics(true),
            );
        }
    };

    ui.section_collapsing_header("Contents")
        .with_action_button(
            &re_ui::icons::ADD,
            "Add a new view or container to this container",
            || {
                show_add_view_or_container_modal(*container_id);
            },
        )
        .show(ui, show_content);
}

fn data_section_ui(item: &Item) -> Option<Box<dyn DataUi>> {
    match item {
        Item::AppId(app_id) => Some(Box::new(app_id.clone())),
        Item::DataSource(data_source) => Some(Box::new(data_source.clone())),
        Item::StoreId(store_id) => Some(Box::new(store_id.clone())),
        Item::ComponentPath(component_path) => Some(Box::new(component_path.clone())),
        Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
            Some(Box::new(instance_path.clone()))
        }
        // Skip data ui since we don't know yet what to show for these.
        Item::TableId(_)
        | Item::View(_)
        | Item::Container(_)
        | Item::RedapEntry(_)
        | Item::RedapServer(_) => None,
    }
}

fn view_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view: &re_viewport_blueprint::ViewBlueprint,
) -> egui::Response {
    let item = Item::View(view.id);
    let is_selected = ctx.selection().contains_item(&item);
    let view_name = view.display_name_or_default();
    let class = view.class(ctx.view_class_registry());

    let response = ui
        .selectable_label_with_icon(
            class.icon(),
            view_name.as_ref(),
            is_selected,
            contents_name_style(&view_name),
        )
        .on_hover_text(format!("{} view", class.display_name()));
    item_ui::cursor_interact_with_selectable(ctx, response, item)
}

/// Display a list of all the views an entity appears in.
fn list_existing_data_blueprints(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    instance_path: &InstancePath,
) {
    let views_with_path =
        viewport.views_containing_entity_path(ctx, instance_path.entity_path.hash());

    let (query, db) = guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);

    if views_with_path.is_empty() {
        ui.weak("(Not shown in any view)");
    } else {
        for &view_id in &views_with_path {
            if let Some(view) = viewport.view(&view_id) {
                let response = ui.list_item().show_flat(
                    ui,
                    PropertyContent::new("Shown in").value_fn(|ui, _| {
                        view_button(ctx, ui, view);
                    }),
                );

                let item = Item::DataResult(view_id, instance_path.clone());
                let response = response.on_hover_ui(|ui| {
                    let include_subtree = false;
                    item_ui::instance_hover_card_ui(
                        ui,
                        ctx,
                        &query,
                        db,
                        instance_path,
                        include_subtree,
                    );
                });

                // We don't use item_ui::cursor_interact_with_selectable here because the forced
                // hover background is distracting and not useful.
                ctx.handle_select_hover_drag_interactions(&response, item, false);
            }
        }
    }
}

/// Display the top-level properties of a view.
///
/// This includes the name, space origin entity, and view type. These properties are singled
/// out as needing to be edited in most case when creating a new view, which is why they are
/// shown at the very top.
fn view_top_level_properties(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view: &re_viewport_blueprint::ViewBlueprint,
) {
    ui.list_item_flat_noninteractive(PropertyContent::new("Name").value_fn(|ui, _| {
        ui.spacing_mut().text_edit_width = ui
            .spacing_mut()
            .text_edit_width
            .at_least(ui.available_width());

        let mut name = view.display_name.clone().unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("(default)"));
        view.set_display_name(ctx, if name.is_empty() { None } else { Some(name) });
    }));

    ui.list_item_flat_noninteractive(PropertyContent::new("Space origin").value_fn(|ui, _| {
        ui.spacing_mut().text_edit_width = ui
            .spacing_mut()
            .text_edit_width
            .at_least(ui.available_width());

        super::view_space_origin_ui::view_space_origin_widget_ui(ui, ctx, view);
    }))
    .on_hover_text(
        "The origin entity for this view. For spatial views, the space \
                    view's origin is the same as this entity's origin and all transforms are \
                    relative to it.",
    );

    ui.list_item_flat_noninteractive(
        PropertyContent::new("View type")
            .value_text(view.class(ctx.view_class_registry()).display_name()),
    )
    .on_hover_text("The type of this view");
}

fn container_top_level_properties(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    container_id: &ContainerId,
) {
    let Some(container) = viewport.container(container_id) else {
        return;
    };

    ui.list_item_flat_noninteractive(PropertyContent::new("Name").value_fn(|ui, _| {
        ui.spacing_mut().text_edit_width = ui
            .spacing_mut()
            .text_edit_width
            .at_least(ui.available_width());

        let mut name = container.display_name.clone().unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut name));
        container.set_display_name(ctx, if name.is_empty() { None } else { Some(name) });
    }));

    ui.list_item_flat_noninteractive(PropertyContent::new("Container kind").value_fn(|ui, _| {
        let mut container_kind = container.container_kind;
        container_kind_selection_ui(ui, &mut container_kind);
        viewport.set_container_kind(*container_id, container_kind);
    }));

    if container.container_kind == ContainerKind::Grid {
        ui.list_item_flat_noninteractive(PropertyContent::new("Columns").value_fn(|ui, _| {
            fn columns_to_string(columns: &Option<u32>) -> String {
                match columns {
                    None => "Auto".to_owned(),
                    Some(cols) => cols.to_string(),
                }
            }

            let mut new_columns = container.grid_columns;

            egui::ComboBox::from_id_salt("container_grid_columns")
                .selected_text(columns_to_string(&new_columns))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut new_columns, None, columns_to_string(&None));

                    ui.separator();

                    for columns in 1..=container.contents.len() as u32 {
                        ui.selectable_value(
                            &mut new_columns,
                            Some(columns),
                            columns_to_string(&Some(columns)),
                        );
                    }
                });

            container.set_grid_columns(ctx, new_columns);
        }));
    }

    ui.list_item_flat_noninteractive(
        list_item::ButtonContent::new("Simplify hierarchy")
            .on_click(|| {
                viewport.simplify_container(
                    container_id,
                    egui_tiles::SimplificationOptions {
                        prune_empty_tabs: true,
                        prune_empty_containers: true,
                        prune_single_child_tabs: false,
                        prune_single_child_containers: false,
                        all_panes_must_have_tabs: true,
                        join_nested_linear_containers: true,
                    },
                );
            })
            .hover_text("Simplify this container and its children"),
    );

    fn equal_shares(shares: &[f32]) -> bool {
        shares.iter().all(|&share| share == shares[0])
    }

    let all_shares_are_equal =
        equal_shares(&container.col_shares) && equal_shares(&container.row_shares);

    if container.contents.len() > 1
        && match container.container_kind {
            ContainerKind::Tabs => false,
            ContainerKind::Horizontal | ContainerKind::Vertical | ContainerKind::Grid => true,
        }
    {
        ui.list_item_flat_noninteractive(
            list_item::ButtonContent::new("Distribute content equally")
                .on_click(|| {
                    viewport.make_all_children_same_size(container_id);
                })
                .enabled(!all_shares_are_equal)
                .hover_text("Make all children the same size"),
        );
    }
}

fn container_kind_selection_ui(ui: &mut egui::Ui, in_out_kind: &mut ContainerKind) {
    let selected_text = format!("{in_out_kind:?}");

    ui.drop_down_menu("container_kind", selected_text, |ui| {
        static_assertions::const_assert_eq!(ContainerKind::ALL.len(), 4);
        for (kind, icon) in [
            (ContainerKind::Tabs, &icons::CONTAINER_TABS),
            (ContainerKind::Grid, &icons::CONTAINER_GRID),
            (ContainerKind::Horizontal, &icons::CONTAINER_HORIZONTAL),
            (ContainerKind::Vertical, &icons::CONTAINER_VERTICAL),
        ] {
            let response = ui.list_item().selected(*in_out_kind == kind).show_flat(
                ui,
                list_item::LabelContent::new(format!("{kind:?}")).with_icon(icon),
            );

            if response.clicked() {
                *in_out_kind = kind;
            }
        }
    })
    .widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, "Container kind"));
}

// TODO(#4560): this code should be generic and part of re_data_ui
/// Show a list item for a single container child.
///
/// Return true if successful.
fn show_list_item_for_container_child(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    child_contents: &Contents,
) -> bool {
    let mut remove_contents = false;
    let (item, list_item_content) = match child_contents {
        Contents::View(view_id) => {
            let Some(view) = viewport.view(view_id) else {
                re_log::warn_once!("Could not find view with ID {view_id:?}",);
                return false;
            };

            let view_name = view.display_name_or_default();
            (
                Item::View(*view_id),
                list_item::LabelContent::new(view_name.as_ref())
                    .label_style(contents_name_style(&view_name))
                    .with_icon(view.class(ctx.view_class_registry()).icon())
                    .with_buttons(|ui| {
                        let response = ui
                            .small_icon_button(&icons::REMOVE, "Remove this view")
                            .on_hover_text("Remove this view");

                        if response.clicked() {
                            remove_contents = true;
                        }
                    }),
            )
        }
        Contents::Container(container_id) => {
            let Some(container) = viewport.container(container_id) else {
                re_log::warn_once!("Could not find container with ID {container_id:?}",);
                return false;
            };

            let container_name = container.display_name_or_default();

            (
                Item::Container(*container_id),
                list_item::LabelContent::new(container_name.as_ref())
                    .label_style(contents_name_style(&container_name))
                    .with_icon(icon_for_container_kind(&container.container_kind))
                    .with_buttons(|ui| {
                        let response = ui
                            .small_icon_button(&icons::REMOVE, "Remove this container")
                            .on_hover_text("Remove this container");

                        if response.clicked() {
                            remove_contents = true;
                        }
                    }),
            )
        }
    };

    let is_item_hovered =
        ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

    let response = ui
        .list_item()
        .force_hovered(is_item_hovered)
        .show_flat(ui, list_item_content);

    context_menu_ui_for_item(
        ctx,
        viewport,
        &item,
        &response,
        SelectionUpdateBehavior::Ignore,
    );
    ctx.handle_select_hover_drag_interactions(&response, item, false);

    if remove_contents {
        viewport.mark_user_interaction(ctx);
        viewport.remove_contents(*child_contents);
    }

    true
}

fn visible_interactive_toggle_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    query_result: &DataQueryResult,
    data_result: &DataResult,
) {
    {
        let visible_before = data_result.is_visible();
        let mut visible = visible_before;

        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Visible").value_bool_mut(&mut visible),
        )
        .on_hover_text("If disabled, the entity won't be shown in the view.");

        if visible_before != visible {
            data_result.save_visible(ctx.viewer_ctx, &query_result.tree, visible);
        }
    }
    {
        let interactive_before = data_result.is_interactive();
        let mut interactive = interactive_before;

        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Interactive").value_bool_mut(&mut interactive),
        )
        .on_hover_text("If disabled, the entity will not react to any mouse interaction.");

        if interactive_before != interactive {
            data_result.save_interactive(ctx.viewer_ctx, &query_result.tree, interactive);
        }
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::{LatestAtQuery, RowId, TimePoint, Timeline};
    use re_log_types::TimeType;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_sdk_types::archetypes;
    use re_test_context::TestContext;
    use re_test_context::external::egui_kittest::kittest::Queryable as _;
    use re_test_viewport::{TestContextExt as _, TestView};
    use re_viewer_context::{RecommendedView, ViewClass as _, blueprint_timeline};
    use re_viewport_blueprint::ViewBlueprint;

    use super::*;

    fn get_test_context() -> TestContext {
        let mut test_context = TestContext::new_with_store_info(
            re_log_types::StoreInfo::testing_with_recording_id("test_recording"),
        );
        test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
        re_data_ui::register_component_uis(&mut test_context.component_ui_registry);
        test_context
    }

    fn selection_panel_ui(
        test_context: &TestContext,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) {
        test_context.run(&ui.ctx().clone(), |viewer_ctx| {
            ui.scope_builder(
                // We need this to for `re_ui::is_in_resizable_panel` to return the correct thing…
                egui::UiBuilder::new()
                    .ui_stack_info(egui::UiStackInfo::new(egui::UiKind::RightPanel)),
                |ui| {
                    egui::Frame::new().inner_margin(8.0).show(ui, |ui| {
                        SelectionPanel::default().contents(
                            viewer_ctx,
                            viewport_blueprint,
                            &mut ViewStates::default(),
                            ui,
                        );
                    });
                },
            );
        });

        test_context.handle_system_commands(ui.ctx());
    }

    /// Snapshot test for the selection panel when a recording is selected.
    #[test]
    fn selection_panel_recording_snapshot() {
        let mut test_context = get_test_context();

        // Select recording:
        let recording_id = test_context.active_store_id();
        test_context
            .selection_state
            .lock()
            .set_selection(Item::StoreId(recording_id));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([600.0, 400.0])
            .build_ui(move |ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();

        {
            // Redact size estimation, since small changes to our code can change it slightly:
            let mut recording_size_label = harness
                .get_all_by_label_contains(" KiB")
                .next()
                .unwrap()
                .rect();
            recording_size_label.set_width(80.0); // Compensate for different number of digits
            harness.mask(recording_size_label);
        }

        harness.snapshot("selection_panel_recording");
    }

    /// Snapshot test for the selection panel when a recording is selected
    /// and hovering on app id.
    #[test]
    fn selection_panel_recording_hover_app_id_snapshot() {
        let mut test_context = get_test_context();

        // Select recording:
        let recording_id = test_context.active_store_id();
        test_context
            .selection_state
            .lock()
            .set_selection(Item::StoreId(recording_id));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([600.0, 400.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.get_by_label("test_app").hover();

        harness.run_steps(5);

        harness.snapshot("selection_panel_recording_hover_app_id");
    }

    /// Snapshot test for the selection panel when a static component is selected.
    #[test]
    fn selection_panel_static_component_snapshot() {
        let mut test_context = get_test_context();

        let entity_path = EntityPath::from("static");
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });

        let component_path = re_log_types::ComponentPath {
            entity_path,
            component: archetypes::Points2D::descriptor_positions().component,
        };

        test_context
            .selection_state
            .lock()
            .set_selection(Item::ComponentPath(component_path));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 350.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();
        harness.snapshot("selection_panel_component_static");
    }

    /// Snapshot test for the selection panel when a static component that was overwritten is selected.
    #[test]
    fn selection_panel_component_static_overwrite_snapshot() {
        let mut test_context = get_test_context();

        let entity_path = EntityPath::from("static");

        test_context.log_entity(entity_path.clone(), |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &archetypes::Points2D::new([(0., 0.), (1., 1.), (5., 2.)]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &archetypes::Points2D::new([(0., 0.), (1., 1.), (10., 2.)]),
                )
        });

        test_context
            .selection_state
            .lock()
            .set_selection(Item::ComponentPath(re_log_types::ComponentPath {
                entity_path,
                component: archetypes::Points2D::descriptor_positions().component,
            }));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 350.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();
        harness.snapshot("selection_panel_component_static_overwrite");
    }

    /// Snapshot test for the selection panel when a static component with additional time information is
    /// selected (which means a user error).
    #[test]
    fn selection_panel_component_static_hybrid_snapshot() {
        let mut test_context = get_test_context();

        let entity_path = EntityPath::from("hybrid");

        let timeline_frame = Timeline::new("frame_nr", TimeType::Sequence);
        let timeline_other = Timeline::new("other_time", TimeType::Sequence);

        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_frame, 1)],
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_frame, 1)],
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_other, 10)],
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });

        test_context
            .selection_state
            .lock()
            .set_selection(Item::ComponentPath(re_log_types::ComponentPath {
                entity_path,
                component: archetypes::Points2D::descriptor_positions().component,
            }));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 350.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();
        harness.snapshot("selection_panel_component_hybrid_overwrite");
    }

    #[test]
    fn selection_panel_view_snapshot() {
        let mut test_context = get_test_context();
        test_context.register_view_class::<TestView>();

        let entity_path = EntityPath::from("static");
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &MyPoints::new([MyPoint::default()]),
            )
        });

        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint
                .add_view_at_root(ViewBlueprint::new_with_root_wildcard(TestView::identifier()))
        });

        test_context
            .selection_state
            .lock()
            .set_selection(Item::View(view_id));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 500.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();

        harness.snapshot("selection_panel_view");
    }

    #[test]
    fn selection_panel_view_entity_no_visualizable_snapshot() {
        let mut test_context = get_test_context();
        test_context.register_view_class::<TestView>();

        let entity_path = EntityPath::from("points2d");
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &archetypes::Points2D::new([(0., 0.), (1., 1.), (2., 2.)]),
            )
        });

        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint
                .add_view_at_root(ViewBlueprint::new_with_root_wildcard(TestView::identifier()))
        });

        // Select component:
        test_context
            .selection_state
            .lock()
            .set_selection(Item::View(view_id));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 500.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();

        harness.snapshot("selection_panel_view_entity_no_visualizable");
    }

    #[test]
    fn selection_panel_view_entity_no_match_snapshot() {
        let mut test_context = get_test_context();
        test_context.register_view_class::<TestView>();

        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new(
                TestView::identifier(),
                RecommendedView::new_single_entity("does_not_exist"),
            ))
        });

        // Select component:
        test_context
            .selection_state
            .lock()
            .set_selection(Item::View(view_id));

        let viewport_blueprint = ViewportBlueprint::from_db(
            test_context.active_blueprint(),
            &LatestAtQuery::latest(blueprint_timeline()),
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([400.0, 500.0])
            .build_ui(|ui| {
                selection_panel_ui(&test_context, &viewport_blueprint, ui);
            });

        harness.run();

        harness.snapshot("selection_panel_view_entity_no_match");
    }
}
