use crate::icon_text::{IconText, IconTextItem};
use crate::{design_tokens, icons, ColorToken, Icon, Scale, UiExt};
use eframe::emath::Align;
use egui::{Color32, Layout, OpenUrl, Response, RichText, Sense, Ui, UiBuilder, Widget};

pub struct Help {
    title: String,
    markdown: Option<String>,
    docs_link: Option<String>,
    controls: Vec<ControlRow>,
}

pub struct ControlRow {
    text: String,
    items: IconText<'static>,
}

impl ControlRow {
    pub fn new(text: impl ToString, items: IconText<'static>) -> Self {
        Self {
            text: text.to_string(),
            items,
        }
    }
}

impl Help {
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

    pub fn with_controls(mut self, controls: Vec<ControlRow>) -> Self {
        self.controls = controls;
        self
    }

    pub fn ui(&self, ui: &mut Ui) {
        egui::Sides::new().show(
            ui,
            |ui| {
                ui.strong(&self.title);
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

                            ui.label(RichText::new("Docs").color(tint));
                        })
                        .response;

                    if response.clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab(docs_link));
                    }
                }
            },
        );
        if let Some(markdown) = &self.markdown {
            ui.separator();
            ui.markdown_ui(markdown);
        }

        if !self.controls.is_empty() {
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

        // TODO: Id
        egui::Grid::new("help").num_columns(2).show(ui, |ui| {
            for row in &self.controls {
                ui.strong(&row.text);

                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    for item in row.items.0.iter().rev() {
                        match item {
                            IconTextItem::Icon(icon) => {
                                ui.small_icon(icon, None);
                            }
                            IconTextItem::Text(text) => {
                                ui.label(
                                    RichText::new(&**text).monospace().color(
                                        design_tokens().color(ColorToken::gray(Scale::S700)),
                                    ),
                                );
                            }
                        }
                    }
                });

                ui.end_row();
            }
        });

        // egui::Sides::new()
        //     .show(ui, |ui| {
        //         for row in &self.controls {
        //             ui.label(&row.text);
        //         }
        //     }, |ui| {});
        // Add labels
    }
}
