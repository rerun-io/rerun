use egui::{NumExt as _, Ui};
use egui_tiles::ContainerKind;

use re_context_menu::{context_menu_ui_for_item, SelectionUpdateBehavior};
use re_data_ui::{
    image_meaning_for_entity, item_ui,
    item_ui::{guess_instance_path_icon, guess_query_and_db_for_selected_entity},
    DataUi,
};
use re_entity_db::{
    ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties, InstancePath,
};
use re_log_types::EntityPathFilter;
use re_space_view::DataResultQuery as _;
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    archetypes::{Axes3D, Pinhole},
    components::{
        AxisLength, ImagePlaneDistance, PinholeProjection, Transform3D, VisualizerOverrides,
    },
    tensor_data::TensorDataMeaning,
};
use re_ui::{icons, list_item, ContextExt as _, DesignTokens, SyntaxHighlighting as _, UiExt as _};
use re_viewer_context::{
    contents_name_style, gpu_bridge::colormap_dropdown_button_ui, icon_for_container_kind,
    ContainerId, Contents, DataQueryResult, DataResult, HoverHighlight, Item, SpaceViewClass,
    SpaceViewId, UiLayout, ViewContext, ViewStates, ViewerContext,
};
use re_viewport_blueprint::{
    ui::show_add_space_view_or_container_modal, SpaceViewBlueprint, ViewportBlueprint,
};

use crate::override_ui::{override_ui, override_visualizer_ui};
use crate::space_view_entity_picker::SpaceViewEntityPicker;
use crate::{
    query_range_ui::query_range_ui_data_result, query_range_ui::query_range_ui_space_view,
    selection_history_ui::SelectionHistoryUi,
};

// ---
fn default_selection_panel_width(screen_width: f32) -> f32 {
    (0.45 * screen_width).min(300.0).round()
}

/// The "Selection view" sidebar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SelectionPanel {
    selection_state_ui: SelectionHistoryUi,

    #[serde(skip)]
    /// State for the "Add entity" modal.
    space_view_entity_modal: SpaceViewEntityPicker,
}

