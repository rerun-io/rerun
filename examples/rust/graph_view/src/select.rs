use re_viewer::external::egui::{self, ahash::HashMap};

use crate::types::NodeIndex;

pub struct Layout {
    nodes: HashMap<NodeIndex, egui::Rect>,
}

// TODO(grtlr): For now we use enumerate to get slight disturbances, in the future we should use a proper random distribution.
#[deprecated]
fn rect_from_index(i: usize) -> egui::Rect {
    egui::Rect::from_center_size(egui::Pos2::new(0.0*i as f32, 0.0*i as f32), egui::Vec2::ZERO)
}

impl Layout {
    pub fn select(&mut self, nodes: impl IntoIterator<Item = NodeIndex>) {
        self.nodes = nodes.into_iter().enumerate().map(|(i,incoming)| {
            match self.nodes.get_mut(&incoming) {
                Some(rect) => (incoming, *rect),
                None => (incoming, rect_from_index(i)),
            }
        }).collect();
    }

    pub fn extent(&self, ix: &NodeIndex) -> Option<&egui::Rect> {
        self.nodes.get(ix)
    }

    pub fn update(&mut self, ix: NodeIndex, extent: egui::Rect) -> Option<egui::Rect> {
        self.nodes.insert(ix, extent)
    }
}

trait Drawable {
    // Decorations don't influence the extent of an object an are not considered during a measurement path.
    type Decoration;

    fn draw(&self, ui: &mut egui::Ui, decorations: Self::Decoration) -> egui::Response;
}

impl Layout {
    // fn update(&mut self, nodes: impl Iterator<Item = (NodeIndex, )>) {
    //     todo!();
    //     // check if nodes have changed
    //     //   * added node indexes
    //     //   * removed node indexes
    //     //   * current nodes -> need to remeasure first.

    // }


}
