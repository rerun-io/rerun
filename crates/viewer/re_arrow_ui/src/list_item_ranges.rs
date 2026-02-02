use std::ops::Range;

use egui::Ui;
use re_ui::UiExt as _;
use re_ui::list_item::LabelContent;

/// Utility to efficiently display a large list as recursive tree of smaller ranges.
pub fn list_item_ranges(ui: &mut Ui, range: Range<usize>, item_fn: &mut dyn FnMut(&mut Ui, usize)) {
    let range_len = range.len();

    const RANGE_SIZE: usize = 100;

    if range_len <= RANGE_SIZE {
        for i in range {
            item_fn(ui, i);
        }
        return;
    }

    let chunk_size = if range_len <= 10_000 {
        100
    } else if range_len <= 1_000_000 {
        10_000
    } else if range_len <= 100_000_000 {
        1_000_000
    } else {
        100_000_000
    };

    let mut current = range.start;
    while current < range.end {
        let chunk_end = usize::min(current + chunk_size, range.end);
        let chunk_range = current..chunk_end;
        let id = ui.unique_id().with(chunk_range.clone());
        ui.list_item().show_hierarchical_with_children(
            ui,
            id,
            false,
            LabelContent::new(format!("{}..{}", chunk_range.start, chunk_range.end - 1)),
            |ui| {
                list_item_ranges(ui, chunk_range, item_fn);
            },
        );
        current = chunk_end;
    }
}
