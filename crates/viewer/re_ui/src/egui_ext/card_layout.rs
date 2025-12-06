use egui::{Frame, Id, NumExt as _, Ui};

pub struct CardLayoutItem {
    pub frame: Frame,
    pub min_width: f32,
}

pub struct CardLayout {
    items: Vec<CardLayoutItem>,
}

#[derive(Default, Debug, Clone)]
struct IntroSectionLayoutStats {
    max_inner_height: f32,
}

impl CardLayout {
    pub fn new(items: Vec<CardLayoutItem>) -> Self {
        Self { items }
    }

    pub fn show(self, ui: &mut Ui, mut show_item: impl FnMut(&mut Ui, usize)) {
        let Self { mut items } = self;
        // We pop from the end, so reverse to make it easier to read
        items.reverse();

        let available_width = ui.available_width();

        let mut row = 0;
        let mut index = 0;

        while !items.is_empty() {
            let mut row_width = 0.0;
            let mut row_items = vec![];
            while let Some(item) = items.pop_if(|item| {
                row_width + item.min_width <= available_width || row_items.is_empty()
            }) {
                row_width += item.min_width;
                row_items.push(item);
            }

            let gap_space = ui.spacing().item_spacing.x * (row_items.len() - 1) as f32;
            let gap_space_item = gap_space / row_items.len() as f32;
            let item_growth = available_width / row_width;

            let row_stats_id = Id::new(row);
            let row_stats = ui.data_mut(|data| {
                data.get_temp_mut_or_default::<IntroSectionLayoutStats>(row_stats_id)
                    .clone()
            });
            let mut new_row_stats = IntroSectionLayoutStats::default();

            ui.horizontal(|ui| {
                for item in row_items {
                    let frame = item.frame;
                    let frame_margin_x = frame.inner_margin.sum().x;
                    frame.show(ui, |ui| {
                        ui.set_width(
                            ((item_growth * item.min_width) - frame_margin_x - gap_space_item)
                                .at_most(ui.available_width()),
                        );
                        show_item(&mut *ui, index);

                        let height = ui.min_size().y;
                        new_row_stats.max_inner_height =
                            f32::max(new_row_stats.max_inner_height, height);

                        ui.set_height(row_stats.max_inner_height);
                    });
                    index += 1;
                }
            });

            row += 1;
            ui.data_mut(|data| {
                data.insert_temp(row_stats_id, new_row_stats);
            });
        }
    }
}
