use re_log_types::component_types::{
    Angle, Rotation3D, RotationAxisAngle, Scale3D, Transform3D, Transform3DRepr,
    TranslationAndMat3, TranslationRotationScale3D,
};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for Transform3D {
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
                // TODO(andreas): Preview some information instead of just a label with hover ui.
                ui.label("3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                let dir_string = if self.from_parent {
                    "parent ➡ child"
                } else {
                    "child ➡ parent"
                };

                ui.vertical(|ui| {
                    ui.label("3D transform");
                    ui.indent("transform_repr", |ui| {
                        ui.label(dir_string);
                        self.transform.data_ui(ctx, ui, verbosity, query);
                    });
                });
            }
        }
    }
}

impl DataUi for Transform3DRepr {
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
                ui.label("3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => match self {
                Transform3DRepr::TranslationAndMat3(translation_matrix) => {
                    translation_matrix.data_ui(ctx, ui, verbosity, query);
                }
                Transform3DRepr::TranslationRotationScale(translation_rotation_scale) => {
                    translation_rotation_scale.data_ui(ctx, ui, verbosity, query);
                }
            },
        }
    }
}

impl DataUi for TranslationRotationScale3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let TranslationRotationScale3D {
            translation,
            rotation,
            scale,
        } = self;

        egui::Grid::new("translation_rotation_scale")
            .num_columns(2)
            .show(ui, |ui| {
                // Unlike Rotation/Scale, we don't have a value that indicates that nothing was logged.
                // We still skip zero translations though since they are typically not logged explicitly.
                if let Some(translation) = translation {
                    ui.label("translation");
                    translation.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                }

                if let Some(rotation) = rotation {
                    ui.label("rotation");
                    rotation.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                }

                if let Some(scale) = scale {
                    ui.label("scale");
                    scale.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                }
            });
    }
}

impl DataUi for Rotation3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Rotation3D::Quaternion(q) => {
                // TODO(andreas): Better formatting for quaternions.
                ui.label(format!("{q:?}"));
            }
            Rotation3D::AxisAngle(RotationAxisAngle { axis, angle }) => {
                egui::Grid::new("axis_angle").num_columns(2).show(ui, |ui| {
                    ui.label("axis");
                    axis.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();

                    ui.label("angle");
                    match angle {
                        Angle::Radians(v) => {
                            ui.label(format!("{}rad", re_format::format_f32(*v)));
                        }
                        Angle::Degrees(v) => {
                            // TODO(andreas): Convert to arc minutes/seconds for very small angles.
                            // That code should be in re_format!
                            ui.label(format!("{}°", re_format::format_f32(*v),));
                        }
                    }
                    ui.end_row();
                });
            }
        }
    }
}

impl DataUi for Scale3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Scale3D::Uniform(scale) => {
                ui.label(re_format::format_f32(*scale));
            }
            Scale3D::ThreeD(v) => {
                v.data_ui(ctx, ui, verbosity, query);
            }
        }
    }
}

impl DataUi for TranslationAndMat3 {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let TranslationAndMat3 {
            translation,
            matrix,
        } = self;

        egui::Grid::new("translation_and_mat3")
            .num_columns(2)
            .show(ui, |ui| {
                if let Some(translation) = translation {
                    ui.label("translation");
                    translation.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                }

                ui.label("matrix");
                matrix.data_ui(ctx, ui, verbosity, query);
                ui.end_row();
            });
    }
}
