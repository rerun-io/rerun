//! The heading of each item in the selection panel.
//!
//! It shows "bread-crumbs" of the hierarchy of the item, wrapping to the next
//! line when the path is too long:
//!
//! A > B > C > D > item
//!
//! Each bread-crumb is a button — an icon, a name, or both — and each is
//! clickable, the last one included. Only the last one is shown as selected.
//! The `>` between two crumbs is a chevron that opens the children of the crumb
//! before it, so the user can walk the tree from here.
//!
//! The bread crumbs hierarchy should be identical to the hierarchy in the
//! either the blueprint tree panel, or the streams/time panel.

use egui::Color32;
use re_chunk::EntityPath;
use re_data_ui::item_ui::{cursor_interact_with_selectable, guess_instance_path_icon};
use re_entity_db::InstancePath;
use re_log_types::EntityPathPart;
use re_ui::{ReButton, Size, UiExt as _, icons, list_item, menu::menu_style};
use re_viewer_context::{ContainerId, Contents, Item, ViewId, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::item_title::ItemTitle;

/// The popup grows with its content (e.g. when the user expands a subtree),
/// up to this size — beyond it, the content scrolls.
const NAV_POPUP_MAX_SIZE: egui::Vec2 = egui::Vec2::new(400.0, 600.0);

/// Every crumb, the selected one included, shares one button size: 20 px tall, 4 px side padding.
const CRUMB_SIZE: Size = Size::Tiny;

/// Width of every navigation chevron: both the `>` between two crumbs,
/// and the hover-only one after the last crumb.
const NAV_CHEVRON_WIDTH: f32 = 12.0;

// Show the bread crumbs leading to (but not including) the final item.
fn item_bread_crumbs_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::RedapEntry { .. }
        | Item::RedapServer(_)
        | Item::TableId(_) => {
            // These have no bread crumbs, at least not currently.
            // I guess one could argue that the `StoreId` should have the `AppId` as its ancestor?
        }
        Item::InstancePath(instance_path) => {
            let InstancePath {
                entity_path,
                instance,
            } = instance_path;

            if instance.is_all() {
                // Entity path. Exclude the last part from the breadcrumbs,
                // as we will show it in full later on.
                if let [all_but_last @ .., _] = entity_path.as_slice() {
                    entity_path_breadcrumbs(ctx, ui, None, &EntityPath::root(), all_but_last, true);
                }
            } else {
                // Instance path.
                // Show the full entity path, and save the `[instance_nr]` for later.
                entity_path_breadcrumbs(
                    ctx,
                    ui,
                    None,
                    &EntityPath::root(),
                    entity_path.as_slice(),
                    true,
                );
            }
        }
        Item::ComponentPath(component_path) => {
            entity_path_breadcrumbs(
                ctx,
                ui,
                None,
                &EntityPath::root(),
                component_path.entity_path.as_slice(),
                true,
            );
        }
        Item::Container(container_id) => {
            if let Some(parent) = viewport.parent(&Contents::Container(*container_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::View(view_id) => {
            if let Some(parent) = viewport.parent(&Contents::View(*view_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::DataResult(data_result) => {
            viewport_breadcrumbs(ctx, viewport, ui, Contents::View(data_result.view_id));

            let InstancePath {
                entity_path,
                instance,
            } = &data_result.instance_path;

            if let Some(view) = viewport.view(&data_result.view_id) {
                let common_ancestor = data_result
                    .instance_path
                    .entity_path
                    .common_ancestor(&view.space_origin);

                let relative = &entity_path.as_slice()[common_ancestor.len()..];

                let is_projection = !entity_path.starts_with(&view.space_origin);
                // TODO(#10649): the projection breadcrumbs are wrong for nuscenes (but correct for arkit!),
                // at least if we consider the blueprint tree panel as "correct".
                // I fear we need to use the undocumented `DataResultNodeOrPath` and friends to match the
                // hierarchy of the blueprint tree panel.

                if instance.is_all() {
                    // Entity path. Exclude the last part from the breadcrumbs,
                    // as we will show it in full later on.
                    if let [all_but_last @ .., _] = relative {
                        entity_path_breadcrumbs(
                            ctx,
                            ui,
                            Some(data_result.view_id),
                            &common_ancestor,
                            all_but_last,
                            !is_projection,
                        );
                    }
                } else {
                    // Instance path.
                    // Show the full entity path, and save the `[instance_nr]` for later.
                    entity_path_breadcrumbs(
                        ctx,
                        ui,
                        Some(data_result.view_id),
                        &common_ancestor,
                        relative,
                        !is_projection,
                    );
                }
            }
        }
    }
}

/// Heading for entity path, component path, or data result items.
///
/// Shows breadcrumbs wrapping to the next line when the path is too long,
/// with only the last (selected) item shown as selected (blue).
///
/// For entity-path items (not component paths), a chevron is shown after the last crumb
/// on hover when the entity has children — clicking it opens a tree popup for navigation.
pub fn item_heading_with_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    re_tracing::profile_function!();

    let children = NavChildren::from_item(viewport, item).and_then(|it| it.non_empty(ctx));

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        // Every crumb brings its own padding (they are all `ReButton`s),
        // so we only set the gap between them.
        ui.spacing_mut().item_spacing.x = 2.0;
        // The chevron separators size themselves off `interact_size.y`.
        // 20 px per row, 2 px gap between wrapped rows.
        ui.spacing_mut().interact_size.y = CRUMB_SIZE.height();
        ui.spacing_mut().item_spacing.y = 2.0;

        item_bread_crumbs_ui(ctx, viewport, ui, item);
        last_crumb_ui(ctx, viewport, ui, item, children);
    });
    ui.add_space(4.0);
}

/// Returns the direct children of `entity_path`.
///
/// With a `view_id` they come from the view's `DataResultTree` (so entity-path
/// filters apply), without one straight from the recording's entity tree.
fn entity_children(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
    view_id: Option<ViewId>,
) -> Vec<EntityPath> {
    if let Some(view_id) = view_id {
        let query_result = ctx.lookup_query_result(view_id);
        let result_tree = &query_result.tree;
        let Some(node) = result_tree.lookup_node_by_path(entity_path.hash()) else {
            return vec![];
        };
        node.children
            .iter()
            .filter_map(|handle| result_tree.lookup_node(*handle))
            .map(|child_node| child_node.data_result.entity_path.clone())
            .collect()
    } else {
        let engine = ctx.recording_engine();
        let store = engine.store();
        let Some(subtree) = store.entity_tree().subtree(entity_path) else {
            return vec![];
        };
        subtree.children.values().map(|t| t.path.clone()).collect()
    }
}

/// Are there any [`entity_children`]?
///
/// Cheaper than building the list, which matters because every crumb asks this every frame.
fn has_entity_children(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
    view_id: Option<ViewId>,
) -> bool {
    if let Some(view_id) = view_id {
        ctx.lookup_query_result(view_id)
            .tree
            .lookup_node_by_path(entity_path.hash())
            .is_some_and(|node| !node.children.is_empty())
    } else {
        let engine = ctx.recording_engine();
        let store = engine.store();
        store
            .entity_tree()
            .subtree(entity_path)
            .is_some_and(|subtree| !subtree.children.is_empty())
    }
}

/// The views and containers inside a container.
fn container_contents<'a>(
    viewport: &'a ViewportBlueprint,
    container_id: &ContainerId,
) -> &'a [Contents] {
    viewport
        .container(container_id)
        .map_or(&[], |container| container.contents.as_slice())
}

