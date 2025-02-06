use crate::icon_text::{IconText, IconTextItem};
use crate::{design_tokens, icons, ColorToken, DesignTokens, Icon, Scale, UiExt};
use eframe::emath::Align;
use egui::{
    Color32, Layout, OpenUrl, Response, RichText, Sense, TextBuffer, Ui, UiBuilder, Widget,
};

pub struct Help<'a> {
    title: String,
    markdown: Option<String>,
    docs_link: Option<String>,
    controls: Vec<ControlRow<'a>>,
}

pub struct ControlRow<'a> {
    text: String,
    items: IconText<'a>,
}

impl<'a> ControlRow<'a> {
    pub fn new(text: impl ToString, items: IconText<'a>) -> Self {
        Self {
            text: text.to_string(),
            items,
        }
    }
}

impl<'a> Help<'a> {
    pub fn new(title: impl ToString) -> Self {
        Self {
            title: title.to_string(),
            markdown: None,
            docs_link: None,
            controls: Vec::new(),
        }
    }

    pub fn with_markdown(mut self, markdown: impl ToString) -> Self {
        self.markdown = Some(markdown.to_string());
        self
    }

    pub fn with_docs_link(mut self, docs_link: impl ToString) -> Self {
        self.docs_link = Some(docs_link.to_string());
        self
    }

    pub fn with_controls(mut self, controls: Vec<ControlRow<'a>>) -> Self {
        self.controls = controls;
        self
    }

    pub fn with_control(mut self, label: &'a str, items: IconText<'a>) -> Self {
        self.controls.push(ControlRow::new(label, items));
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
        if let Some(markdown) = &self.markdown {
            Self::separator(ui);
            ui.markdown_ui(markdown);
        }

        if !self.controls.is_empty() {
            Self::separator(ui);
        }

        for row in &self.controls {
            egui::Sides::new().spacing(4.0).show(
                ui,
                |ui| {
                    ui.strong(RichText::new(&row.text).size(11.0));
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
                                    RichText::new(text.as_str()).monospace().size(11.0).color(
                                        design_tokens().color(ColorToken::gray(Scale::S700)),
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
