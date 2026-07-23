//! Flamegraph visualization for memory usage trees.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

use re_byte_size::NamedMemUsageTree;

mod flamegraph;

pub use flamegraph::{FlamegraphState, flamegraph_ui};

/// Show a memory usage tree as a flamegraph.
///
/// This is a convenience function that creates or retrieves the state from `ui.data_mut()`.
pub fn memory_flamegraph_ui(ui: &mut egui::Ui, tree: &NamedMemUsageTree) {
    let state_id = ui.id().with("flamegraph_state");
    let mut state = ui
        .data_mut(|data| data.get_temp::<FlamegraphState>(state_id))
        .unwrap_or_default();

    flamegraph_ui(ui, tree, &mut state);

    ui.data_mut(|data| data.insert_temp(state_id, state));
}