/// Calls `visit` for each entity that appears as a direct child of a view in the
/// blueprint tree, until `visit` returns `false`.
///
/// This matches the blueprint tree's logic: the `space_origin` node of the
/// `DataResultTree` ("origin subtree"), plus any "projection" entities (entities in the
/// view but outside `space_origin` that are not tree-prefix-only placeholders).
fn visit_view_root_children(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    view_id: ViewId,
    visit: &mut dyn FnMut(&EntityPath) -> bool,
) {
    let Some(view) = viewport.view(&view_id) else {
        return;
    };
    let space_origin = &view.space_origin;
    let query_result = ctx.lookup_query_result(view_id);

    // Origin tree — the blueprint tree shows the space_origin entity itself as the
    // first child of the view, then its children underneath. We do the same.
    if query_result
        .tree
        .lookup_node_by_path(space_origin.hash())
        .is_some()
        && !visit(space_origin)
    {
        return;
    }

    // Projection roots — entities in the view but NOT under space_origin.
    // Matches the blueprint tree's visitor in data.rs.
    let mut keep_going = true;
    query_result.tree.visit(&mut |node| {
        let path = &node.data_result.entity_path;
        if !keep_going || path.starts_with(space_origin) {
            false // skip the entire origin subtree (already handled)
        } else if node.data_result.tree_prefix_only {
            true // intermediate ancestor — keep looking deeper
        } else {
            keep_going = visit(path);
            false // found a projection root — don't recurse further into it
        }
    });
}

