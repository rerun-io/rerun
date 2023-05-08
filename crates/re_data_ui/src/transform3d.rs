use re_log_types::component_types::{
    Affine3D, Angle, AxisAngleRotation, Pinhole, Rotation3D, Scale3D, Transform3D,
    TranslationMatrix3x3, TranslationRotationScale, Vec3D,
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
                // TODO(andreas): Should we skip zero translations?
                // Unlike Rotation/Scale, we don't have a value that indicates that nothing was logged.
                ui.label("translation");
                translation.data_ui(ctx, ui, verbosity, query);
                ui.end_row();

                // Skip identity rotations as they typically aren't logged explicitly.
                if !matches!(rotation, Rotation3D::Identity) {
                    ui.label("rotation");
                    rotation.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                }

                // Skip unit scales as they typically aren't logged explicitly.
                if !matches!(scale, Scale3D::Unit) {
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
            Rotation3D::Identity => {
                ui.label("No rotation");
            }
            Rotation3D::Quaternion(q) => {
                // TODO(andreas): Better formatting for quaternions
                ui.label(format!("{q:?}"));
            }
            Rotation3D::AxisAngle(AxisAngleRotation { axis, angle }) => {
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
            Scale3D::Unit => {
                ui.label("No scaling");
            }
            Scale3D::Uniform(scale) => {
                ui.label(re_format::format_f32(*scale));
            }
            Scale3D::ThreeD(v) => {
                v.data_ui(ctx, ui, verbosity, query);
            }
        }
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
