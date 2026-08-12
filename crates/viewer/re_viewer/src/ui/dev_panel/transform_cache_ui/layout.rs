use ahash::{HashMap, HashSet};
use re_sdk_types::TransformFrameIdHash;

use super::{LayoutDirection, Model};

/// Simple tidy-tree style layout for the transform-cache graph.
///
/// The recursive pass places leaves on a monotonically increasing sibling axis and centers each
/// parent over the span of its laid-out children.
///
/// This uses the core idea from Reingold and Tilford's
/// [Tidier Drawings of Trees](https://doi.org/10.1109/TSE.1981.234519), but deliberately omits
/// contour-based subtree compaction. Fixed spacing keeps node labels and right-angled edge routing
/// predictable for this UI.
///
/// Layout is computed in orientation-independent coordinates:
///
/// * `depth` is the axis that moves away from the root through child transforms.
/// * `cross` is the sibling axis, perpendicular to `depth`, used to spread leaves and disjoint
///   trees.
pub(super) struct Layout<'a> {
    model: &'a Model,
    pub(super) direction: LayoutDirection,
    pub(super) positions: HashMap<TransformFrameIdHash, egui::Pos2>,
    visited: HashSet<TransformFrameIdHash>,

    // Next available coordinate on the `cross` axis for a leaf or disconnected tree.
    next_cross: f32,

    // Total offset between adjacent node slots, including the node size.
    node_offset: egui::Vec2,

    pub(super) margin: f32,
}

impl<'a> Layout<'a> {
    const START_CROSS_OFFSET: f32 = 30.0;
    const MARGIN: f32 = 30.0;

    /// Computes screen-space node positions for the current filtered transform-cache model.
    ///
    /// The same algorithm is used for horizontal and vertical output; only the final mapping of
    /// depth/cross coordinates to x/y changes.
    pub(super) fn compute(
        model: &'a Model,
        direction: LayoutDirection,
        node_size: egui::Vec2,
    ) -> Self {
        let mut layout = Self::new(model, direction, node_size);

        // Start with true roots so disconnected components are laid out independently.
        let mut roots = model
            .snapshot
            .frames
            .iter()
            .filter(|node| !model.edge_indices_by_child.contains_key(&node.id))
            .map(|node| node.id)
            .collect::<Vec<_>>();
        roots.sort_by_key(|id| model.sort_key(*id));

        for root in roots {
            layout.layout_tree(root, 0);
            layout.finish_tree();
        }

        // Any nodes not reached from a root are still useful to show: they are disconnected or cyclic.
        // Treat each as a separate tree instead of hiding broken transform data.
        for node in &model.snapshot.frames {
            if !layout.visited.contains(&node.id) {
                layout.layout_tree(node.id, 0);
                layout.finish_tree();
            }
        }

        layout
    }

    /// Returns the full scene-space bounds needed to contain every laid-out node.
    pub(super) fn content_rect(&self, node_size: egui::Vec2) -> egui::Rect {
        let max_x = self.positions.values().map(|pos| pos.x).fold(0.0, f32::max)
            + node_size.x
            + self.margin;
        let max_y = self.positions.values().map(|pos| pos.y).fold(0.0, f32::max)
            + node_size.y
            + self.margin;
        egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(max_x, max_y).max(egui::vec2(1.0, 1.0)),
        )
        .expand(self.margin)
    }

    /// Returns the midpoint between a parent node and child node on the depth axis.
    ///
    /// Shared fork edges use this coordinate for the bus line that fans out to siblings.
    pub(super) fn fork_depth_coordinate(
        &self,
        parent_pos: egui::Pos2,
        child_pos: egui::Pos2,
        node_size: egui::Vec2,
    ) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => (parent_pos.x + node_size.x + child_pos.x) / 2.0,
            LayoutDirection::Vertical => (parent_pos.y + node_size.y + child_pos.y) / 2.0,
        }
    }

    /// Creates an empty layout accumulator with spacing tuned for the requested direction.
    fn new(model: &'a Model, direction: LayoutDirection, node_size: egui::Vec2) -> Self {
        Self {
            model,
            direction,
            positions: Default::default(),
            visited: Default::default(),
            next_cross: Self::START_CROSS_OFFSET,
            node_offset: direction.node_offset(node_size),
            margin: Self::MARGIN,
        }
    }

    /// Recursively lays out one tree and returns the node's coordinate on the sibling axis.
    fn layout_tree(&mut self, frame: TransformFrameIdHash, depth: usize) -> f32 {
        if !self.visited.insert(frame) {
            // Cycles should not happen for a valid transform graph, but the view should stay
            // responsive if bad data sneaks in.
            return self
                .positions
                .get(&frame)
                .map_or(self.next_cross, |pos| self.cross_coordinate(*pos));
        }

        let children = self
            .model
            .edge_indices_by_parent
            .get(&frame)
            .cloned()
            .unwrap_or_default();
        let child_cross_coordinates = children
            .into_iter()
            .map(|edge_index| {
                self.layout_tree(self.model.snapshot.edges[edge_index].child, depth + 1)
            })
            .collect::<Vec<_>>();

        let cross = if child_cross_coordinates.is_empty() {
            // Leaves claim the next available slot on the sibling axis.
            let cross = self.next_cross;
            self.next_cross += self.cross_spacing();
            cross
        } else {
            // Parent nodes are centered over the span occupied by their visible children.
            child_cross_coordinates
                .first()
                .copied()
                .unwrap_or(self.next_cross)
                .midpoint(
                    child_cross_coordinates
                        .last()
                        .copied()
                        .unwrap_or(self.next_cross),
                )
        };
        self.positions
            .insert(frame, self.node_position(depth, cross));
        cross
    }

    /// Adds spacing between independent components in the same model.
    fn finish_tree(&mut self) {
        self.next_cross += self.cross_spacing();
    }

    /// Converts algorithm coordinates (`depth`, `cross`) into scene-space node positions.
    fn node_position(&self, depth: usize, cross: f32) -> egui::Pos2 {
        let depth_coordinate = self.margin + depth as f32 * self.depth_spacing();
        // The tree algorithm is orientation-independent: `depth` flows away from the root,
        // while `cross` spreads siblings. The final position maps those axes to x/y.
        match self.direction {
            LayoutDirection::Horizontal => egui::pos2(depth_coordinate, cross),
            LayoutDirection::Vertical => egui::pos2(cross, depth_coordinate),
        }
    }

    /// Extracts the sibling-axis coordinate from a scene-space position.
    fn cross_coordinate(&self, pos: egui::Pos2) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => pos.y,
            LayoutDirection::Vertical => pos.x,
        }
    }

    fn cross_spacing(&self) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => self.node_offset.y,
            LayoutDirection::Vertical => self.node_offset.x,
        }
    }

    fn depth_spacing(&self) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => self.node_offset.x,
            LayoutDirection::Vertical => self.node_offset.y,
        }
    }
}

impl LayoutDirection {
    /// Total offset between nodes in one tree step, including the node size.
    fn node_offset(self, node_size: egui::Vec2) -> egui::Vec2 {
        match self {
            Self::Horizontal => node_size + egui::vec2(65.0, 20.0),
            Self::Vertical => node_size + egui::vec2(50.0, 60.0),
        }
    }
}