/// The entities at the root of a view, sorted by entity path.
fn view_root_children(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    view_id: ViewId,
) -> Vec<EntityPath> {
    let mut children = Vec::new();
    visit_view_root_children(ctx, viewport, view_id, &mut |path| {
        children.push(path.clone());
        true
    });
    children.sort();
    children
}

/// The children of one crumb, as shown in its navigation popup.
///
/// This only says *where* the children are — they are looked up when the popup opens.
#[derive(Clone, Copy)]
enum NavChildren<'a> {
    /// Child entities. Inside a view they open as `Item::DataResult`,
    /// outside one as `Item::InstancePath`.
    Entity {
        entity_path: &'a EntityPath,
        view_id: Option<ViewId>,
    },

    /// The views and containers inside a container.
    Container {
        viewport: &'a ViewportBlueprint,
        container_id: ContainerId,
    },

    /// The entities at the root of a view.
    View {
        viewport: &'a ViewportBlueprint,
        view_id: ViewId,
    },
}

impl<'a> NavChildren<'a> {
    /// What the navigation chevron of `item`'s last crumb opens,
    /// or `None` if there is nothing to navigate into.
    ///
    /// Component paths and instances have no children, so they get no chevron.
    fn from_item(viewport: &'a ViewportBlueprint, item: &'a Item) -> Option<Self> {
        match item {
            Item::InstancePath(instance_path) if instance_path.instance.is_all() => {
                Some(Self::Entity {
                    entity_path: &instance_path.entity_path,
                    view_id: None,
                })
            }

            Item::DataResult(data_result) if data_result.instance_path.is_all() => {
                Some(Self::Entity {
                    entity_path: &data_result.instance_path.entity_path,
                    view_id: Some(data_result.view_id),
                })
            }

            Item::View(view_id) => Some(Self::from_contents(viewport, &Contents::View(*view_id))),

            Item::Container(container_id) => Some(Self::from_contents(
                viewport,
                &Contents::Container(*container_id),
            )),

            _ => None,
        }
    }

    /// What the navigation popup of a viewport crumb shows.
    fn from_contents(viewport: &'a ViewportBlueprint, contents: &Contents) -> Self {
        match contents {
            Contents::Container(container_id) => Self::Container {
                viewport,
                container_id: *container_id,
            },
            Contents::View(view_id) => Self::View {
                viewport,
                view_id: *view_id,
            },
        }
    }

    /// `None` if there is nothing to navigate into.
    ///
    /// Every crumb asks this every frame, so it stops at the first child
    /// instead of building the list.
    fn non_empty(self, ctx: &ViewerContext<'_>) -> Option<Self> {
        let any = match self {
            Self::Entity {
                entity_path,
                view_id,
            } => has_entity_children(ctx, entity_path, view_id),

            Self::Container {
                viewport,
                container_id,
            } => !container_contents(viewport, &container_id).is_empty(),

            Self::View { viewport, view_id } => {
                let mut any = false;
                visit_view_root_children(ctx, viewport, view_id, &mut |_| {
                    any = true;
                    false // stop at the first one
                });
                any
            }
        };
        any.then_some(self)
    }
}

