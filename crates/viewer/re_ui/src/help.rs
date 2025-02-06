use crate::icon_text::{IconText, IconTextItem};
use crate::{design_tokens, icons, ColorToken, DesignTokens, Scale, UiExt};
use egui::{OpenUrl, RichText, Sense, TextBuffer, Ui, UiBuilder};

#[derive(Debug, Clone)]
pub struct Help<'a> {
    title: String,
    docs_link: Option<String>,
    sections: Vec<HelpSection<'a>>,
}

#[derive(Debug, Clone)]
enum HelpSection<'a> {
    Markdown(String),
    Controls(Vec<ControlRow<'a>>),
}

#[derive(Debug, Clone)]
pub struct ControlRow<'a> {
    text: String,
    items: IconText<'a>,
}

impl<'a> ControlRow<'a> {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(text: impl ToString, items: IconText<'a>) -> Self {
        Self {
            text: text.to_string(),
            items,
        }
    }
}

impl<'a> Help<'a> {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(title: impl ToString) -> Self {
        Self {
            title: title.to_string(),
            docs_link: None,
            sections: Vec::new(),
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    #[inline]
    pub fn docs_link(mut self, docs_link: impl ToString) -> Self {
        self.docs_link = Some(docs_link.to_string());
        self
    }

    #[allow(clippy::needless_pass_by_value)]
    #[inline]
    pub fn markdown(mut self, markdown: impl ToString) -> Self {
        self.sections
            .push(HelpSection::Markdown(markdown.to_string()));
        self
    }

    #[inline]
    pub fn controls(mut self, controls: Vec<ControlRow<'a>>) -> Self {
        self.sections.push(HelpSection::Controls(controls));
        self
    }

    #[allow(clippy::needless_pass_by_value)]
    #[inline]
    pub fn control(mut self, label: impl ToString, items: IconText<'a>) -> Self {
        if let Some(HelpSection::Controls(controls)) = self.sections.last_mut() {
            controls.push(ControlRow::new(label, items));
        } else {
            self.sections
                .push(HelpSection::Controls(vec![ControlRow::new(label, items)]));
        }
        self
    }

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
                .color = design_tokens().color_table.gray(Scale::S400);
            ui.separator();
        });
    }

    pub fn ui(&self, ui: &mut Ui) {
        egui::Sides::new().show(
            ui,
            |ui| {
                ui.label(RichText::new(&self.title).strong().size(11.0));
            },
            |ui| {
                if let Some(docs_link) = &self.docs_link {
                    // Since we are in rtl layout, we need to make our own link since the
                    // re_ui link icon would be reversed.
                    let response = ui
                        .scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let hovered = ui.response().hovered();

                            let tint = design_tokens().color(ColorToken::gray(if hovered {
                                Scale::S900
                            } else {
                                Scale::S700
                            }));

                            ui.small_icon(&icons::EXTERNAL_LINK, Some(tint));

                            ui.label(RichText::new("Docs").color(tint).size(11.0));
                        })
                        .response;

                    if response.clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab(docs_link));
                    }
                }
            },
        );

        for section in &self.sections {
            Self::separator(ui);
            match section {
                HelpSection::Markdown(md) => {
                    ui.markdown_ui(md);
                }
                HelpSection::Controls(controls) => {
                    for row in controls {
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                ui.strong(RichText::new(&row.text).size(11.0));
                                ui.add_space(8.0);
                            },
                            |ui| {
                                ui.set_height(DesignTokens::small_icon_size().y);
                                for item in row.items.0.iter().rev() {
                                    match item {
                                        IconTextItem::Icon(icon) => {
                                            ui.small_icon(icon, None);
                                        }
                                        IconTextItem::Text(text) => {
                                            ui.label(
                                                RichText::new(text.as_str())
                                                    .monospace()
                                                    .size(11.0)
                                                    .color(
                                                        design_tokens()
                                                            .color(ColorToken::gray(Scale::S700)),
                                                    ),
                                            );
                                        }
                                    }
                                }
                            },
                        );
                    }
                }
            }
        }
    }
}
