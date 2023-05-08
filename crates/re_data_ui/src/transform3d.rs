use re_log_types::component_types::{
    Affine3D, Pinhole, Transform3D, TranslationMatrix3x3, TranslationRotationScale,
};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for Transform3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Transform3D::Unknown => {
                ui.label("Unknown transform");
            }
            Transform3D::Affine3D(affine3d) => affine3d.data_ui(ctx, ui, verbosity, query),
            Transform3D::Pinhole(pinhole) => pinhole.data_ui(ctx, ui, verbosity, query),
        }
    }
}

impl DataUi for Affine3D {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label("Rigid 3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                ui.vertical(|ui| {
                    ui.label("Affine 3D transform:");
                    ui.indent("affine3", |ui| match self {
                        Affine3D::TranslationMatrix3x3(translation_matrix) => {
                            translation_matrix.data_ui(ctx, ui, verbosity, query);
                        }
                        Affine3D::TranslationRotationScale(translation_rotation_scale) => {
                            translation_rotation_scale.data_ui(ctx, ui, verbosity, query);
                        }
                    });
                });
            }
        }
    }
}

impl DataUi for TranslationRotationScale {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let TranslationRotationScale {
            translation,
            rotation,
            scale,
        } = self;

        egui::Grid::new("translation_rotation_scale")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("translation");
                translation.data_ui(ctx, ui, verbosity, query);
                ui.end_row();

                ui.label("rotation");
                ui.monospace(format!("{rotation:?}")); // TODO: make prettier
                ui.end_row();

                ui.label("scale");
                ui.monospace(format!("{scale:?}")); // TODO: make prettier
                ui.end_row();
            });
    }
}

impl DataUi for TranslationMatrix3x3 {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let TranslationMatrix3x3 {
            translation,
            matrix,
        } = self;

        egui::Grid::new("translation_rotation_scale")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("translation");
                translation.data_ui(ctx, ui, verbosity, query);
                ui.end_row();

                ui.label("matrix");
                matrix.data_ui(ctx, ui, verbosity, query);
                ui.end_row();
            });
    }
}

impl DataUi for Pinhole {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label("Pinhole transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                let Pinhole {
                    image_from_cam: image_from_view,
                    resolution,
                } = self;

                ui.vertical(|ui| {
                    ui.label("Pinhole transform:");
                    ui.indent("pinole", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("resolution:");
                            if let Some(re_log_types::component_types::Vec2D([x, y])) = resolution {
                                ui.monospace(format!("{x}x{y}"));
                            } else {
                                ui.weak("(none)");
                            }
                        });

                        ui.label("image from view:");
                        ui.indent("image_from_view", |ui| {
                            image_from_view.data_ui(ctx, ui, verbosity, query);
                        });
                    });
                });
            }
        }
    }
}