/// Shared contents of the child-navigation popups.
///
/// The caller must apply [`menu_style`] to the popup, so the frame and hover
/// highlights match the context menu.
fn nav_popup_contents_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    popup_id: egui::Id,
    children: NavChildren<'_>,
) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    // Make the list item hover highlight respect the Frames inner margin
    let full_span = ui.max_rect().x_range();

    // Allow popup content to grow past its initial size
    ui.set_max_size(NAV_POPUP_MAX_SIZE);

    ui.full_span_scope(full_span, |ui| {
        list_item::list_item_scope(ui, popup_id, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([true, true])
                .max_width(NAV_POPUP_MAX_SIZE.x)
                .max_height(NAV_POPUP_MAX_SIZE.y)
                .show(ui, |ui| {
                    ui.set_min_width(150.0);
                    nav_children_ui(ctx, ui, children);
                });
        });
    });
}

/// One row of a navigation popup.
///
/// Collapsible when the row has children, so the subtree can be shown in place.
/// Clicking the row selects `item` and closes the popup.
///
/// The rounded hover highlight matches the buttons of a context menu (see [`menu_style`]).
fn nav_popup_item_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    item: Item,
    item_title: &ItemTitle,
    children: NavChildren<'_>,
) {
    let content = item_title.to_label_content();

    let list_item = ui
        .list_item()
        .with_corner_radius(ui.tokens().small_corner_radius())
        .collapse_temporary(true);

    let response = if let Some(children) = children.non_empty(ctx) {
        let id = egui::Id::new(&item).with("nav_popup_list_item");
        list_item
            .show_hierarchical_with_children(ui, id, false, content, |ui| {
                nav_children_ui(ctx, ui, children);
            })
            .item_response
    } else {
        list_item.show_hierarchical(ui, content)
    };

    if response.clicked() {
        ui.close_kind(egui::UiKind::Popup);
    }
    cursor_interact_with_selectable(&ctx.app_ctx, response, item);
}

/// Shows the children of one crumb as rows of its navigation popup.
fn nav_children_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, children: NavChildren<'_>) {
    match children {
        NavChildren::Entity {
            entity_path,
            view_id,
        } => {
            for child_path in entity_children(ctx, entity_path, view_id) {
                entity_tree_item_ui(ctx, ui, &child_path, view_id);
            }
        }

        NavChildren::Container {
            viewport,
            container_id,
        } => {
            for contents in container_contents(viewport, &container_id) {
                nav_popup_item_ui(
                    ctx,
                    ui,
                    Item::from(*contents),
                    &ItemTitle::from_contents(ctx, viewport, contents),
                    NavChildren::from_contents(viewport, contents),
                );
            }
        }

        NavChildren::View { viewport, view_id } => {
            for child_path in view_root_children(ctx, viewport, view_id) {
                entity_tree_item_ui(ctx, ui, &child_path, Some(view_id));
            }
        }
    }
}

/// Renders one entity as a list item inside the child-navigation popup.
///
/// When `view_id` is `Some`, click creates `Item::DataResult` (stays in view context).
/// When `None`, click creates `Item::InstancePath` (streams context).
fn entity_tree_item_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    view_id: Option<ViewId>,
) {
    let instance_path = InstancePath::entity_all(entity_path.clone());
    let item_title = ItemTitle::from_instance_path(ctx, ui.style(), &instance_path);

    nav_popup_item_ui(
        ctx,
        ui,
        item_for_entity(view_id, instance_path),
        &item_title,
        NavChildren::Entity {
            entity_path,
            view_id,
        },
    );
}

/// The item to select for an entity, keeping the view context when there is one.
fn item_for_entity(view_id: Option<ViewId>, instance_path: InstancePath) -> Item {
    if let Some(view_id) = view_id {
        Item::DataResult(
            re_viewer_context::DataResultInteractionAddress::from_entity_path(
                view_id,
                instance_path.entity_path,
            ),
        )
    } else {
        Item::from(instance_path)
    }
}

