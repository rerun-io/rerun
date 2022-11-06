use crate::{ui::space_view::SceneQuery, ViewerContext};
use egui::{Color32, RichText};
use re_data_store::{
    query::visit_type_data_3, FieldName, ObjPath, ObjectTreeProperties, TimeQuery,
};
use re_log_types::{IndexHash, LogMsg, MsgId, ObjectType, TimePoint};

// TODO: deal with the proliferation of pub(crate) specifiers in another PR.

// --- Scene ---

/// A single text entry as part of a whole text scene.
pub struct TextEntry {
    // props
    pub msg_id: MsgId,
    pub obj_path: ObjPath,
    pub time: i64,
    pub color: Option<[u8; 4]>,

    // text entry
    pub level: Option<String>,
    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneText {
    pub text_entries: Vec<TextEntry>,
}

impl SceneText {
    pub(crate) fn load(
        &mut self,
        ctx: &ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery,
    ) {
        let Some(timeline_store) = ctx.log_db.obj_db.store.get(&query.timeline) else {return};

        puffin::profile_function!();

        {
            puffin::profile_scope!("SceneText - load text entries");
            let text_entries = query
                .objects
                .iter()
                .filter(|obj_path| obj_tree_props.projected.get(obj_path).visible)
                .filter_map(|obj_path| {
                    let obj_type = ctx.log_db.obj_db.types.get(obj_path.obj_type_path());
                    (obj_type == Some(&ObjectType::TextEntry))
                        .then(|| {
                            timeline_store
                                .get(obj_path)
                                .map(|obj_store| (obj_store, obj_path))
                        })
                        .flatten()
                })
                .flat_map(|(obj_store, obj_path)| {
                    let mut batch = Vec::new();
                    // TODO: obviously cloning all these strings is not ideal... there are two
                    // situations to account for here.
                    // We could avoid these by modifying how we store all of this in the existing
                    // datastore, but then again we are about to rewrite the datastore so...?
                    // We will need to make sure that we don't need these copies once we switch to
                    // Arrow though!
                    visit_type_data_3(
                        obj_store,
                        &FieldName::from("body"),
                        &TimeQuery::EVERYTHING, // always sticky!
                        ("_visible", "level", "color"),
                        |_instance_index: Option<&IndexHash>,
                         time: i64,
                         msg_id: &MsgId,
                         body: &String,
                         visible: Option<&bool>,
                         level: Option<&String>,
                         color: Option<&[u8; 4]>| {
                            if *visible.unwrap_or(&true) {
                                batch.push(TextEntry {
                                    msg_id: msg_id.clone(),
                                    obj_path: obj_path.clone(),
                                    time,
                                    color: color.copied(),
                                    level: level.map(ToOwned::to_owned),
                                    body: body.to_owned(),
                                });
                            }
                        },
                    );
                    batch
                });
            self.text_entries.extend(text_entries);
        }
    }
}

impl SceneText {
    pub fn clear(&mut self) {
        let Self { text_entries } = self;

        text_entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        let Self { text_entries } = self;

        text_entries.is_empty()
    }
}

// --- UI state & entrypoint ---

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewTextEntryState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,
}

pub(crate) fn view_text_entry(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextEntryState,
    scene: &SceneText,
) -> egui::Response {
    crate::profile_function!();

    let time = ctx
        .rec_cfg
        .time_ctrl
        .time_query()
        .map_or(state.latest_time, |q| match q {
            re_data_store::TimeQuery::LatestAt(time) => time,
            re_data_store::TimeQuery::Range(range) => *range.start(),
        });

    // Did the time cursor move since last time?
    // - If it did, time to autoscroll approriately.
    // - Otherwise, let the user scroll around freely!
    let time_cursor_moved = state.latest_time != time;
    let scroll_to_row = time_cursor_moved.then(|| {
        crate::profile_scope!("TextEntryState - search scroll time");
        let index = scene
            .text_entries
            .partition_point(|entry| entry.time < time);
        usize::min(index, index.saturating_sub(1))
    });

    state.latest_time = time;

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(format!("{} text entries", scene.text_entries.len()));
        ui.separator();

        egui::ScrollArea::horizontal().show(ui, |ui| {
            crate::profile_scope!("render table");
            show_table(ctx, ui, &scene.text_entries, scroll_to_row);
        })
    })
    .response
}

