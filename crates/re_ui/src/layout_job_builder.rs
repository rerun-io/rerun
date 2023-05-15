/// Utility for building layout jobs.
pub struct LayoutJobBuilder<'a> {
    pub layout_job: egui::text::LayoutJob,
    pub re_ui: &'a crate::ReUi,
}

impl<'a> LayoutJobBuilder<'a> {
    pub fn new(re_ui: &'a crate::ReUi) -> Self {
        Self {
            layout_job: egui::text::LayoutJob::default(),
            re_ui,
        }
    }

    /// Append a generic text block.
    pub fn add<'b, T: Into<LayoutJobBuilderBuildingBlock<'b>>>(&mut self, text_block: T) {
        let text_block: LayoutJobBuilderBuildingBlock<'_> = text_block.into();
        match text_block {
            LayoutJobBuilderBuildingBlock::Body(text) => self.add_body(text),
            LayoutJobBuilderBuildingBlock::Key(key) => self.add_key(key),
            LayoutJobBuilderBuildingBlock::Modifier(modifier) => self.add_modifier(modifier),
            LayoutJobBuilderBuildingBlock::MouseButton(button) => self.add_mouse_button(button),
        };
    }

    /// Append body text.
    pub fn add_body(&mut self, text: &str) {
        self.layout_job
            .append(text, 0.0, self.re_ui.text_format_body());
    }

    /// Append text that has special formatting for a button.
    pub fn add_button_text(&mut self, text: &str) {
        self.layout_job
            .append(&text.to_lowercase(), 0.0, self.re_ui.text_format_key());
    }

    /// Append text for a keyboard key.
    pub fn add_key(&mut self, key: egui::Key) {
        self.add_button_text(key.name());
    }

    /// Append text for one or more modifier keys.
    pub fn add_modifier(&mut self, modifier: egui::Modifiers) {
        let is_mac = matches!(
            self.re_ui.egui_ctx.os(),
            egui::os::OperatingSystem::Mac | egui::os::OperatingSystem::IOS
        );
        let text = egui::ModifierNames::NAMES.format(&modifier, is_mac);
        self.add_button_text(&text);
    }

    /// Append text for a mouse button.
    pub fn add_mouse_button(&mut self, button: egui::PointerButton) {
        self.add_button_text(match button {
            egui::PointerButton::Primary => "left mouse button",
            egui::PointerButton::Secondary => "right mouse button",
            egui::PointerButton::Middle => "middle mouse button",
            egui::PointerButton::Extra1 => "extra mouse button 1",
            egui::PointerButton::Extra2 => "extra mouse button 2",
        });
    }
}

/// Generic building block that the layout job builder can consume.
///
/// Not meant to be used directly, use [`LayoutJobBuilder::add`] instead.
pub enum LayoutJobBuilderBuildingBlock<'a> {
    Body(&'a str),
    Key(egui::Key),
    Modifier(egui::Modifiers),
    MouseButton(egui::PointerButton),
}

impl<'a> From<&'a str> for LayoutJobBuilderBuildingBlock<'a> {
    fn from(text: &'a str) -> Self {
        Self::Body(text)
    }
}

impl From<egui::Key> for LayoutJobBuilderBuildingBlock<'_> {
    fn from(key: egui::Key) -> Self {
        Self::Key(key)
    }
}

impl From<egui::Modifiers> for LayoutJobBuilderBuildingBlock<'_> {
    fn from(modifier: egui::Modifiers) -> Self {
        Self::Modifier(modifier)
    }
}

impl From<egui::PointerButton> for LayoutJobBuilderBuildingBlock<'_> {
    fn from(button: egui::PointerButton) -> Self {
        Self::MouseButton(button)
    }
}
