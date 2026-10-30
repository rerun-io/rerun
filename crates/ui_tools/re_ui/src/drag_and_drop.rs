//! Helpers for drag and drop support for reordering hierarchical lists.
//!
//! Works well in combination with [`crate::list_item::ListItem`].

pub enum ItemKind<ItemId: Copy> {
    /// Root container item.
    ///
    /// Root container don't have a parent and are restricted when hovered as drop target (the dragged item can only go
    /// _in_ the root container, not before or after).
    RootContainer,

    /// Container item.
    Container {
        parent_id: ItemId,
        position_index_in_parent: usize,
    },

    /// Leaf item.
    Leaf {
        parent_id: ItemId,
        position_index_in_parent: usize,
    },
}

/// Context information about the hovered item.
///
/// This is used by [`find_drop_target`] to compute the [`DropTarget`], if any.
pub struct ItemContext<ItemId: Copy> {
    /// ID of the item being hovered during drag
    pub id: ItemId,

    /// What kind of item is this?
    pub item_kind: ItemKind<ItemId>,

    /// ID of the container just before this item within the parent, if such a container exists.
    pub previous_container_id: Option<ItemId>,
}

/// Drop target information, including where to draw the drop indicator and where to insert the dragged item.
#[derive(Clone, Debug)]
pub struct DropTarget<ItemId: Copy> {
    /// Range of X coordinates for the drag target indicator
    pub indicator_span_x: egui::Rangef,

    /// Y coordinate for drag target indicator
    pub indicator_position_y: f32,

    /// Destination container ID
    pub target_parent_id: ItemId,

    /// Destination position within the container
    pub target_position_index: usize,
}

impl<ItemId: Copy> DropTarget<ItemId> {
    pub fn new(
        indicator_span_x: egui::Rangef,
        indicator_position_y: f32,
        target_parent_id: ItemId,
        target_position_index: usize,
    ) -> Self {
        Self {
            indicator_span_x,
            indicator_position_y,
            target_parent_id,
            target_position_index,
        }
    }
}

