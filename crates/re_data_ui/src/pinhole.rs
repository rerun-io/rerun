use re_types::components::{PinholeProjection, Resolution};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for PinholeProjection {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        if verbosity == UiVerbosity::Small {
            // See if this is a trivial pinhole, and can be displayed as such:
            let fl = self.focal_length_in_pixels();
            let pp = self.principal_point();
            if *self == Self::from_focal_length_and_principal_point(fl, pp) {
                let fl = if fl.x() == fl.y() {
                    fl.x().to_string()
                } else {
                    fl.to_string()
                };

                ui.label(format!("Focal length: {fl}\nPrincipal point: {pp}"))
                    .on_hover_ui(|ui| self.data_ui(ctx, ui, UiVerbosity::Reduced, query));
                return;
            }
        }

        self.0.data_ui(ctx, ui, verbosity, query);
    }
}

impl DataUi for Resolution {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let [x, y] = self.0 .0;
        ui.monospace(format!("{x}x{y}"));
    }
}
