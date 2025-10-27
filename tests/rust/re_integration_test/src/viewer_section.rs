use egui::PointerButton;
use egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use egui_kittest::kittest::Queryable as _;

pub struct ViewerSection<'a, 'h> {
    pub harness: &'a mut egui_kittest::Harness<'h, re_viewer::App>,
    pub section_label: &'a str,
}

impl<'a, 'h: 'a> ViewerSection<'a, 'h> {
    pub fn root<'n>(&'n self) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        let node = self
            .harness
            .get_by_role_and_label(Role::Pane, self.section_label);
        node
    }

    pub fn get_label<'n>(&'n mut self, label: &'n str) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        self.root().get_by_label(label)
    }

    pub fn get_nth_label<'n>(&'n mut self, label: &'n str, index: usize) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        let mut nodes = self.root().get_all_by_label(label).collect::<Vec<_>>();
        assert!(
            index < nodes.len(),
            "Failed to find label '{label}' #{index}, there are only {} nodes:\n{nodes:#?}",
            nodes.len()
        );
        nodes.swap_remove(index)
    }

    fn get_nth_label_inner<'n>(
        &'n mut self,
        label: &'n str,
        index: Option<usize>,
    ) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        if let Some(index) = index {
            self.get_nth_label(label, index)
        } else {
            self.get_label(label)
        }
    }

    pub fn click_label(&mut self, label: &str) {
        self.root().get_by_label(label).click();
        self.harness.run_ok();
    }

    pub fn click_label_modifiers(&mut self, label: &str, modifiers: egui::Modifiers) {
        self.root().get_by_label(label).click_modifiers(modifiers);
        self.harness.run_ok();
    }

    pub fn right_click_label(&mut self, label: &str) {
        self.root().get_by_label(label).click_secondary();
        self.harness.run_ok();
    }

    fn drag_label_inner(&mut self, label: &str, index: Option<usize>) {
        let node = self.get_nth_label_inner(label, index);

        let center = node.rect().center();
        self.harness.event(egui::Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });

        // Step until the time has passed `max_click_duration` so this gets
        // registered as a drag.
        // self.harness.step();
        let wait_time = self
            .harness
            .ctx
            .options(|o| o.input_options.max_click_duration);
        let end_time = self.harness.ctx.input(|i| i.time + wait_time);
        while self.harness.ctx.input(|i| i.time) < end_time {
            self.harness.step();
        }
        // self.harness.step();
    }

    pub fn drag_nth_label(&mut self, label: &str, index: usize) {
        self.drag_label_inner(label, Some(index));
    }

    pub fn drag_label(&mut self, label: &str) {
        self.drag_label_inner(label, None);
    }

    pub fn drop_label_inner(&mut self, label: &str, index: Option<usize>) {
        let node = self.get_nth_label_inner(label, index);
        let event = egui::Event::PointerButton {
            pos: node.rect().center(),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        self.harness.event(event);
        self.harness.remove_cursor();
        self.harness.run_ok();
    }

    pub fn drop_nth_label(&mut self, label: &str, index: usize) {
        self.drop_label_inner(label, Some(index));
    }

    pub fn drop_label(&mut self, label: &str) {
        self.drop_label_inner(label, None);
    }

    pub fn hover_label(&mut self, label: &str) {
        self.get_label(label).hover();
        self.harness.run_ok();
    }

    pub fn hover_nth_label(&mut self, label: &str, index: usize) {
        self.get_nth_label(label, index).hover();
        self.harness.run_ok();
    }
}

pub trait GetSection<'h> {
    fn get_section<'a>(&'a mut self, section_label: &'a str) -> ViewerSection<'a, 'h>
    where
        'h: 'a;

    fn blueprint_tree<'a>(&'a mut self) -> ViewerSection<'a, 'h> {
        self.get_section("_blueprint_tree")
    }

    fn streams_tree<'a>(&'a mut self) -> ViewerSection<'a, 'h> {
        self.get_section("_streams_tree")
    }

    fn selection_panel<'a>(&'a mut self) -> ViewerSection<'a, 'h> {
        self.get_section("_selection_panel")
    }
}

impl<'h> GetSection<'h> for egui_kittest::Harness<'h, re_viewer::App> {
    fn get_section<'a>(&'a mut self, section_label: &'a str) -> ViewerSection<'a, 'h>
    where
        'h: 'a,
    {
        ViewerSection::<'a, 'h> {
            harness: self,
            section_label,
        }
    }
}