/// The last crumb: the selected item itself, together with its navigation chevron.
///
/// The two move to the next row as one unit, and the label is cut with an
/// ellipsis only when it is longer than a whole row.
fn last_crumb_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
    children: Option<NavChildren<'_>>,
) {
    let row_width = ui.max_rect().width();
    let heading_hovered = ui.rect_contains_pointer(ui.response().rect);

    // The chevron must stay on the same row as the crumb, so keep room for it.
    let chevron_width = if children.is_some() {
        NAV_CHEVRON_WIDTH + ui.spacing().item_spacing.x
    } else {
        0.0
    };

    ui.wrap_unit(|ui| {
        // The crumb may take a whole row, but no more — past that it truncates.
        ui.set_max_width(row_width - chevron_width);

        selected_crumb_ui(ctx, viewport, ui, item);

        if children.is_some() {
            // Unlike the separators, this chevron only shows on hover.
            chevron_with_popup(ctx, ui, children, "last_crumb", heading_hovered);
        }
    });
}

/// The button of the selected item, with its tooltip.
///
/// The blue fill comes from the `selection` token group in the theme `.ron` files.
/// Its icon and text follow the same color, and hover and press feedback is a
/// color change only (no growth).
fn selected_crumb_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    let ItemTitle {
        icon,
        label,
        label_style: _,
        tooltip,
    } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

    // View, container and recording items show their type icon next to the name.
    let label = label.text().to_owned();
    let button = if matches!(item, Item::View(_) | Item::Container(_) | Item::StoreId(_)) {
        ReButton::new((
            icon.as_image()
                .fit_to_exact_size(ui.tokens().small_icon_size),
            label,
        ))
        .size(CRUMB_SIZE)
    } else {
        ReButton::new(label).size(CRUMB_SIZE)
    };

    // The caller limits the width to one row, so `truncate` only kicks in
    // for a label that is longer than a whole row.
    let mut response = ui.add(button.truncate().selected(true));

    // Entity and component items get a tooltip with a "Copy path" button.
    let copy: Option<(&str, String)> = match item {
        Item::ComponentPath(component_path) => {
            Some(("Copy component path", component_path.to_string()))
        }
        Item::InstancePath(instance_path) => Some(("Copy entity path", instance_path.to_string())),
        Item::DataResult(data_result) => {
            Some(("Copy entity path", data_result.instance_path.to_string()))
        }
        _ => None,
    };
    if let Some((copy_label, path)) = copy {
        response = response.on_hover_ui(|ui| {
            copy_path_button_ui(ui, copy_label, &path);
        });
    } else if let Some(tooltip) = tooltip {
        response = response.on_hover_ui(|ui| {
            ui.label(tooltip);
        });
    }
    cursor_interact_with_selectable(&ctx.app_ctx, response, item.clone());
}

/// The breadcrumbs of containers and views in the viewport.
fn viewport_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    contents: Contents,
) {
    let item = Item::from(contents);

    if let Some(parent) = viewport.parent(&contents) {
        // Recurse!
        viewport_breadcrumbs(ctx, viewport, ui, parent.into());
    }

    let ItemTitle {
        icon,
        label: _,       // ignored: we just show the icon for breadcrumbs
        label_style: _, // no label
        tooltip,
    } = ItemTitle::from_contents(ctx, viewport, &contents);

    let mut response = ui.add(ReButton::icon(*icon).size(CRUMB_SIZE));
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    chevron_with_popup(
        ctx,
        ui,
        NavChildren::from_contents(viewport, &contents).non_empty(ctx),
        &item,
        true,
    );

    cursor_interact_with_selectable(&ctx.app_ctx, response, item);
}

