use egui::{AtomLayout, Atoms, IntoAtoms, OpenUrl, RichText, TextStyle, Ui};

use crate::{UiExt as _, icons};

/// A help popup where you can show markdown text and controls as a table.
#[derive(Debug, Clone)]
pub struct Help {
    title: Option<String>,
    docs_link: Option<String>,
    sections: Vec<HelpSection>,
}

/// A single section, separated by a [`egui::Separator`].
#[derive(Debug, Clone)]
enum HelpSection {
    Markdown(String),
    Controls(Vec<ControlRow>),
}

/// A single row in the controls table.
#[derive(Debug, Clone)]
pub struct ControlRow {
    text: String,
    items: Atoms<'static>,
}

impl ControlRow {
    /// Create a new control row.
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(text: impl ToString, items: Atoms<'static>) -> Self {
        Self {
            text: text.to_string(),
            items,
        }
    }
}

impl Help {
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Create a new help popup.
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(title: impl ToString) -> Self {
        Self {
            title: Some(title.to_string()),
            docs_link: None,
            sections: Vec::new(),
        }
    }

    /// Create a new help popup.
    #[expect(clippy::needless_pass_by_value)]
    pub fn new_without_title() -> Self {
        Self {
            title: None,
            docs_link: None,
            sections: Vec::new(),
        }
    }

    /// Add a docs link, to be shown in the top right corner.
    #[expect(clippy::needless_pass_by_value)]
    #[inline]
    pub fn docs_link(mut self, docs_link: impl ToString) -> Self {
        self.docs_link = Some(docs_link.to_string());
        self
    }

    /// Add a markdown section.
    #[expect(clippy::needless_pass_by_value)]
    #[inline]
    pub fn markdown(mut self, markdown: impl ToString) -> Self {
        self.sections
            .push(HelpSection::Markdown(markdown.to_string()));
        self
    }

    /// Add a controls section.
    #[inline]
    pub fn controls(mut self, controls: Vec<ControlRow>) -> Self {
        self.sections.push(HelpSection::Controls(controls));
        self
    }

    /// Add a single control row to the last controls section.
    ///
    /// Split any + or / into an extra `IconTextItem`, like this:
    /// ```rust
    /// re_ui::Help::new("Example").control("Pan", ("click", "+", "drag"));
    /// ```
    #[expect(clippy::needless_pass_by_value)]
    #[inline]
    pub fn control(mut self, label: impl ToString, items: impl IntoAtoms<'static>) -> Self {
        if let Some(HelpSection::Controls(controls)) = self.sections.last_mut() {
            controls.push(ControlRow::new(label, items.into_atoms()));
        } else {
            self.sections
                .push(HelpSection::Controls(vec![ControlRow::new(
                    label,
                    items.into_atoms(),
                )]));
        }
        self
    }

    /// Create a new empty control section.
    #[inline]
    pub fn control_separator(mut self) -> Self {
        self.sections.push(HelpSection::Controls(vec![]));
        self
    }

    fn separator(ui: &mut Ui) {
        ui.scope(|ui| {
            ui.style_mut()
                .visuals
                .widgets
                .noninteractive
                .bg_stroke
                .color = ui.visuals().weak_text_color();
            ui.separator();
        });
    }

    /// Show the help popup. Usually you want to show this in [`egui::Response::on_hover_ui`].
    pub fn ui(self, ui: &mut Ui) {
        let Self {
            title,
            docs_link,
            sections,
        } = self;

        let show_heading = title.is_some() || docs_link.is_some();

        if show_heading {
            egui::Sides::new().show(
                ui,
                |ui| {
                    if let Some(title) = title {
                        ui.label(RichText::new(&title).strong().size(11.0));
                    }
                },
                |ui| {
                    if let Some(docs_link) = &docs_link {
                        let response = ui.add(
                            egui::Button::image_and_text(
                                &icons::EXTERNAL_LINK,
                                RichText::new("Docs").size(11.0),
                            )
                            .image_tint_follows_text_color(true)
                            .frame(false),
                        );

                        if response.clicked() {
                            ui.ctx().open_url(OpenUrl::new_tab(docs_link));
                        }
                    }
                },
            );
        }

        for (i, section) in sections.into_iter().enumerate() {
            if show_heading || 0 < i {
                Self::separator(ui);
            }

            section_ui(ui, section);
        }
    }
}

fn section_ui(ui: &mut Ui, section: HelpSection) {
    let tokens = ui.tokens();

    match section {
        HelpSection::Markdown(md) => {
            ui.markdown_ui(&md);
        }
        HelpSection::Controls(controls) => {
            for mut row in controls {
                egui::Sides::new().spacing(12.0).show(
                    ui,
                    |ui| {
                        ui.strong(RichText::new(&row.text).size(11.0));
                    },
                    |ui| {
                        let color = ui.visuals().widgets.inactive.text_color();
                        ui.set_height(tokens.small_icon_size.y);
                        ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                        ui.visuals_mut().override_text_color = Some(color);
                        row.items.map_images(|i| i.tint(color));
                        AtomLayout::new(row.items).gap(2.0).show(ui);
                    },
                );
            }
        }
    }
}