impl SelectionPanel {
    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        blueprint: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
        expanded: bool,
    ) {
        let screen_width = ui.ctx().screen_rect().width();

        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width(default_selection_panel_width(screen_width))
            .max_width((0.65 * screen_width).round())
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.style().visuals.panel_fill,
                ..Default::default()
            });

        // Always reset the VH highlight, and let the UI re-set it if needed.
        ctx.rec_cfg.time_ctrl.write().highlighted_range = None;

        panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
            ui.panel_content(|ui| {
                let hover = "The Selection View contains information and options about \
                    the currently selected object(s)";
                ui.panel_title_bar_with_buttons("Selection", Some(hover), |ui| {
                    let mut history = ctx.selection_state().history.lock();
                    if let Some(selection) =
                        self.selection_state_ui
                            .selection_ui(ui, blueprint, &mut history)
                    {
                        ctx.selection_state().set_selection(selection);
                    }
                });
            });

            // move the vertical spacing between the title and the content to _inside_ the scroll
            // area
            ui.add_space(-ui.spacing().item_spacing.y);

            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(ui.spacing().item_spacing.y);
                    ui.panel_content(|ui| {
                        self.contents(ctx, blueprint, view_states, ui);
                    });
                });
        });

        // run modals (these are noop if the modals are not active)
        self.space_view_entity_modal.ui(ui.ctx(), ctx, blueprint);
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ctx: &ViewerContext<'_>,
        blueprint: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
    ) {
        re_tracing::profile_function!();

        if ctx.selection().is_empty() {
            return;
        }

        // no gap before the first item title
        ui.add_space(-ui.spacing().item_spacing.y);

        let selection = ctx.selection();
        let ui_layout = if selection.len() > 1 {
            UiLayout::SelectionPanelLimitHeight
        } else {
            UiLayout::SelectionPanelFull
        };
        for (i, item) in selection.iter_items().enumerate() {
            ui.push_id(item, |ui| {
                what_is_selected_ui(ctx, blueprint, ui, item);

                match item {
                    Item::Container(container_id) => {
                        container_top_level_properties(ctx, blueprint, ui, container_id);
                        ui.add_space(12.0);
                        container_children(ctx, blueprint, ui, container_id);
                    }

                    Item::SpaceView(space_view_id) => {
                        space_view_top_level_properties(ctx, blueprint, ui, space_view_id);
                    }

                    _ => {}
                }

                if let Some(data_ui_item) = data_section_ui(item) {
                    ui.large_collapsing_header("Data", true, |ui| {
                        let (query, db) = if let Some(entity_path) = item.entity_path() {
                            guess_query_and_db_for_selected_entity(ctx, entity_path)
                        } else {
                            (ctx.current_query(), ctx.recording())
                        };
                        data_ui_item.data_ui(ctx, ui, ui_layout, &query, db);
                    });
                }

                // Special override section for space-view-entities
                if let Item::DataResult(view_id, instance_path) = item {
                    if let Some(view) = blueprint.space_views.get(view_id) {
                        // TODO(jleibs): Overrides still require special handling inside the visualizers.
                        // For now, only show the override section for TimeSeries until support is implemented
                        // generically.
                        if view.class_identifier() == TimeSeriesSpaceView::identifier()
                            || ctx.app_options.experimental_visualizer_selection
                        {
                            ui.large_collapsing_header("Visualizers", true, |ui| {
                                override_visualizer_ui(ctx, view, instance_path, ui);
                            });

                            let view_state = view_states
                                .get_mut(
                                    ctx.space_view_class_registry,
                                    *view_id,
                                    view.class_identifier(),
                                )
                                .view_state
                                .as_ref();

                            let visualizer_collection = ctx
                                .space_view_class_registry
                                .new_visualizer_collection(view.class_identifier());

                            let view_ctx = ViewContext {
                                viewer_ctx: ctx,
                                view_id: *view_id,
                                view_state,
                                visualizer_collection: &visualizer_collection,
                            };

                            ui.large_collapsing_header("Component Overrides", true, |ui| {
                                override_ui(&view_ctx, view, instance_path, ui);
                            });
                        }
                    }
                }

                if has_blueprint_section(item) {
                    ui.large_collapsing_header("Blueprint", true, |ui| {
                        self.blueprint_ui(ctx, blueprint, view_states, ui, item);
                    });
                }

                if i < selection.len() - 1 {
                    // Add space some space between selections
                    ui.add_space(8.);
                }
            });
        }
    }

    /// What is the blueprint stuff for this item?
    fn blueprint_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        blueprint: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut egui::Ui,
        item: &Item,
    ) {
        match item {
            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::ComponentPath(_)
            | Item::Container(_)
            | Item::InstancePath(_) => {}

            Item::SpaceView(space_view_id) => {
                self.blueprint_ui_for_space_view(ctx, blueprint, view_states, ui, *space_view_id);
            }

            Item::DataResult(space_view_id, instance_path) => {
                let Some(space_view) = blueprint.space_view(space_view_id) else {
                    return;
                };

                let visualizer_collection = ctx
                    .space_view_class_registry
                    .new_visualizer_collection(space_view.class_identifier());

                let Some(view_state) = view_states
                    .get(*space_view_id)
                    .map(|states| states.view_state.as_ref())
                else {
                    return;
                };

                let view_ctx = ViewContext {
                    viewer_ctx: ctx,
                    view_id: *space_view_id,
                    view_state,
                    visualizer_collection: &visualizer_collection,
                };

                blueprint_ui_for_data_result(
                    &view_ctx,
                    space_view,
                    ui,
                    *space_view_id,
                    instance_path,
                );
            }
        }
    }

    fn blueprint_ui_for_space_view(
        &mut self,
        ctx: &ViewerContext<'_>,
        blueprint: &ViewportBlueprint,
        view_states: &mut ViewStates,
        ui: &mut Ui,
        space_view_id: SpaceViewId,
    ) {
        if let Some(space_view) = blueprint.space_view(&space_view_id) {
            if let Some(new_entity_path_filter) = self.entity_path_filter_ui(
                ctx,
                ui,
                space_view_id,
                &space_view.contents.entity_path_filter,
                &space_view.space_origin,
            ) {
                space_view
                    .contents
                    .set_entity_path_filter(ctx, &new_entity_path_filter);
            }

            ui.add_space(ui.spacing().item_spacing.y);
        }

        if ui
            .button("Clone space view")
            .on_hover_text(
                "Create an exact duplicate of this space view including all blueprint settings",
            )
            .clicked()
        {
            if let Some(new_space_view_id) = blueprint.duplicate_space_view(&space_view_id, ctx) {
                ctx.selection_state()
                    .set_selection(Item::SpaceView(new_space_view_id));
                blueprint.mark_user_interaction(ctx);
            }
        }

        ui.add_space(ui.spacing().item_spacing.y / 2.0);
        ui.full_span_separator();
        ui.add_space(ui.spacing().item_spacing.y / 2.0);

        if let Some(space_view) = blueprint.space_view(&space_view_id) {
            let class_identifier = space_view.class_identifier();

            let space_view_state = view_states.get_mut(
                ctx.space_view_class_registry,
                space_view.id,
                class_identifier,
            );

            query_range_ui_space_view(ctx, ui, space_view);

            // Space View don't inherit (legacy) properties.
            let mut props =
                space_view.legacy_properties(ctx.store_context.blueprint, ctx.blueprint_query);
            let props_before = props.clone();

            let space_view_class = space_view.class(ctx.space_view_class_registry);
            if let Err(err) = space_view_class.selection_ui(
                ctx,
                ui,
                space_view_state.view_state.as_mut(),
                &space_view.space_origin,
                space_view.id,
                &mut props,
            ) {
                re_log::error!(
                    "Error in space view selection UI (class: {}, display name: {}): {err}",
                    space_view.class_identifier(),
                    space_view_class.display_name(),
                );
            }

            if props_before != props {
                space_view.save_legacy_properties(ctx, props);
            }
        }
    }

    /// Returns a new filter when the editing is done, and there has been a change.
    fn entity_path_filter_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view_id: SpaceViewId,
        filter: &EntityPathFilter,
        origin: &EntityPath,
    ) -> Option<EntityPathFilter> {
        fn entity_path_filter_help_ui(ui: &mut egui::Ui) {
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

            ui.markdown_ui(egui::Id::new("entity_path_filter_help_ui"), markdown);
        }

        fn syntax_highlight_entity_path_filter(
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
                    egui::Color32::LIGHT_RED
                } else {
                    egui::Color32::LIGHT_GREEN
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
            string: &str,
            wrap_width: f32,
        ) -> std::sync::Arc<egui::Galley> {
            let mut layout_job = syntax_highlight_entity_path_filter(ui.style(), string);
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        }

        // We store the string we are temporarily editing in the `Ui`'s temporary data storage.
        // This is so it can contain invalid rules while the user edits it, and it's only normalized
        // when they press enter, or stops editing.
        let filter_text_id = ui.id().with("filter_text");

        let mut filter_string = ui.data_mut(|data| {
            data.get_temp_mut_or_insert_with::<String>(filter_text_id, || filter.formatted())
                .clone()
        });

        let rightmost_x = ui.cursor().min.x;
        ui.horizontal(|ui| {
            ui.label("Entity path query").on_hover_text(
                "The entity path query consists of a list of include/exclude rules \
            that determines what entities are part of this space view",
            );

            let current_x = ui.cursor().min.x;
            // Compute a width that results in these things to be right-aligned with the following text edit.
            let desired_width = (ui.available_width() - ui.spacing().item_spacing.x)
                .at_most(ui.spacing().text_edit_width - (current_x - rightmost_x));

            ui.allocate_ui_with_layout(
                egui::vec2(desired_width, ui.available_height()),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.help_hover_button()
                        .on_hover_ui(entity_path_filter_help_ui);
                    if ui
                        .button("Edit")
                        .on_hover_text("Modify the entity query using the editor")
                        .clicked()
                    {
                        self.space_view_entity_modal.open(space_view_id);
                    }
                },
            );
        });

        let response =
            ui.add(egui::TextEdit::multiline(&mut filter_string).layouter(&mut text_layouter));

        if response.has_focus() {
            ui.data_mut(|data| data.insert_temp::<String>(filter_text_id, filter_string.clone()));
        } else {
            // Reconstruct it from the filter next frame
            ui.data_mut(|data| data.remove::<String>(filter_text_id));
        }

        // Show some statistics about the query, print a warning text if something seems off.
        let query = ctx.lookup_query_result(space_view_id);
        if query.num_matching_entities == 0 {
            ui.label(ui.ctx().warning_text("Does not match any entity"));
        } else if query.num_matching_entities == 1 {
            ui.label("Matches 1 entity");
        } else {
            ui.label(format!("Matches {} entities", query.num_matching_entities));
        }
        if query.num_matching_entities != 0 && query.num_visualized_entities == 0 {
            // TODO(andreas): Talk about this root bit only if it's a spatial view.
            ui.label(ui.ctx().warning_text(
                format!("This space view is not able to visualize any of the matched entities using the current root \"{origin:?}\"."),
            ));
        }

        // Apply the edit.
        let new_filter = EntityPathFilter::parse_forgiving(&filter_string, &Default::default());
        if &new_filter == filter {
            None // no change
        } else {
            Some(new_filter)
        }
    }
}