/// A chevron that opens the children of the crumb before it in a popup.
///
/// Used both as the `>` separator between crumbs, and as the navigation chevron
/// after the last crumb.
fn chevron_with_popup(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    children: Option<NavChildren<'_>>,
    popup_seed: impl std::hash::Hash + std::fmt::Debug,
    visible: bool,
) {
    // Seed with `ui.id()` so the same seed (e.g. the same entity selected twice
    // in a multi-selection) still gets a unique popup per heading section.
    let popup_id = ui.id().with("nav_popup").with(popup_seed);

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(NAV_CHEVRON_WIDTH, ui.spacing().interact_size.y),
        if children.is_some() {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let interactive = visible && children.is_some() && response.hovered();

    let tint = if !visible {
        Color32::TRANSPARENT
    } else if interactive {
        ui.visuals().widgets.active.fg_stroke.color
    } else {
        ui.visuals().widgets.noninteractive.fg_stroke.color
    };

    // `0.0` openness: the chevron always points right, it never rotates open.
    ui.paint_collapsing_triangle(0.0, rect.center(), tint);

    if interactive {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if let Some(children) = children {
        egui::Popup::from_toggle_button_response(&response)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .style(menu_style())
            .show(|ui| {
                nav_popup_contents_ui(ctx, ui, popup_id, children);
            });
    }
}

/// A tooltip button that copies a path to the clipboard.
///
/// Shows `<copy icon> label` by default. After clicking, switches to `<checkmark> Copied!`
/// for 1.5 seconds, then reverts. The tooltip stays open while the cursor is inside it
/// (via `selectable_labels = true`) and closes when the cursor moves away.
fn copy_path_button_ui(ui: &mut egui::Ui, label: &str, path: &str) {
    // Keep the tooltip sticky so the user can move into it and click.
    ui.style_mut().interaction.selectable_labels = true;

    let copied_at_id = ui.id().with("copied_at");
    let now = ui.input(|i| i.time);
    let copied_at: Option<f64> = ui.ctx().data(|d| d.get_temp(copied_at_id));
    let show_copied = copied_at.is_some_and(|t| now - t < 1.5);

    if show_copied {
        let success = ui.tokens().success_text_color;
        ui.horizontal(|ui| {
            ui.add(
                icons::CHECKED
                    .as_image()
                    .max_size(ui.tokens().small_icon_size)
                    .tint(success),
            );
            ui.colored_label(success, "Copied!");
        });
        // Keep repainting so the 1.5 s timer can expire and revert to the button.
        ui.ctx().request_repaint();
    } else {
        let btn = ui.horizontal(|ui| {
            ui.add(
                icons::COPY
                    .as_image()
                    .max_size(ui.tokens().small_icon_size)
                    .tint(ui.tokens().text_default),
            );
            ui.colored_label(ui.tokens().text_default, label)
        });
        // Make the whole row clickable.
        let response = ui.interact(
            btn.response.rect,
            copied_at_id.with("btn"),
            egui::Sense::click(),
        );
        if response.clicked() {
            ui.copy_text(path.to_owned());
            ui.ctx().data_mut(|d| d.insert_temp(copied_at_id, now));
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

/// The breadcrumbs of an entity path,
/// that may or may not be part of a view.
fn entity_path_breadcrumbs(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    // If we are in a view
    view_id: Option<ViewId>,
    // Everything is relative to this
    origin: &EntityPath,
    // Show crumbs for all of these
    entity_parts: &[EntityPathPart],
    include_root: bool,
) {
    if let [ancestry @ .., _] = entity_parts {
        // Recurse!

        if !ancestry.is_empty() || include_root {
            entity_path_breadcrumbs(ctx, ui, view_id, origin, ancestry, include_root);
        }
    }

    let full_entity_path = origin.join(&EntityPath::new(entity_parts.to_vec()));

    let button = if let Some(last) = full_entity_path.last() {
        let atoms = last.unescaped_str().to_owned();
        ReButton::new(atoms).size(CRUMB_SIZE)
    } else {
        // Root
        let icon = if view_id.is_some() {
            // Inside a view, we show the root with an icon
            // that matches the one in the blueprint tree panel.
            guess_instance_path_icon(ctx, &InstancePath::from(full_entity_path.clone()))
        } else {
            // For a streams hierarchy, we show the root using a different icon,
            // just to make it clear that this is a different kind of hierarchy.
            &icons::RECORDING // streams hierarchy
        };
        ReButton::icon(*icon).size(CRUMB_SIZE)
    };

    // No tooltip on breadcrumb segments — only the last crumb gets the copy tooltip.
    let response = ui.add(button);

    let item = item_for_entity(view_id, InstancePath::entity_all(full_entity_path.clone()));
    cursor_interact_with_selectable(&ctx.app_ctx, response, item);

    chevron_with_popup(
        ctx,
        ui,
        NavChildren::Entity {
            entity_path: &full_entity_path,
            view_id,
        }
        .non_empty(ctx),
        &full_entity_path,
        true,
    );
}
