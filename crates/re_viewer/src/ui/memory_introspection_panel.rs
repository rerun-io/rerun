use re_memory::TextTree as _;

#[derive(Default)]
pub struct MemoryIntrospectionPanel {
    global: re_memory::Global,
    node: re_memory::Node,
    summary: String,
}

impl MemoryIntrospectionPanel {
    /// Returns `true` if we need to update the contents before next frame.
    #[must_use]
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        crate::profile_function!();

        ui.heading("Memory profile");

        let mut recalculate = self.summary.is_empty();
        recalculate |= ui.button("New snapshot").clicked();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::Label::new(self.summary.clone()).wrap(false));
        });

        recalculate
    }

    pub fn set_snapshot(&mut self, global: re_memory::Global, node: re_memory::Node) {
        crate::profile_function!();

        self.global = global;
        self.node = node;

        self.summary = format!("{}\n{}", self.global.text_tree(), self.node.text_tree());
    }
}