fn container_children(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
    container_id: &ContainerId,
) {
    let Some(container) = blueprint.container(container_id) else {
        return;
    };

    ui.horizontal(|ui| {
        ui.strong("Contents");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_icon_button(&icons::ADD).clicked() {
                show_add_space_view_or_container_modal(*container_id);
            }
        });
    });

    let show_content = |ui: &mut egui::Ui| {
        let mut has_child = false;
        for child_contents in &container.contents {
            has_child |= show_list_item_for_container_child(ctx, blueprint, ui, child_contents);
        }

        if !has_child {
            ui.list_item().interactive(false).show_flat(
                ui,
                list_item::LabelContent::new("empty — use the + button to add content")
                    .weak(true)
                    .italics(true),
            );
        }
    };

    egui::Frame {
        outer_margin: egui::Margin::ZERO,
        inner_margin: egui::Margin::ZERO,
        stroke: ui.visuals().widgets.noninteractive.bg_stroke,
        ..Default::default()
    }
    .show(ui, |ui| {
        list_item::list_item_scope(ui, "children list", |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            egui::Frame {
                inner_margin: egui::Margin::symmetric(4.0, 0.0),
                ..Default::default()
            }
            .show(ui, show_content);
        });
    });
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
        Item::SpaceView(_) | Item::Container(_) => None,
    }
}

