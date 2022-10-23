const SHOW_ALLOC_COUNT: bool = false;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct MemoryPanel {}

impl MemoryPanel {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.style_mut().wrap = Some(false);

        ui.heading("Memory profile");

        let (total_allocs, total_bytes) = re_mem_tracker::global_allocs_and_bytes();
        let tree = re_mem_tracker::thread_local_tree();

        egui::Grid::new("grid").show(ui, |ui| {
            row_ui(ui, "Total", total_bytes as _, total_allocs as _);

            for (name, root) in &tree.children {
                show_tree(ui, name, root);
            }
        });
    }
}

fn show_tree(ui: &mut egui::Ui, name: &String, tree: &re_mem_tracker::Tree) {
    row_ui(
        ui,
        &format!("{:?}", name),
        tree.stats.total_bytes(),
        tree.stats.total_allocs(),
    );

    // if !tree.children.is_empty() {
    //     ui.indent(name, |ui| {
    //         row_ui(
    //             ui,
    //             "Unaccounted",
    //             tree.stats.total_bytes(),
    //             tree.stats.total_allocs(),
    //         );
    //         for (name, tree) in &tree.children {
    //             show_tree(ui, name, tree);
    //         }
    //     });
    // }
}

fn row_ui(ui: &mut egui::Ui, name: &str, bytes: isize, num_allocs: isize) {
    ui.monospace(name);
    ui.monospace(format_bytes(bytes));
    if SHOW_ALLOC_COUNT {
        ui.monospace(format!("({} allocations)", format_number(num_allocs)));
    }
    ui.end_row();
}

fn format_number(num: isize) -> String {
    num.to_string() // TODO: thousands-separators
}

fn format_bytes(bytes: isize) -> String {
    if bytes < 0 {
        format!("-{}", format_bytes(-bytes))
    } else if bytes < 1_000 {
        format!("{} B", bytes)
    } else if bytes < 1_000_000 {
        format!("{:.2} kB", bytes as f32 / 1_000.0)
    } else if bytes < 1_000_000_000 {
        format!("{:.2} MB", bytes as f32 / 1_000_000.0)
    } else {
        format!("{:.2} GB", bytes as f32 / 1_000_000_000.0)
    }
}
