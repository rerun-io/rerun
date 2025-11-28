use eframe::epaint::Margin;
use egui::{Frame, Id, Ui};
use emath::{NumExt, Rect};
use re_ui::{DesignTokens, UiExt};

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

    fn weight(&self) -> u32 {
        match self {
            IntroItem::DocItem { .. } => 1,
            IntroItem::CloudLoginItem => 2,
        }
    }

    fn frame(&self, tokens: &DesignTokens) -> Frame {
        let frame = Frame::new()
            .inner_margin(Margin::same(16))
            .corner_radius(8)
            .stroke(tokens.native_frame_stroke);
        match self {
            IntroItem::DocItem { .. } => frame,
            IntroItem::CloudLoginItem => frame,
        }
    }

    fn show(&self, ui: &mut Ui) {
        ui.vertical(|ui| match self {
            IntroItem::DocItem { title, url, body } => {
                egui::Sides::new().shrink_left().show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.heading(*title);
                }, |ui| {
                    ui.re_hyperlink("Docs", *url, true);
                });
                ui.label(*body);
            }
            IntroItem::CloudLoginItem => {
                ui.heading("Rerun Cloud");
                ui.label("Iterate faster on robotics learning with unified infrastructure. Interested? Read more here or book a demo");

                ui.button("Add server and login");
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
    items.reverse();

    let available_width = ui.available_width();
    let target_column_width = 240.0;
    let max_columns = (available_width / target_column_width).ceil() as u32;

    ui.ctx().debug_painter().debug_rect(
        Rect::from_min_size(ui.cursor().min, ui.available_size()),
        egui::Color32::LIGHT_BLUE,
        "",
    );

    ui.add_space(8.0);

    let mut row = 0;

    while !items.is_empty() {
        let mut row_columns = 0;
        let mut row_items = vec![];
        while let Some(item) =
            items.pop_if(|item| row_columns + item.weight() <= max_columns || row_items.is_empty())
        {
            row_columns += item.weight();
            row_items.push(item);
        }

        let gap_space = ui.spacing().item_spacing.x * (row_items.len() - 1) as f32;
        let gap_space_item = gap_space / row_items.len() as f32;
        let column_width = available_width / row_columns as f32;

        let row_stats_id = Id::new(row);
        let row_stats = ui.data_mut(|data| {
            data.get_temp_mut_or_default::<IntroSectionLayoutStats>(row_stats_id)
                .clone()
        });
        let mut new_row_stats = IntroSectionLayoutStats::default();

        ui.horizontal(|ui| {
            for item in row_items {
                let frame = item.frame(ui.tokens());
                let frame_margin_x = frame.inner_margin.sum().x;
                frame.show(ui, |ui| {
                    ui.set_width(
                        ((column_width * item.weight() as f32) - frame_margin_x - gap_space_item)
                            .at_most(ui.available_width()),
                    );
                    item.show(ui);

                    let height = ui.min_size().y;
                    new_row_stats.max_inner_height =
                        f32::max(new_row_stats.max_inner_height, height);

                    ui.set_height(row_stats.max_inner_height);
                });
            }
        });

        row += 1;
        ui.data_mut(|data| {
            data.insert_temp(row_stats_id, new_row_stats);
        });
    }
}
