use re_log_types::{
    msg_bundle::MsgBundle, ArrowMsg, BeginRecordingMsg, Data, DataMsg, LogMsg, LoggedData,
    PathOpMsg, RecordingInfo, TypeMsg,
};

use crate::{misc::ViewerContext, ui::Preview};

use super::DataUi;

impl DataUi for LogMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        match self {
            LogMsg::BeginRecordingMsg(msg) => msg.data_ui(ctx, ui, preview),
            LogMsg::TypeMsg(msg) => msg.data_ui(ctx, ui, preview),
            LogMsg::DataMsg(msg) => msg.data_ui(ctx, ui, preview),
            LogMsg::PathOpMsg(msg) => msg.data_ui(ctx, ui, preview),
            LogMsg::ArrowMsg(msg) => msg.data_ui(ctx, ui, preview),
        }
    }
}

impl DataUi for BeginRecordingMsg {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        ui.code("BeginRecordingMsg");
        let BeginRecordingMsg { msg_id: _, info } = self;
        let RecordingInfo {
            application_id,
            recording_id,
            started,
            recording_source,
        } = info;

        egui::Grid::new("fields")
            .num_columns(2)
            .show(ui, |ui| {
                ui.monospace("application_id:");
                ui.label(application_id.to_string());
                ui.end_row();

                ui.monospace("recording_id:");
                ui.label(format!("{recording_id:?}"));
                ui.end_row();

                ui.monospace("started:");
                ui.label(started.format());
                ui.end_row();

                ui.monospace("recording_source:");
                ui.label(format!("{recording_source}"));
                ui.end_row();
            })
            .response
    }
}

impl DataUi for TypeMsg {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        ui.horizontal(|ui| {
            ui.code(self.type_path.to_string());
            ui.label(" = ");
            ui.code(format!("{:?}", self.obj_type));
        })
        .response
    }
}

impl DataUi for DataMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        let DataMsg {
            msg_id: _,
            time_point,
            data_path,
            data,
        } = self;

        egui::Grid::new("fields")
            .num_columns(2)
            .show(ui, |ui| {
                ui.monospace("data_path:");
                ui.label(format!("{data_path}"));
                ui.end_row();

                ui.monospace("time_point:");
                time_point.data_ui(ctx, ui, preview);
                ui.end_row();

                ui.monospace("data:");
                data.data_ui(ctx, ui, preview);
                ui.end_row();
            })
            .response
    }

    fn detailed_data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        let DataMsg {
            msg_id: _,
            time_point,
            data_path,
            data,
        } = self;

        let is_image = matches!(data, LoggedData::Single(Data::Tensor(_)));

        let grid_resp = egui::Grid::new("fields")
            .num_columns(2)
            .show(ui, |ui| {
                ui.monospace("data_path:");
                ctx.data_path_button(ui, data_path);
                ui.end_row();

                ui.monospace("time_point:");
                time_point.data_ui(ctx, ui, preview);
                ui.end_row();

                if !is_image {
                    ui.monospace("data:");
                    data.data_ui(ctx, ui, Preview::Medium);
                    ui.end_row();
                }
            })
            .response;

        if let LoggedData::Single(Data::Tensor(tensor)) = &data {
            tensor.data_ui(ctx, ui, preview)
        } else {
            grid_resp
        }
    }
}

impl DataUi for LoggedData {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        match self {
            LoggedData::Null(data_type) => ui.label(format!("null: {:?}", data_type)),
            LoggedData::Batch { data, .. } => ui.label(format!("batch: {:?}", data)),
            LoggedData::Single(data) => data.data_ui(ctx, ui, preview),
            LoggedData::BatchSplat(data) => {
                ui.horizontal(|ui| {
                    ui.label("Batch Splat:");
                    data.data_ui(ctx, ui, preview)
                })
                .response
            }
        }
    }
}

/// Previously `path_op_msg_ui()`
impl DataUi for PathOpMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        let PathOpMsg {
            msg_id: _,
            time_point,
            path_op,
        } = self;

        egui::Grid::new("fields")
            .num_columns(2)
            .show(ui, |ui| {
                ui.monospace("time_point:");
                time_point.data_ui(ctx, ui, preview);
                ui.end_row();

                ui.monospace("path_op:");
                path_op.data_ui(ctx, ui, preview);
                ui.end_row();
            })
            .response
    }
}

/// Previously `arrow_msg_ui()`
impl DataUi for ArrowMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        match self.try_into() {
            Ok(MsgBundle {
                msg_id: _,
                obj_path,
                time_point,
                components,
            }) => {
                egui::Grid::new("fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.monospace("obj_path:");
                        ui.label(format!("{obj_path}"));
                        ui.end_row();

                        ui.monospace("time_point:");
                        time_point.data_ui(ctx, ui, preview);
                        ui.end_row();

                        ui.monospace("data:");
                        components.as_slice().data_ui(ctx, ui, preview);
                        ui.label("TODO(john)");
                        ui.end_row();
                    })
                    .response
            }
            Err(e) => ui.label(format!("Error parsing ArrowMsg: {e}")),
        }
    }
}
