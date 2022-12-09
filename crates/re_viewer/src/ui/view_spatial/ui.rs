use re_data_store::InstanceIdHash;

use crate::misc::ViewerContext;

use super::{ui_2d::View2DState, ui_3d::View3DState};

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewSpatialState {
    // TODO(andreas): Not pub?
    pub state_2d: View2DState,
    pub state_3d: View3DState,
}

impl ViewSpatialState {
    pub fn show_settings_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        // TODO: 2d/3d
        self.state_3d.show_settings_ui(ctx, ui);
    }

    pub fn hovered_instance_hash(&self) -> InstanceIdHash {
        self.state_3d.hovered_instance_hash() // TODO:
    }
}
