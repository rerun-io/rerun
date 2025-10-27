use egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use egui_kittest::kittest::Queryable as _;

pub struct ViewerSection<'a, 'h> {
    pub harness: &'a mut egui_kittest::Harness<'h, re_viewer::App>,
    pub section_label: &'a str,
}

impl<'a, 'h: 'a> ViewerSection<'a, 'h> {
    pub fn root(&'a self) -> egui_kittest::Node<'a> {
        let node = self
            .harness
            .get_by_role_and_label(Role::Pane, self.section_label);
        node
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
