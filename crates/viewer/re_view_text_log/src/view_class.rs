use std::collections::BTreeSet;

use re_data_ui::item_ui;
use re_log_types::{EntityPath, TimelineName};
use re_types::blueprint::archetypes::{TextLogColumns, TextLogRows};
use re_types::blueprint::components::TextLogColumn;
use re_types::{View as _, datatypes};
use re_types::{ViewClassIdentifier, components::TextLogLevel};
use re_ui::list_item::LabelContent;
use re_ui::{DesignTokens, Help, UiExt as _};
use re_viewer_context::{
    IdentifiedViewSystem as _, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewContext,
    ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewerContext, level_to_rich_text,
};
use re_viewport_blueprint::ViewProperty;

use super::visualizer_system::{Entry, TextLogSystem};

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct TextViewState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    monospace: bool,

    seen_levels: BTreeSet<String>,
}

impl ViewState for TextViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextView;

type ViewType = re_types::blueprint::views::TextLogView;

impl ViewClass for TextView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text log"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_LOG
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Text log view")
            .docs_link("https://rerun.io/docs/reference/types/views/text_log_view")
            .markdown(
                "TextLog entries over time.

Filter message types and toggle column visibility in a selection panel.",
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_columns().component,
            |ctx| {
                let mut columns: Vec<_> = ctx
                    .recording()
                    .times_per_timeline()
                    .timelines()
                    .map(|timeline| {
                        TextLogColumn(datatypes::TextLogColumn::Timeline(
                            timeline.name().as_str().into(),
                        ))
                    })
                    .collect();

                columns.push(datatypes::TextLogColumn::EntityPath.into());
                columns.push(datatypes::TextLogColumn::LogLevel.into());
                columns.push(datatypes::TextLogColumn::Message.into());

                columns
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogRows::descriptor_log_levels().component,
            |ctx| {
                let Ok(state) = ctx.view_state().downcast_ref::<TextViewState>() else {
                    re_log::error_once!(
                        "Failed to get `TextViewState` in text log view fallback, this is a bug."
                    );

                    return Vec::new();
                };
                state
                    .seen_levels
                    .iter()
                    .map(|lvl| TextLogLevel::from(lvl.as_str()))
                    .collect::<Vec<_>>()
            },
        );
        system_registry.register_visualizer::<TextLogSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<TextViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        Some(2.0) // Make text logs wide
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Spawn a single log view at the root if there's any text logs around anywhere.
        // Checking indicators is enough, since we know that this is enough to infer visualizability here.
        if ctx
            .indicated_entities_per_visualizer
            .get(&TextLogSystem::identifier())
            .is_some_and(|entities| entities.iter().any(include_entity))
        {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextViewState>()?;

        let view_ctx = self.view_context(ctx, view_id, state);
        let columns_property = ViewProperty::from_archetype::<TextLogColumns>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );

        let mut columns = columns_property.component_array_or_fallback::<TextLogColumn>(
            &view_ctx,
            TextLogColumns::descriptor_columns().component,
        )?;

        // We need a custom UI here because we use arrays, whih component UI doesn't support.
        ui.list_item_scope("text_log_selection_ui", |ui| {
            ui.list_item().show_hierarchical_with_children(
                ui,
                ui.id(),
                false,
                LabelContent::new("Columns"),
                |ui| {
                    let res =
                        egui_dnd::dnd(ui, "text_log_columns_dnd").show_vec(
                            &mut columns,
                            |ui, item, _handle, _state| {
                                egui::ComboBox::new(
                                    "column_types",
                                    match &item.0 {
                                        datatypes::TextLogColumn::Timeline(_) => "Timeline",
                                        datatypes::TextLogColumn::EntityPath => "Entity Path",
                                        datatypes::TextLogColumn::LogLevel => "Level",
                                        datatypes::TextLogColumn::Message => "Message",
                                    },
                                )
                                .show_ui(ui, |ui| {
                                    let timeline =
                                        if let datatypes::TextLogColumn::Timeline(name) = &item.0 {
                                            name.as_str().to_owned()
                                        } else {
                                            ctx.time_ctrl.timeline().name().to_string()
                                        };
                                    ui.selectable_value(
                                        &mut item.0,
                                        datatypes::TextLogColumn::Timeline(datatypes::Utf8::from(
                                            timeline,
                                        )),
                                        "Timeline",
                                    );

                                    ui.selectable_value(
                                        &mut item.0,
                                        datatypes::TextLogColumn::EntityPath,
                                        "Entity Path",
                                    );

                                    ui.selectable_value(
                                        &mut item.0,
                                        datatypes::TextLogColumn::LogLevel,
                                        "Level",
                                    );

                                    ui.selectable_value(
                                        &mut item.0,
                                        datatypes::TextLogColumn::Message,
                                        "Message",
                                    );
                                });

                                if let datatypes::TextLogColumn::Timeline(name) = &mut item.0 {
                                    egui::ComboBox::new("column_timeline_name", name.as_str())
                                        .show_ui(ui, |ui| {
                                            for timeline in
                                                ctx.recording().times_per_timeline().timelines()
                                            {
                                                ui.selectable_value(
                                                    name,
                                                    datatypes::Utf8::from(timeline.name().as_str()),
                                                    timeline.name().as_str(),
                                                );
                                            }
                                        });
                                }
                            },
                        );

                    if res.is_drag_finished() {
                        columns_property.save_blueprint_component(
                            ctx,
                            &TextLogColumns::descriptor_columns(),
                            &columns,
                        );
                    }
                },
            )
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,

        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tokens = ui.tokens();
        let state = state.downcast_mut::<TextViewState>()?;
        let text = system_output.view_systems.get::<TextLogSystem>()?;

        let rows_property = ViewProperty::from_archetype::<TextLogRows>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        for te in &text.entries {
            if let Some(lvl) = &te.level {
                state.seen_levels.insert(lvl.to_string());
            }
        }

        let view_ctx = self.view_context(ctx, query.view_id, state);

        let levels = rows_property.component_array_or_fallback::<TextLogLevel>(
            &view_ctx,
            TextLogRows::descriptor_log_levels().component,
        )?;

        // TODO(andreas): Should filter text entries in the part-system instead.
        // this likely requires a way to pass state into a context.
        let entries = text
            .entries
            .iter()
            .filter(|te| {
                te.level
                    .as_ref()
                    .is_none_or(|lvl| levels.iter().any(|l| l == lvl))
            })
            .collect::<Vec<_>>();

        let time = ctx.time_ctrl.time_i64().unwrap_or(state.latest_time);
        egui::Frame {
            inner_margin: tokens.view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            // Did the time cursor move since last time?
            // - If it did, autoscroll to the text log to reveal the current time.
            // - Otherwise, let the user scroll around freely!
            let time_cursor_moved = state.latest_time != time;
            let scroll_to_row = time_cursor_moved.then(|| {
                re_tracing::profile_scope!("search scroll time");
                entries.partition_point(|te| te.time.as_i64() < time)
            });

            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                egui::ScrollArea::horizontal()
                    .show(ui, |ui| {
                        re_tracing::profile_scope!("render table");
                        table_ui(&view_ctx, ui, state, &entries, scroll_to_row)
                    })
                    .inner
            })
            .inner
        })
        .inner?;
        state.latest_time = time;

        Ok(())
    }
}

