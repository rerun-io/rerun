struct Layout {
    nodes: HashMap<NodeIndex, egui::Rect>,
}

trait Drawable {
    // Decorations don't influence the extent of an object an are not considered during a measurement path.
    type Decoration;

    fn draw(&self, ui: &mut egui::Ui, decorations: Self::Decoration) -> egui::Response;
}

impl Layout {
    fn update(&mut self, nodes: impl Iterator<Item = (NodeIndex, )>) {
        todo!();
        // check if nodes have changed
        //   * added node indexes
        //   * removed node indexes
        //   * current nodes -> need to remeasure first.

    }

    fn extent(&self, NodeIndex)
}
