use re_log_types::{
    msg_bundle::MsgBundle, ArrowMsg, BeginRecordingMsg, DataMsg, LogMsg, LoggedData, PathOpMsg,
    RecordingInfo, TypeMsg,
};

use crate::{misc::ViewerContext, ui::UiVerbosity};

use super::DataUi;

impl DataUi for LogMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            LogMsg::BeginRecordingMsg(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::TypeMsg(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::DataMsg(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::PathOpMsg(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::ArrowMsg(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::Goodbye(_) => {
                ui.label("Goodbye");
            }
        }
    }
}

impl DataUi for BeginRecordingMsg {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.code("BeginRecordingMsg");
        let BeginRecordingMsg { msg_id: _, info } = self;
        let RecordingInfo {
            application_id,
            recording_id,
            started,
            recording_source,
            is_official_example,
        } = info;

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
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

            ui.monospace("is_official_example:");
            ui.label(format!("{is_official_example}"));
            ui.end_row();
        });
    }
}

impl DataUi for TypeMsg {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.horizontal(|ui| {
            ui.code(self.type_path.to_string());
            ui.label(" = ");
            ui.code(format!("{:?}", self.obj_type));
        });
    }
}

impl DataUi for DataMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let DataMsg {
            msg_id: _,
            time_point,
            data_path,
            data,
        } = self;

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            ui.monospace("data_path:");
            ctx.data_path_button(ui, data_path);
            ui.end_row();

            ui.monospace("time_point:");
            time_point.data_ui(ctx, ui, verbosity, query);
            ui.end_row();

            ui.monospace("data:");
            data.data_ui(ctx, ui, verbosity, query);
            ui.end_row();
        });
    }
}

impl DataUi for LoggedData {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            LoggedData::Null(data_type) => {
                ui.label(format!("null: {:?}", data_type));
            }
            LoggedData::Batch { data, .. } => {
                ui.label(format!("batch: {:?}", data));
            }
            LoggedData::Single(data) => data.data_ui(ctx, ui, verbosity, query),
            LoggedData::BatchSplat(data) => {
                ui.horizontal(|ui| {
                    ui.label("Batch Splat:");
                    data.data_ui(ctx, ui, verbosity, query);
                });
            }
        }
    }
}

impl DataUi for PathOpMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let PathOpMsg {
            msg_id: _,
            time_point,
            path_op,
        } = self;

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            ui.monospace("time_point:");
            time_point.data_ui(ctx, ui, verbosity, query);
            ui.end_row();

            ui.monospace("path_op:");
            path_op.data_ui(ctx, ui, verbosity, query);
            ui.end_row();
        });
    }
}

impl DataUi for ArrowMsg {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self.try_into() {
            Ok(MsgBundle {
                msg_id: _,
                obj_path,
                time_point,
                components,
            }) => {
                egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
                    ui.monospace("obj_path:");
                    ctx.obj_path_button(ui, None, &obj_path);
                    ui.end_row();

                    ui.monospace("time_point:");
                    time_point.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();

                    ui.monospace("components:");
                    components.as_slice().data_ui(ctx, ui, verbosity, query);
                    ui.end_row();
                });
            }
            Err(e) => {
                ui.label(format!("Error parsing ArrowMsg: {e}"));
            }
        }
    }
}