// ---

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn table_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    state: &TextViewState,
    entries: &[&Entry],
    scroll_to_row: Option<usize>,
) -> Result<(), ViewSystemExecutionError> {
    let tokens = ui.tokens();
    let table_style = re_ui::TableStyle::Dense;

    use egui_extras::Column;

    let (global_timeline, global_time) = (
        *ctx.viewer_ctx.time_ctrl.timeline(),
        ctx.viewer_ctx.time_ctrl.time_int(),
    );

    let mut table_builder = egui_extras::TableBuilder::new(ui)
        .resizable(true)
        .vscroll(true)
        .auto_shrink([false; 2]) // expand to take up the whole View
        .min_scrolled_height(0.0) // we can go as small as we need to be in order to fit within the view!
        .max_scroll_height(f32::INFINITY) // Fill up whole height
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));

    if let Some(scroll_to_row) = scroll_to_row {
        table_builder = table_builder.scroll_to_row(scroll_to_row, Some(egui::Align::Center));
    }

    let mut body_clip_rect = None;
    let mut current_time_y = None; // where to draw the current time indicator cursor

    let columns_property = ViewProperty::from_archetype::<TextLogColumns>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    );

    let columns = columns_property.component_array_or_fallback::<TextLogColumn>(
        ctx,
        TextLogColumns::descriptor_columns().component,
    )?;

    for col in &columns {
        match **col {
            datatypes::TextLogColumn::Timeline(_) | datatypes::TextLogColumn::EntityPath => {
                table_builder = table_builder.column(Column::auto().clip(true).at_least(32.0));
            }
            datatypes::TextLogColumn::LogLevel => {
                table_builder = table_builder.column(Column::auto().at_least(30.0));
            }
            datatypes::TextLogColumn::Message => {
                table_builder = table_builder.column(Column::remainder().at_least(100.0));
            }
        }
    }

    table_builder
        .header(tokens.deprecated_table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            for col in &columns {
                header.col(|ui| match &col.0 {
                    datatypes::TextLogColumn::Timeline(name) => {
                        item_ui::timeline_button(ctx.viewer_ctx, ui, &TimelineName::new(name));
                    }
                    datatypes::TextLogColumn::EntityPath => {
                        ui.strong("Entity path");
                    }
                    datatypes::TextLogColumn::LogLevel => {
                        ui.strong("Level");
                    }
                    datatypes::TextLogColumn::Message => {
                        ui.strong("Body");
                    }
                });
            }
        })
        .body(|mut body| {
            tokens.setup_table_body(&mut body, table_style);

            body_clip_rect = Some(body.max_rect());

            let query = ctx.current_query();

            let row_heights = entries
                .iter()
                .map(|te| calc_row_height(tokens, table_style, te));
            body.heterogeneous_rows(row_heights, |mut row| {
                let entry = &entries[row.index()];

                for col in &columns {
                    row.col(|ui| {
                        match &col.0 {
                            datatypes::TextLogColumn::Timeline(name) => {
                                let timeline = TimelineName::new(name);
                                let row_time = entry
                                    .timepoint
                                    .get(&timeline)
                                    .map(re_log_types::TimeInt::from)
                                    .unwrap_or(re_log_types::TimeInt::STATIC);
                                item_ui::time_button(ctx.viewer_ctx, ui, &timeline, row_time);

                                if let Some(global_time) = global_time
                                    && timeline == *global_timeline.name()
                                {
                                    #[expect(clippy::comparison_chain)]
                                    if global_time < row_time {
                                        // We've past the global time - it is thus above this row.
                                        if current_time_y.is_none() {
                                            current_time_y = Some(ui.max_rect().top());
                                        }
                                    } else if global_time == row_time {
                                        // This row is exactly at the current time.
                                        // We could draw the current time exactly onto this row, but that would look bad,
                                        // so let's draw it under instead. It looks better in the "following" mode.
                                        current_time_y = Some(ui.max_rect().bottom());
                                    }
                                }
                            }
                            datatypes::TextLogColumn::EntityPath => {
                                item_ui::entity_path_button(
                                    ctx.viewer_ctx,
                                    &query,
                                    ctx.recording(),
                                    ui,
                                    None,
                                    &entry.entity_path,
                                );
                            }
                            datatypes::TextLogColumn::LogLevel => {
                                if let Some(lvl) = &entry.level {
                                    ui.label(level_to_rich_text(ui, lvl));
                                } else {
                                    ui.label("-");
                                }
                            }
                            datatypes::TextLogColumn::Message => {
                                let mut text = egui::RichText::new(entry.body.as_str());

                                if state.monospace {
                                    text = text.monospace();
                                }
                                if let Some(color) = entry.color {
                                    text = text.color(color);
                                }

                                ui.label(text);
                            }
                        }
                    });
                }
            });
        });

    // TODO(cmc): this draws on top of the headers :(
    if let (Some(body_clip_rect), Some(current_time_y)) = (body_clip_rect, current_time_y) {
        // Show that the current time is here:
        ui.painter().with_clip_rect(body_clip_rect).hline(
            ui.max_rect().x_range(),
            current_time_y,
            (1.0, ui.tokens().strong_fg_color),
        );
    }

    Ok(())
}

fn calc_row_height(tokens: &DesignTokens, table_style: re_ui::TableStyle, entry: &Entry) -> f32 {
    // Simple, fast, ugly, and functional
    let num_newlines = entry.body.bytes().filter(|&c| c == b'\n').count();
    let num_rows = 1 + num_newlines;
    num_rows as f32 * tokens.table_row_height(table_style)
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TextView.help(ctx));
}
