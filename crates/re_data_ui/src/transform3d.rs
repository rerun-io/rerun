use re_types::datatypes::{Scale3D, Transform3D, TranslationAndMat3x3, TranslationRotationScale3D};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::DataUi;

impl DataUi for re_types::components::Transform3D {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List => {
                // TODO(andreas): Preview some information instead of just a label with hover ui.
                ui_layout.label(ui, "3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiLayout::Tooltip, query, db);
                });
            }

            UiLayout::SelectionPanelFull
            | UiLayout::SelectionPanelLimitHeight
            | UiLayout::Tooltip => {
                let from_parent = match &self.0 {
                    Transform3D::TranslationRotationScale(t) => t.from_parent,
                    Transform3D::TranslationAndMat3x3(t) => t.from_parent,
                };
                let dir_string = if from_parent {
                    "parent ➡ child"
                } else {
                    "child ➡ parent"
                };

                ui.vertical(|ui| {
                    ui_layout.label(ui, "3D transform");
                    ui.indent("transform_repr", |ui| {
                        ui_layout.label(ui, dir_string);
                        self.0.data_ui(ctx, ui, ui_layout, query, db);
                    });
                });
            }
        }
    }
}

impl DataUi for re_types::components::OutOfTreeTransform3D {
    #[inline]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        re_types::components::Transform3D(self.0).data_ui(ctx, ui, ui_layout, query, db);
    }
}

impl DataUi for Transform3D {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List => {
                ui.label("3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiLayout::SelectionPanelLimitHeight, query, db);
                });
            }

            UiLayout::SelectionPanelFull
            | UiLayout::SelectionPanelLimitHeight
            | UiLayout::Tooltip => match self {
                Self::TranslationAndMat3x3(translation_matrix) => {
                    translation_matrix.data_ui(ctx, ui, ui_layout, query, db);
                }
                Self::TranslationRotationScale(translation_rotation_scale) => {
                    translation_rotation_scale.data_ui(ctx, ui, ui_layout, query, db);
                }
            },
        }
    }
}

impl DataUi for TranslationRotationScale3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            translation,
            rotation,
            scale,
            from_parent: _,
        } = self;

        egui::Grid::new("translation_rotation_scale")
            .num_columns(2)
            .show(ui, |ui| {
                // Unlike Rotation/Scale, we don't have a value that indicates that nothing was logged.
                // We still skip zero translations though since they are typically not logged explicitly.
                if let Some(translation) = translation {
                    ui.label("translation");
                    translation.data_ui(ctx, ui, ui_layout, query, db);
                    ui.end_row();
                }

                if let Some(rotation) = rotation {
                    ui.label("rotation");
                    rotation.data_ui(ctx, ui, ui_layout, query, db);
                    ui.end_row();
                }

                if let Some(scale) = scale {
                    ui.label("scale");
                    scale.data_ui(ctx, ui, ui_layout, query, db);
                    ui.end_row();
                }
            });
    }
}

impl DataUi for Scale3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match self {
            Self::Uniform(scale) => {
                ui.label(re_format::format_f32(*scale));
            }
            Self::ThreeD(v) => {
                v.data_ui(ctx, ui, ui_layout, query, db);
            }
        }
    }
}

impl DataUi for TranslationAndMat3x3 {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            translation,
            mat3x3,
            from_parent: _,
        } = self;

        egui::Grid::new("translation_and_mat3")
            .num_columns(2)
            .show(ui, |ui| {
                if let Some(translation) = translation {
                    ui.label("translation");
                    translation.data_ui(ctx, ui, ui_layout, query, db);
                    ui.end_row();
                }

                if let Some(matrix) = mat3x3 {
                    ui.label("matrix");
                    matrix.data_ui(ctx, ui, ui_layout, query, db);
                    ui.end_row();
                }
            });
    }
}
