use re_types::components::{PinholeProjection, Resolution};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for PinholeProjection {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        self.0.data_ui(ctx, ui, verbosity, query);
    }
}

impl DataUi for Resolution {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let [x, y] = self.0 .0;
        ui.monospace(format!("{x}x{y}"));
    }
}
