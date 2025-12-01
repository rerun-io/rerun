use eframe::epaint::Margin;
use egui::{Frame, RichText, TextStyle, Theme, Ui};
use re_ui::egui_ext::card_layout::{CardLayout, CardLayoutItem};
use re_ui::{UiExt, design_tokens_of};
use re_uri::Origin;

pub struct CloudLoginState {
    pub has_server: Option<Origin>,
    pub has_token: bool,
}

pub enum IntroItem {
    DocItem {
        title: &'static str,
        url: &'static str,
        body: &'static str,
    },
    CloudLoginItem,
}

impl IntroItem {
    fn items() -> Vec<Self> {
        vec![
            IntroItem::DocItem {
                title: "Send data in",
                url: "",
                body: "Send data to Rerun from your running applications or existing files.",
            },
            IntroItem::DocItem {
                title: "Explore data",
                url: "",
                body: "Familiarize yourself with the basics of using the Rerun Viewer.",
            },
            IntroItem::DocItem {
                title: "Query data out",
                url: "",
                body: "Perform analysis and send back the results to the original recording.",
            },
            IntroItem::CloudLoginItem,
        ]
    }

    fn frame(&self, ui: &Ui) -> Frame {
        let opposite_theme = match ui.theme() {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };
        let opposite_tokens = design_tokens_of(opposite_theme);
        let tokens = ui.tokens();
        let frame = Frame::new()
            .inner_margin(Margin::same(16))
            .corner_radius(8)
            .stroke(tokens.native_frame_stroke);
        match self {
            IntroItem::DocItem { .. } => frame,
            IntroItem::CloudLoginItem => frame.fill(opposite_tokens.panel_bg_color),
        }
    }

    fn card_item(&self, ui: &Ui) -> CardLayoutItem {
        let frame = self.frame(ui);
        let min_width = match &self {
            IntroItem::DocItem { .. } => 200.0,
            IntroItem::CloudLoginItem => 400.0,
        };
        CardLayoutItem { frame, min_width }
    }

    fn show(&self, ui: &mut Ui) {
        let label_size = 13.0;
        ui.vertical(|ui| match self {
            IntroItem::DocItem { title, url, body } => {
                egui::Sides::new().shrink_left().show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                    ui.heading(RichText::new(*title).strong());
                }, |ui| {
                    ui.re_hyperlink("Docs", *url, true);
                });
                ui.label(RichText::new(*body).size(label_size));
            }
            IntroItem::CloudLoginItem => {
                let opposite_theme = match ui.theme() {
                    Theme::Dark => Theme::Light,
                    Theme::Light => Theme::Dark,
                };
                let opposite_tokens = design_tokens_of(opposite_theme);
                ui.set_style(ui.ctx().style_of(opposite_theme));

                ui.heading(RichText::new("Rerun Cloud").strong());

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.style_mut().text_styles.get_mut(&TextStyle::Body).unwrap().size = label_size;
                    ui.label(
                        "Iterate faster on robotics learning with unified infrastructure. Interested? Read more "
                    );
                    ui.hyperlink_to("here", "");
                    ui.label(" or ");
                    ui.hyperlink_to("book a demo", "");
                    ui.label(".");
                });

                if ui.primary_button("Add server and login").clicked() {

                };
            }
        });
    }
}

#[derive(Default, Debug, Clone)]
struct IntroSectionLayoutStats {
    max_inner_height: f32,
}

pub fn intro_section(ui: &mut egui::Ui) {
    let mut items = IntroItem::items();

    ui.add_space(8.0);

    CardLayout::new(items.iter().map(|item| item.card_item(ui)).collect()).show(ui, |ui, index| {
        let item = &items[index];
        item.show(ui);
    });
}