// --- UI impl ---

// TODO: let-else everywhere

fn get_time_point(ctx: &ViewerContext<'_>, entry: &TextEntry) -> Option<TimePoint> {
    let msg = ctx.log_db.get_log_msg(&entry.msg_id).or_else(|| {
        re_log::warn_once!("Missing LogMsg for {:?}", entry.obj_path.obj_type_path());
        None
    })?;

    let data_msg = if let LogMsg::DataMsg(data_msg) = msg {
        data_msg
    } else {
        re_log::warn_once!(
            "LogMsg must be a DataMsg ({:?})",
            entry.obj_path.obj_type_path()
        );
        return None;
    };

    Some(data_msg.time_point.clone())
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn show_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text_entries: &[TextEntry],
    scroll_to_row: Option<usize>,
) {
    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;

    let spacing_y = ui.spacing().item_spacing.y;

    let mut builder = egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(true);

    if let Some(index) = scroll_to_row {
        let row_height_full = ROW_HEIGHT + spacing_y;
        let scroll_to_offset = index as f32 * row_height_full;
        builder = builder.vertical_scroll_offset(scroll_to_offset);
    }

    builder
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(
            Size::initial(180.0).at_least(100.0),
            ctx.log_db.time_points.0.len(),
        ) // time(s)
        .column(Size::initial(120.0).at_least(100.0)) // path
        .column(Size::initial(60.0).at_least(60.0)) // level
        .column(Size::remainder().at_least(200.0)) // body
        .header(20.0, |mut header| {
            for timeline in ctx.log_db.time_points.0.keys() {
                header.col(|ui| {
                    ui.heading(timeline.name().as_str());
                });
            }
            header.col(|ui| {
                ui.heading("Path");
            });
            header.col(|ui| {
                ui.heading("Level");
            });
            header.col(|ui| {
                ui.heading("Body");
            });
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, text_entries.len(), |index, mut row| {
                let text_entry = &text_entries[index];

                // NOTE: `try_from_props` is where we actually fetch data from the underlying
                // store, which is a costly operation.
                // Doing this here guarantees that it only happens for visible rows.
                let time_point = if let Some(time_point) = get_time_point(ctx, text_entry) {
                    time_point
                } else {
                    row.col(|ui| {
                        ui.colored_label(
                            Color32::RED,
                            "<failed to load TextEntry from data store>",
                        );
                    });
                    return;
                };

                // time(s)
                for timeline in ctx.log_db.time_points.0.keys() {
                    row.col(|ui| {
                        if let Some(value) = time_point.0.get(timeline) {
                            ctx.time_button(ui, timeline, *value);
                        }
                    });
                }

                // path
                row.col(|ui| {
                    ctx.obj_path_button(ui, &text_entry.obj_path);
                });

                // level
                row.col(|ui| {
                    if let Some(lvl) = &text_entry.level {
                        ui.label(level_to_rich_text(ui, &lvl));
                    } else {
                        ui.label("-");
                    }
                });

                // body
                row.col(|ui| {
                    if let Some(c) = text_entry.color {
                        let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        ui.colored_label(color, &text_entry.body);
                    } else {
                        ui.label(&text_entry.body);
                    }
                });
            });
        });
}

fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
    match lvl {
        "CRITICAL" => RichText::new(lvl)
            .color(Color32::WHITE)
            .background_color(ui.visuals().error_fg_color),
        "ERROR" => RichText::new(lvl).color(ui.visuals().error_fg_color),
        "WARN" => RichText::new(lvl).color(ui.visuals().warn_fg_color),
        "INFO" => RichText::new(lvl).color(Color32::LIGHT_GREEN),
        "DEBUG" => RichText::new(lvl).color(Color32::LIGHT_BLUE),
        "TRACE" => RichText::new(lvl).color(Color32::LIGHT_GRAY),
        _ => RichText::new(lvl).color(ui.visuals().text_color()),
    }
}