fn space_view_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view: &re_viewport_blueprint::SpaceViewBlueprint,
) -> egui::Response {
    let item = Item::SpaceView(space_view.id);
    let is_selected = ctx.selection().contains_item(&item);
    let space_view_name = space_view.display_name_or_default();

    let response = ui
        .selectable_label_with_icon(
            space_view.class(ctx.space_view_class_registry).icon(),
            space_view_name.as_ref(),
            is_selected,
            contents_name_style(&space_view_name),
        )
        .on_hover_text("Space view");
    item_ui::cursor_interact_with_selectable(ctx, response, item)
}

/// What is selected and where is it located?
///
/// This includes a title bar and contextual information about there this item is located.
fn what_is_selected_ui(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    match item {
        Item::AppId(app_id) => {
            let title = app_id.to_string();
            item_title_ui(ui, &title, Some(&icons::APPLICATION), None, &title);
        }
        Item::DataSource(data_source) => {
            let title = data_source.to_string();
            item_title_ui(ui, &title, Some(&icons::DATA_SOURCE), None, &title);
        }

        Item::StoreId(store_id) => {
            let id_str = format!("{} ID: {}", store_id.kind, store_id);

            let title = if let Some(entity_db) = ctx.store_context.bundle.get(store_id) {
                if let Some(info) = entity_db.store_info() {
                    let time = info
                        .started
                        .format_time_custom("[hour]:[minute]:[second]", ctx.app_options.time_zone)
                        .unwrap_or("<unknown time>".to_owned());

                    format!("{} - {}", info.application_id, time)
                } else {
                    id_str.clone()
                }
            } else {
                id_str.clone()
            };

            let icon = match store_id.kind {
                re_log_types::StoreKind::Recording => &icons::RECORDING,
                re_log_types::StoreKind::Blueprint => &icons::BLUEPRINT,
            };

            item_title_ui(ui, &title, Some(icon), None, &id_str);
        }

        Item::Container(container_id) => {
            if let Some(container_blueprint) = blueprint.container(container_id) {
                let hover_text =
                    if let Some(display_name) = container_blueprint.display_name.as_ref() {
                        format!(
                            "{:?} container {display_name:?}",
                            container_blueprint.container_kind,
                        )
                    } else {
                        format!("Unnamed {:?} container", container_blueprint.container_kind,)
                    };

                let container_name = container_blueprint.display_name_or_default();
                item_title_ui(
                    ui,
                    container_name.as_ref(),
                    Some(re_viewer_context::icon_for_container_kind(
                        &container_blueprint.container_kind,
                    )),
                    Some(contents_name_style(&container_name)),
                    &hover_text,
                );
            }
        }

        Item::ComponentPath(component_path) => {
            let entity_path = &component_path.entity_path;
            let component_name = &component_path.component_name;

            let (query, db) = guess_query_and_db_for_selected_entity(ctx, entity_path);
            let is_static = db.is_component_static(component_path).unwrap_or_default();

            item_title_ui(
                ui,
                component_name.short_name(),
                Some(if is_static {
                    &icons::COMPONENT_STATIC
                } else {
                    &icons::COMPONENT_TEMPORAL
                }),
                None,
                &format!(
                    "{} component {} of entity '{}'",
                    if is_static { "Static" } else { "Temporal" },
                    component_name.full_name(),
                    entity_path
                ),
            );

            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} component of",
                    if is_static { "Static" } else { "Temporal" }
                ));
                item_ui::entity_path_button(ctx, &query, db, ui, None, entity_path);
            });

            list_existing_data_blueprints(ctx, blueprint, ui, &entity_path.clone().into());
        }

        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.space_view(space_view_id) {
                let space_view_class = space_view.class(ctx.space_view_class_registry);

                let hover_text = if let Some(display_name) = space_view.display_name.as_ref() {
                    format!(
                        "Space view {:?} of type {}",
                        display_name,
                        space_view_class.display_name()
                    )
                } else {
                    format!(
                        "Unnamed space view of type {}",
                        space_view_class.display_name()
                    )
                };

                let space_view_name = space_view.display_name_or_default();
                item_title_ui(
                    ui,
                    space_view_name.as_ref(),
                    Some(space_view.class(ctx.space_view_class_registry).icon()),
                    Some(contents_name_style(&space_view_name)),
                    &hover_text,
                );
            }
        }

        Item::InstancePath(instance_path) => {
            let typ = item.kind();
            let name = instance_path.syntax_highlighted(ui.style());

            item_title_ui(
                ui,
                name,
                Some(guess_instance_path_icon(ctx, instance_path)),
                None,
                &format!("{typ} '{instance_path}'"),
            );

            let is_instance = !instance_path.instance.is_all();
            let parent = if is_instance {
                Some(instance_path.entity_path.clone())
            } else {
                instance_path.entity_path.parent()
            };
            if let Some(parent) = parent {
                if !parent.is_root() {
                    let (query, db) =
                        guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);
                    ui.horizontal(|ui| {
                        ui.label("Parent");
                        item_ui::entity_path_parts_buttons(ctx, &query, db, ui, None, &parent);
                    });
                }
            }

            list_existing_data_blueprints(ctx, blueprint, ui, instance_path);
        }

        Item::DataResult(space_view_id, instance_path) => {
            let name = instance_path.syntax_highlighted(ui.style());

            if let Some(space_view) = blueprint.space_view(space_view_id) {
                let typ = item.kind();
                item_title_ui(
                    ui,
                    name,
                    Some(guess_instance_path_icon(ctx, instance_path)),
                    None,
                    &format!(
                        "{typ} '{instance_path}' as shown in space view {:?}",
                        space_view.display_name
                    ),
                );

                let is_instance = !instance_path.instance.is_all();
                let parent = if is_instance {
                    Some(instance_path.entity_path.clone())
                } else {
                    instance_path.entity_path.parent()
                };
                if let Some(parent) = parent {
                    if !parent.is_root() {
                        ui.horizontal(|ui| {
                            let (query, db) = guess_query_and_db_for_selected_entity(
                                ctx,
                                &instance_path.entity_path,
                            );

                            ui.label("Parent");
                            item_ui::entity_path_parts_buttons(
                                ctx,
                                &query,
                                db,
                                ui,
                                Some(*space_view_id),
                                &parent,
                            );
                        });
                    }
                }

                ui.horizontal(|ui| {
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// A title bar for an item.
fn item_title_ui(
    ui: &mut egui::Ui,
    name: impl Into<egui::WidgetText>,
    icon: Option<&re_ui::Icon>,
    label_style: Option<re_ui::LabelStyle>,
    hover: &str,
) -> egui::Response {
    let mut content = list_item::LabelContent::new(name);

    if let Some(icon) = icon {
        content = content.with_icon(icon);
    }

    if let Some(label_style) = label_style {
        content = content.label_style(label_style);
    }

    list_item::list_item_scope(ui, ui.next_auto_id(), |ui| {
        ui.list_item()
            .with_height(DesignTokens::title_bar_height())
            .selected(true)
            .show_flat(ui, content)
            .on_hover_text(hover)
    })
}

/// Display a list of all the space views an entity appears in.
fn list_existing_data_blueprints(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
    instance_path: &InstancePath,
) {
    let space_views_with_path =
        blueprint.space_views_containing_entity_path(ctx, &instance_path.entity_path);

    let (query, db) = guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any space view)");
    } else {
        for space_view_id in &space_views_with_path {
            if let Some(space_view) = blueprint.space_view(space_view_id) {
                ui.horizontal(|ui| {
                    item_ui::instance_path_button_to(
                        ctx,
                        &query,
                        db,
                        ui,
                        Some(*space_view_id),
                        instance_path,
                        "Shown",
                    );
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// Display the top-level properties of a space view.
///
/// This includes the name, space origin entity, and space view type. These properties are singled
/// out as needing to be edited in most case when creating a new space view, which is why they are
/// shown at the very top.
fn space_view_top_level_properties(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    space_view_id: &SpaceViewId,
) {
    if let Some(space_view) = viewport.space_view(space_view_id) {
        egui::Grid::new("space_view_top_level_properties")
            .num_columns(2)
            .show(ui, |ui| {
                let mut name = space_view.display_name.clone().unwrap_or_default();
                ui.label("Name").on_hover_text(
                    "The name of the space view used for display purposes. This can be any text \
                    string.",
                );
                ui.text_edit_singleline(&mut name);
                space_view.set_display_name(ctx, if name.is_empty() { None } else { Some(name) });

                ui.end_row();

                ui.label("Space origin").on_hover_text(
                    "The origin entity for this space view. For spatial space views, the space \
                    View's origin is the same as this entity's origin and all transforms are \
                    relative to it.",
                );

                super::space_view_space_origin_ui::space_view_space_origin_widget_ui(
                    ui, ctx, space_view,
                );

                ui.end_row();

                ui.label("Type")
                    .on_hover_text("The type of this space view");
                ui.label(
                    space_view
                        .class(ctx.space_view_class_registry)
                        .display_name(),
                );

                ui.end_row();
            });
    }
}

fn container_top_level_properties(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
    container_id: &ContainerId,
) {
    let Some(container) = blueprint.container(container_id) else {
        return;
    };

    egui::Grid::new("container_top_level_properties")
        .num_columns(2)
        .show(ui, |ui| {
            let mut name = container.display_name.clone().unwrap_or_default();
            ui.label("Name").on_hover_text(
                "The name of the container used for display purposes. This can be any text string.",
            );
            ui.text_edit_singleline(&mut name);
            container.set_display_name(ctx, if name.is_empty() { None } else { Some(name) });

            ui.end_row();

            ui.label("Kind");

            let mut container_kind = container.container_kind;
            container_kind_selection_ui(ui, &mut container_kind);

            blueprint.set_container_kind(*container_id, container_kind);

            ui.end_row();

            if container.container_kind == ContainerKind::Grid {
                ui.label("Columns");

                fn columns_to_string(columns: &Option<u32>) -> String {
                    match columns {
                        None => "Auto".to_owned(),
                        Some(cols) => cols.to_string(),
                    }
                }

                let mut new_columns = container.grid_columns;

                egui::ComboBox::from_id_source("container_grid_columns")
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

                ui.end_row();
            }

            if ui
                .button("Simplify hierarchy")
                .on_hover_text("Simplify this container and its children")
                .clicked()
            {
                blueprint.simplify_container(
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
            }
            ui.end_row();

            // ---

            fn equal_shares(shares: &[f32]) -> bool {
                shares.iter().all(|&share| share == shares[0])
            }

            let all_shares_are_equal =
                equal_shares(&container.col_shares) && equal_shares(&container.row_shares);

            if container.contents.len() > 1
                && match container.container_kind {
                    ContainerKind::Tabs => false,
                    ContainerKind::Horizontal | ContainerKind::Vertical | ContainerKind::Grid => {
                        true
                    }
                }
                && ui
                    .add_enabled(
                        !all_shares_are_equal,
                        egui::Button::new("Distribute content equally"),
                    )
                    .on_hover_text("Make all children the same size")
                    .clicked()
            {
                blueprint.make_all_children_same_size(container_id);
            }
            ui.end_row();
        });
}

fn container_kind_selection_ui(ui: &mut Ui, in_out_kind: &mut ContainerKind) {
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
    });
}

// TODO(#4560): this code should be generic and part of re_data_ui
/// Show a list item for a single container child.
///
/// Return true if successful.
fn show_list_item_for_container_child(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
    child_contents: &Contents,
) -> bool {
    let mut remove_contents = false;
    let (item, list_item_content) = match child_contents {
        Contents::SpaceView(space_view_id) => {
            let Some(space_view) = blueprint.space_view(space_view_id) else {
                re_log::warn_once!("Could not find space view with ID {space_view_id:?}",);
                return false;
            };

            let space_view_name = space_view.display_name_or_default();
            (
                Item::SpaceView(*space_view_id),
                list_item::LabelContent::new(space_view_name.as_ref())
                    .label_style(contents_name_style(&space_view_name))
                    .with_icon(space_view.class(ctx.space_view_class_registry).icon())
                    .with_buttons(|ui| {
                        let response = ui
                            .small_icon_button(&icons::REMOVE)
                            .on_hover_text("Remove this space view");

                        if response.clicked() {
                            remove_contents = true;
                        }

                        response
                    }),
            )
        }
        Contents::Container(container_id) => {
            let Some(container) = blueprint.container(container_id) else {
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
                            .small_icon_button(&icons::REMOVE)
                            .on_hover_text("Remove this container");

                        if response.clicked() {
                            remove_contents = true;
                        }

                        response
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
        blueprint,
        &item,
        &response,
        SelectionUpdateBehavior::Ignore,
    );
    ctx.select_hovered_on_click(&response, item);

    if remove_contents {
        blueprint.mark_user_interaction(ctx);
        blueprint.remove_contents(*child_contents);
    }

    true
}

fn has_blueprint_section(item: &Item) -> bool {
    match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::ComponentPath(_)
        | Item::Container(_)
        | Item::InstancePath(_) => false,

        Item::SpaceView(_) | Item::DataResult(_, _) => true,
    }
}

fn blueprint_ui_for_data_result(
    ctx: &ViewContext<'_>,
    space_view: &SpaceViewBlueprint,
    ui: &mut Ui,
    space_view_id: SpaceViewId,
    instance_path: &InstancePath,
) {
    if instance_path.instance.is_all() {
        // the whole entity
        let entity_path = &instance_path.entity_path;

        let query_result = ctx.lookup_query_result(space_view.id);
        if let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(entity_path)
            .cloned()
        {
            let mut accumulated_legacy_props = data_result.accumulated_properties().clone();
            let accumulated_legacy_props_before = accumulated_legacy_props.clone();

            entity_props_ui(
                ctx,
                ui,
                ctx.lookup_query_result(space_view_id),
                &data_result,
                &mut accumulated_legacy_props,
            );
            if accumulated_legacy_props != accumulated_legacy_props_before {
                data_result.save_individual_override_properties(
                    ctx.viewer_ctx,
                    Some(accumulated_legacy_props),
                );
            }
        }
    }
}

fn entity_props_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    query_result: &DataQueryResult,
    data_result: &DataResult,
    entity_props: &mut EntityProperties,
) {
    use re_types::blueprint::components::Visible;
    use re_types::Loggable as _;

    let entity_path = &data_result.entity_path;

    {
        let visible_before = data_result.is_visible(ctx.viewer_ctx);
        let mut visible = visible_before;

        let override_source =
            data_result.component_override_source(&query_result.tree, &Visible::name());
        let is_inherited =
            override_source.is_some() && override_source.as_ref() != Some(entity_path);

        ui.horizontal(|ui| {
            ui.re_checkbox(&mut visible, "Visible");
            if is_inherited {
                ui.label("(inherited)");
            }
        });

        if visible_before != visible {
            data_result.save_recursive_override_or_clear_if_redundant(
                ctx.viewer_ctx,
                &query_result.tree,
                &Visible(visible),
            );
        }
    }

    ui.re_checkbox(&mut entity_props.interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

    query_range_ui_data_result(ctx.viewer_ctx, ui, data_result);

    egui::Grid::new("entity_properties")
        .num_columns(2)
        .show(ui, |ui| {
            // TODO(wumpf): It would be nice to only show pinhole & depth properties in the context of a 3D view.
            // if *view_state.state_spatial.nav_mode.get() == SpatialNavigationMode::ThreeD {
            pinhole_props_ui(ctx, ui, data_result);
            depth_props_ui(ctx.viewer_ctx, ui, entity_path, entity_props);
            transform3d_visualization_ui(ctx, ui, data_result);
        });
}

fn colormap_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_props: &mut EntityProperties,
) {
    let mut re_renderer_colormap = match *entity_props.color_mapper.get() {
        ColorMapper::Colormap(Colormap::Grayscale) => re_renderer::Colormap::Grayscale,
        ColorMapper::Colormap(Colormap::Turbo) => re_renderer::Colormap::Turbo,
        ColorMapper::Colormap(Colormap::Viridis) => re_renderer::Colormap::Viridis,
        ColorMapper::Colormap(Colormap::Plasma) => re_renderer::Colormap::Plasma,
        ColorMapper::Colormap(Colormap::Magma) => re_renderer::Colormap::Magma,
        ColorMapper::Colormap(Colormap::Inferno) => re_renderer::Colormap::Inferno,
    };

    ui.label("Color map");
    colormap_dropdown_button_ui(ctx.render_ctx, ui, &mut re_renderer_colormap);

    let new_colormap = match re_renderer_colormap {
        re_renderer::Colormap::Grayscale => Colormap::Grayscale,
        re_renderer::Colormap::Turbo => Colormap::Turbo,
        re_renderer::Colormap::Viridis => Colormap::Viridis,
        re_renderer::Colormap::Plasma => Colormap::Plasma,
        re_renderer::Colormap::Magma => Colormap::Magma,
        re_renderer::Colormap::Inferno => Colormap::Inferno,
    };
    entity_props.color_mapper = EditableAutoValue::UserEdited(ColorMapper::Colormap(new_colormap));

    ui.end_row();
}

fn pinhole_props_ui(ctx: &ViewContext<'_>, ui: &mut egui::Ui, data_result: &DataResult) {
    let (query, store) =
        guess_query_and_db_for_selected_entity(ctx.viewer_ctx, &data_result.entity_path);

    if store
        .latest_at_component::<PinholeProjection>(&data_result.entity_path, &query)
        .is_some()
    {
        let results = data_result.latest_at_with_overrides::<Pinhole>(ctx, &query);

        let mut image_plane_value: f32 = results
            .get_mono_with_fallback::<ImagePlaneDistance>()
            .into();

        ui.label("Image plane distance");
        let speed = (image_plane_value * 0.05).at_least(0.01);
        if ui
            .add(
                egui::DragValue::new(&mut image_plane_value)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text("Controls how far away the image plane is")
            .changed()
        {
            let new_image_plane: ImagePlaneDistance = image_plane_value.into();

            data_result.save_individual_override(ctx.viewer_ctx, &new_image_plane);
        }
        ui.end_row();
    }
}

fn transform3d_visualization_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
) {
    re_tracing::profile_function!();

    let (query, store) =
        guess_query_and_db_for_selected_entity(ctx.viewer_ctx, &data_result.entity_path);

    if store
        .latest_at_component::<Transform3D>(&data_result.entity_path, &query)
        .is_none()
    {
        return;
    }

    let arrow_viz = "Transform3DArrows".into();

    let mut show_arrows = data_result.visualizers.contains(&arrow_viz);

    let results = data_result.latest_at_with_overrides::<Axes3D>(ctx, &query);

    let mut arrow_length: f32 = results.get_mono_with_fallback::<AxisLength>().into();

    {
        let response = ui.re_checkbox( &mut show_arrows, "Show transform").on_hover_text(
            "Enables/disables the display of three arrows to visualize the (accumulated) transform at this entity. Red/green/blue show the x/y/z axis respectively.");
        if response.changed() {
            let component = if show_arrows {
                VisualizerOverrides::from(
                    data_result
                        .visualizers
                        .iter()
                        .chain(std::iter::once(&arrow_viz))
                        .map(|v| re_types_core::ArrowString::from(v.as_str()))
                        .collect::<Vec<_>>(),
                )
            } else {
                VisualizerOverrides::from(
                    data_result
                        .visualizers
                        .iter()
                        .filter(|v| **v != arrow_viz)
                        .map(|v| re_types_core::ArrowString::from(v.as_str()))
                        .collect::<Vec<_>>(),
                )
            };

            data_result.save_individual_override(ctx.viewer_ctx, &component);
        }
    }

    if show_arrows {
        ui.end_row();
        ui.label("Transform-arrow length");
        let speed = (arrow_length * 0.05).at_least(0.001);
        let response = ui
            .add(
                egui::DragValue::new(&mut arrow_length)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text(
                "How long the arrows should be in the entity's own coordinate system. Double-click to reset to auto.",
            );
        if response.double_clicked() {
            data_result.clear_individual_override::<AxisLength>(ctx.viewer_ctx);
            response.surrender_focus();
        } else if response.changed() {
            data_result
                .save_individual_override::<AxisLength>(ctx.viewer_ctx, &arrow_length.into());
        }
    }

    ui.end_row();
}

fn depth_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) -> Option<()> {
    re_tracing::profile_function!();

    let (query, db) = guess_query_and_db_for_selected_entity(ctx, entity_path);

    let meaning = image_meaning_for_entity(entity_path, &query, db.store());

    if meaning != TensorDataMeaning::Depth {
        return Some(());
    }
    let image_projection_ent_path = db
        .latest_at_component_at_closest_ancestor::<PinholeProjection>(entity_path, &query)?
        .0;

    let mut backproject_depth = *entity_props.backproject_depth;

    if ui
        .re_checkbox(&mut backproject_depth, "Backproject Depth")
        .on_hover_text(
            "If enabled, the depth texture will be backprojected into a point cloud rather \
                than simply displayed as an image.",
        )
        .changed()
    {
        entity_props.backproject_depth = EditableAutoValue::UserEdited(backproject_depth);
    }
    ui.end_row();

    if backproject_depth {
        ui.label("Pinhole");
        item_ui::entity_path_button(ctx, &query, db, ui, None, &image_projection_ent_path)
            .on_hover_text(
                "The entity path of the pinhole transform being used to do the backprojection.",
            );
        ui.end_row();

        depth_from_world_scale_ui(ui, &mut entity_props.depth_from_world_scale);

        backproject_radius_scale_ui(ui, &mut entity_props.backproject_radius_scale);

        // TODO(cmc): This should apply to the depth map entity as a whole, but for that we
        // need to get the current hardcoded colormapping out of the image cache first.
        colormap_props_ui(ctx, ui, entity_props);
    }

    Some(())
}

fn depth_from_world_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject meter");
    let mut value = *property.get();
    let speed = (value * 0.05).at_least(0.01);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text("How many steps in the depth image correspond to one world-space unit. For instance, 1000 means millimeters.\n\
                    Double-click to reset.");
    if response.double_clicked() {
        // reset to auto - the exact value will be restored somewhere else
        *property = EditableAutoValue::Auto(value);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}

fn backproject_radius_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject radius scale");
    let mut value = *property.get();
    let speed = (value * 0.01).at_least(0.001);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text(
            "Scales the radii of the points in the backprojected point cloud.\n\
            This is a factor of the projected pixel diameter. \
            This means a scale of 0.5 will leave adjacent pixels at the same depth value just touching.\n\
            Double-click to reset.",
        );
    if response.double_clicked() {
        *property = EditableAutoValue::Auto(2.0);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}