/// Compute the geometry of the drag cursor and where the dragged item should be inserted.
///
/// This function performs the following tasks:
/// - based on `item_rect` and `body_rect`, establish the geometry of actual drop zones (see below)
/// - test the mouse cursor against these zones
/// - if one is a match:
///   - compute the geometry of a drop insertion indicator
///   - use the context provided in `item_context` to return the "logical" drop target (ie. the target container and
///     position within it)
///
/// This function implements the following logic:
/// ```text
///
///                        insert         insert last in container before me
///                      before me           (if any) or insert before me
///                          │                             │
///                      ╔═══▼═════════════════════════════▼══════════════════╗
///                      ║      │                                             ║
///         leaf item    ║ ─────┴──────────────────────────────────────────── ║
///                      ║                                                    ║
///                      ╚═════════════════════▲══════════════════════════════╝
///                                            │
///                                     insert after me
///
///
///                        insert         insert last in container before me
///                      before me           (if any) or insert before me
///                          │                             │
///                      ╔═══▼═════════════════════════════▼══════════════════╗
///         leaf item    ║      │                                             ║
///         with body    ║ ─────┴──────────────────────────────────────────── ║
///                      ║                                                    ║
///                      ╚══════╦══════════════════════════════════════▲══════╣ ─┐
///                      │      ║                                      │      ║  │
///                      │      ║                                   insert    ║  │
///                      │      ║                                  after me   ║  │
///                      │      ╠══                                         ══╣  │
///                      │      ║             no insertion possible           ║  │
///                      │      ║             here by definition of           ║  │ body
///                      │      ║              parent being a leaf            ║  │
///                      │      ╠══                                         ══╣  │
///                      │      ║                                             ║  │
///                      │      ║                                             ║  │
///                      │      ║                                             ║  │
///                      └──▲── ╚══════════════════════════▲══════════════════╝ ─┘
///                         │                              │
///                      insert                         insert
///                     after me                       after me
///
///
///                        insert         insert last in container before me
///                      before me           (if any) or insert before me
///                          │                             │
///                      ╔═══▼═════════════════════════════▼══════════════════╗
///    container item    ║      │                                             ║
///  (empty/collapsed    ║ ─────┼──────────────────────────────────────────── ║
///             body)    ║      │                                             ║
///                      ╚═══▲═════════════════════════════▲══════════════════╝
///                          │                             │
///                       insert                   insert inside me
///                      after me                     at pos = 0
///
///
///                        insert         insert last in container before me
///                      before me           (if any) or insert before me
///                          │                             │
///                      ╔═══▼═════════════════════════════▼══════════════════╗
///    container item    ║      │                                             ║
///         with body    ║ ─────┴──────────────────────────────────────────── ║
///                      ║                                                    ║
///                      ╚═▲════╦═════════════════════════════════════════════╣ ─┐
///                        │    ║                                             ║  │
///                     insert  ║                                             ║  │
///                  inside me  ║                                             ║  │
///                 at pos = 0  ╠══                                         ══╣  │
///                             ║                  same logic                 ║  │
///                             ║                 recursively                 ║  │ body
///                     insert  ║                 applied here                ║  │
///                   after me  ╠══                                         ══╣  │
///                        │    ║                                             ║  │
///                      ┌─▼─── ║                                             ║  │
///                      │      ║                                             ║  │
///                      └───── ╚═════════════════════════════════════════════╝ ─┘
///
///                                              not a valid drop zone
///                                                        │
///                      ╔═════════════════════════════════▼══════════════════╗
///    root container    ║                                                    ║
///              item    ║ ────────────────────────────────────────────────── ║
///                      ║                                                    ║
///                      ╚════════════════════════▲═══════════════════════════╝
///                                               │
///                                       insert inside me
///                                          at pos = 0
/// ```
///
/// Here are a few observations of the above that help navigate the "if-statement-of-death"
/// in the implementation:
/// - The top parts of the item are treated the same in most cases (root container is an
///   exception).
/// - Handling of the body can be simplified by making the sensitive area either a small
///   corner (container case), or the entire body (leaf case). Then, that area always maps
///   to "insert after me".
/// - The bottom parts have the most difference between cases and need case-by-case handling.
///   In both leaf item cases, the entire bottom part maps to "insert after me", though.
///
/// **Note**: in debug builds, press `Alt` to visualize the drag zones while dragging.
pub fn find_drop_target<ItemId: Copy>(
    ui: &egui::Ui,
    item_context: &ItemContext<ItemId>,
    item_rect: egui::Rect,
    body_rect: Option<egui::Rect>,
    item_height: f32,
) -> Option<DropTarget<ItemId>> {
    let indent = ui.spacing().indent;
    let item_id = item_context.id;
    let item_kind = &item_context.item_kind;
    let is_non_root_container = matches!(item_kind, ItemKind::Container { .. });

    // For both leaf and containers we have two drag zones on the upper half of the item.
    let (mut top, mut bottom) = item_rect.split_top_bottom_at_fraction(0.5);

    let mut left_top = egui::Rect::NOTHING;
    if matches!(item_kind, ItemKind::RootContainer) {
        top = egui::Rect::NOTHING;
    } else {
        (left_top, top) = top.split_left_right_at_x(top.left() + indent);
    }

    // For the lower part of the item, the story is more complicated:
    // - for leaf and root container items, we have a single drag zone on the entire lower half
    // - for container item, we must distinguish between the indent part and the rest, plus check some area in the
    //   body
    let mut left_bottom = egui::Rect::NOTHING;
    if is_non_root_container {
        (left_bottom, bottom) = bottom.split_left_right_at_x(bottom.left() + indent);
    }

    // For the body area we have two cases:
    // - container item: it's handled recursively by the nested items, so we only need to check a small area down
    //   left, which maps to "insert after me"
    // - leaf item: the entire body area, if any, cannot receive a drag (by definition) and thus homogeneously maps
    //   to "insert after me"
    let body_insert_after_me_area = if let Some(body_rect) = body_rect {
        if is_non_root_container {
            egui::Rect::from_two_pos(
                body_rect.left_bottom() + egui::vec2(indent, -item_height / 2.0),
                body_rect.left_bottom(),
            )
        } else {
            body_rect
        }
    } else {
        egui::Rect::NOTHING
    };

    // body rect, if any AND it actually contains something
    let non_empty_body_rect = body_rect.filter(|r| r.height() > 0.0);

    // visualize the drag zones in debug builds, when the `Alt` key is pressed during drag
    #[cfg(debug_assertions)]
    {
        // Visualize the drag zones
        if ui.input(|i| i.modifiers.alt) {
            ui.debug_painter().debug_rect(top, egui::Color32::RED, "t");
            ui.debug_painter()
                .debug_rect(bottom, egui::Color32::GREEN, "b");

            ui.debug_painter()
                .debug_rect(left_top, egui::Color32::RED.gamma_multiply(0.5), "lt");
            ui.debug_painter().debug_rect(
                left_bottom,
                egui::Color32::GREEN.gamma_multiply(0.5),
                "lb",
            );
            ui.debug_painter()
                .debug_rect(body_insert_after_me_area, egui::Color32::YELLOW, "bdy");
        }
    }

    match *item_kind {
        ItemKind::RootContainer => {
            // we just need to test the bottom section in this case.
            if ui.rect_contains_pointer(bottom) {
                Some(DropTarget::new(
                    item_rect.x_range(),
                    bottom.bottom(),
                    item_id,
                    0,
                ))
            } else {
                None
            }
        }

        ItemKind::Container {
            parent_id,
            position_index_in_parent,
        }
        | ItemKind::Leaf {
            parent_id,
            position_index_in_parent,
        } => {
            /* ===== TOP SECTIONS (same leaf/container items) ==== */
            if ui.rect_contains_pointer(left_top) {
                // insert before me
                Some(DropTarget::new(
                    item_rect.x_range(),
                    top.top(),
                    parent_id,
                    position_index_in_parent,
                ))
            } else if ui.rect_contains_pointer(top) {
                // insert last in the previous container if any, else insert before me
                if let Some(previous_container_id) = item_context.previous_container_id {
                    Some(DropTarget::new(
                        (item_rect.left() + indent..=item_rect.right()).into(),
                        top.top(),
                        previous_container_id,
                        usize::MAX,
                    ))
                } else {
                    Some(DropTarget::new(
                        item_rect.x_range(),
                        top.top(),
                        parent_id,
                        position_index_in_parent,
                    ))
                }
            }
            /* ==== BODY SENSE AREA ==== */
            else if ui.rect_contains_pointer(body_insert_after_me_area) {
                // insert after me in my parent
                Some(DropTarget::new(
                    item_rect.x_range(),
                    body_insert_after_me_area.bottom(),
                    parent_id,
                    position_index_in_parent + 1,
                ))
            }
            /* ==== BOTTOM SECTIONS (leaf item) ==== */
            else if !is_non_root_container {
                if ui.rect_contains_pointer(bottom) {
                    let position_y = if let Some(body_rect) = non_empty_body_rect {
                        body_rect.bottom()
                    } else {
                        bottom.bottom()
                    };

                    // insert after me
                    Some(DropTarget::new(
                        item_rect.x_range(),
                        position_y,
                        parent_id,
                        position_index_in_parent + 1,
                    ))
                } else {
                    None
                }
            }
            /* ==== BOTTOM SECTIONS (container item) ==== */
            else if let Some(body_rect) = non_empty_body_rect {
                if ui.rect_contains_pointer(left_bottom) || ui.rect_contains_pointer(bottom) {
                    // insert at pos = 0 inside me
                    Some(DropTarget::new(
                        (body_rect.left() + indent..=body_rect.right()).into(),
                        left_bottom.bottom(),
                        item_id,
                        0,
                    ))
                } else {
                    None
                }
            } else if ui.rect_contains_pointer(left_bottom) {
                // insert after me in my parent
                Some(DropTarget::new(
                    item_rect.x_range(),
                    left_bottom.bottom(),
                    parent_id,
                    position_index_in_parent + 1,
                ))
            } else if ui.rect_contains_pointer(bottom) {
                // insert at pos = 0 inside me
                Some(DropTarget::new(
                    (item_rect.left() + indent..=item_rect.right()).into(),
                    bottom.bottom(),
                    item_id,
                    0,
                ))
            }
            /* ==== Who knows where else the mouse cursor might wander… ¯\_(ツ)_/¯ ==== */
            else {
                None
            }
        }
    }
}
