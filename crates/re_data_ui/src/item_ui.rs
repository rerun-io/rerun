//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use re_data_store::InstancePath;
use re_log_types::{ComponentPath, EntityPath, TimeInt, Timeline};
use re_viewer_context::{
    DataBlueprintGroupHandle, HoverHighlight, Item, SelectionState, SpaceViewId, UiVerbosity,
    ViewerContext,
};

use super::DataUi;

// TODO(andreas): This is where we want to go, but we need to figure out how get the `SpaceViewCategory` from the `SpaceViewId`.
// Simply pass in optional icons?
//
// Show a button to an [`Item`] with a given text.
// pub fn item_button_to(
//     ctx: &mut ViewerContext<'_>,
//     ui: &mut egui::Ui,
//     item: &Item,
//     text: impl Into<egui::WidgetText>,
// ) -> egui::Response {
//     match item {
//         Item::ComponentPath(component_path) => {
//             component_path_button_to(ctx, ui, text, component_path)
//         }
//         Item::SpaceView(space_view_id) => {
//             space_view_button_to(ctx, ui, text, *space_view_id, space_view_category)
//         }
//         Item::InstancePath(space_view_id, instance_path) => {
//             instance_path_button_to(ctx, ui, *space_view_id, instance_path, text)
//         }
//         Item::DataBlueprintGroup(space_view_id, group_handle) => {
//             data_blueprint_group_button_to(ctx, ui, text, *space_view_id, *group_handle)
//         }
//     }
// }

/// Show an entity path and make it selectable.
pub fn entity_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        entity_path.to_string(),
    )
}

/// Show an entity path and make it selectable.
pub fn entity_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        text,
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        instance_path,
        instance_path.to_string(),
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let item = Item::InstancePath(space_view_id, instance_path.clone());
    let subtype_string = if instance_path.instance_key.is_splat() {
        "Entity"
    } else {
        "Entity Instance"
    };

    let response = ui
        .selectable_label(ctx.selection().contains(&item), text)
        .on_hover_ui(|ui| {
            ui.strong(subtype_string);
            ui.label(format!("Path: {instance_path}"));
            instance_path.data_ui(ctx, ui, UiVerbosity::Reduced, &ctx.current_query());
        });

    cursor_interact_with_selectable(ctx.selection_state_mut(), response, item)
}

/// Show a component path and make it selectable.
pub fn component_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    component_path: &ComponentPath,
) -> egui::Response {
    component_path_button_to(
        ctx,
        ui,
        component_path.component_name.short_name(),
        component_path,
    )
}

/// Show a component path and make it selectable.
pub fn component_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    component_path: &ComponentPath,
) -> egui::Response {
    let item = Item::ComponentPath(component_path.clone());
    let response = ui.selectable_label(ctx.selection().contains(&item), text);
    cursor_interact_with_selectable(ctx.selection_state_mut(), response, item)
}

pub fn data_blueprint_group_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    space_view_id: SpaceViewId,
    group_handle: DataBlueprintGroupHandle,
) -> egui::Response {
    let item = Item::DataBlueprintGroup(space_view_id, group_handle);
    let response = ctx
        .re_ui
        .selectable_label_with_icon(
            ui,
            &re_ui::icons::CONTAINER,
            text,
            ctx.selection().contains(&item),
        )
        .on_hover_text("Group");
    cursor_interact_with_selectable(ctx.selection_state_mut(), response, item)
}

pub fn data_blueprint_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    space_view_id: SpaceViewId,
    entity_path: &EntityPath,
) -> egui::Response {
    let item = Item::InstancePath(
        Some(space_view_id),
        InstancePath::entity_splat(entity_path.clone()),
    );
    let response = ui
        .selectable_label(ctx.selection().contains(&item), text)
        .on_hover_ui(|ui| {
            ui.strong("Space View Entity");
            ui.label(format!("Path: {entity_path}"));
            entity_path.data_ui(ctx, ui, UiVerbosity::Reduced, &ctx.current_query());
        });
    cursor_interact_with_selectable(ctx.selection_state_mut(), response, item)
}

pub fn time_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
    value: TimeInt,
) -> egui::Response {
    let is_selected = ctx.rec_cfg.time_ctrl.is_time_selected(timeline, value);

    let response = ui.selectable_label(is_selected, timeline.typ().format(value));
    if response.clicked() {
        ctx.rec_cfg
            .time_ctrl
            .set_timeline_and_time(*timeline, value);
        ctx.rec_cfg.time_ctrl.pause();
    }
    response
}

pub fn timeline_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
) -> egui::Response {
    timeline_button_to(ctx, ui, timeline.name().to_string(), timeline)
}

pub fn timeline_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    timeline: &Timeline,
) -> egui::Response {
    let is_selected = ctx.rec_cfg.time_ctrl.timeline() == timeline;

    let response = ui
        .selectable_label(is_selected, text)
        .on_hover_text("Click to switch to this timeline");
    if response.clicked() {
        ctx.rec_cfg.time_ctrl.set_timeline(*timeline);
        ctx.rec_cfg.time_ctrl.pause();
    }
    response
}

// TODO(andreas): Move elsewhere, this is not directly part of the item_ui.
pub fn cursor_interact_with_selectable(
    selection_state: &mut SelectionState,
    response: egui::Response,
    item: Item,
) -> egui::Response {
    let is_item_hovered =
        selection_state.highlight_for_ui_element(&item) == HoverHighlight::Hovered;

    select_hovered_on_click(&response, selection_state, &[item]);
    // TODO(andreas): How to deal with shift click for selecting ranges?

    if is_item_hovered {
        response.highlight()
    } else {
        response
    }
}

// TODO(andreas): Move elsewhere, this is not directly part of the item_ui.
pub fn select_hovered_on_click(
    response: &egui::Response,
    selection_state: &mut SelectionState,
    items: &[Item],
) {
    if response.hovered() {
        selection_state.set_hovered(items.iter().cloned());
    }

    if response.clicked() {
        if response.ctx.input(|i| i.modifiers.command) {
            selection_state.toggle_selection(selection_state.hovered().to_vec());
        } else {
            selection_state.set_multi_selection(selection_state.hovered().clone().into_iter());
        }
    }
}
