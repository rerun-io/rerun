use std::collections::BTreeSet;

use re_data_ui::item_ui::{self, timeline_button};
use re_log_types::{EntityPath, TimelineName};
use re_sdk_types::blueprint::archetypes::{TextLogColumns, TextLogFormat, TextLogRows};
use re_sdk_types::blueprint::components::{Enabled, TextLogColumn, TimelineColumn};
use re_sdk_types::blueprint::datatypes as bp_datatypes;
use re_sdk_types::components::TextLogLevel;
use re_sdk_types::{View as _, ViewClassIdentifier, datatypes};
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

    seen_levels: BTreeSet<String>,

    last_columns_min_sizes: Vec<u32>,
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

type ViewType = re_sdk_types::blueprint::views::TextLogView;

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
            TextLogColumns::descriptor_timeline_columns().component,
            |ctx| {
                ctx.viewer_ctx()
                    .recording()
                    .timelines()
                    .keys()
                    .map(|t| {
                        TimelineColumn(bp_datatypes::TimelineColumn {
                            visible: true.into(),
                            timeline: t.as_str().into(),
                        })
                    })
                    .collect::<Vec<_>>()
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_text_log_columns().component,
            |_ctx| {
                [
                    bp_datatypes::TextLogColumnKind::EntityPath,
                    bp_datatypes::TextLogColumnKind::LogLevel,
                    bp_datatypes::TextLogColumnKind::Body,
                ]
                .map(|kind| {
                    TextLogColumn(bp_datatypes::TextLogColumn {
                        kind,
                        visible: true.into(),
                    })
                })
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogRows::descriptor_filter_by_log_level().component,
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
                    .map(|lvl| TextLogLevel(datatypes::Utf8::from(lvl.as_str())))
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
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextViewState>()?;

        ui.list_item_scope("text_log_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            re_view::view_property_ui::<TextLogColumns>(&ctx, ui);
            view_property_ui_rows(&ctx, ui);
            re_view::view_property_ui::<TextLogFormat>(&ctx, ui);
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

        let columns_property = ViewProperty::from_archetype::<TextLogColumns>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let rows_property = ViewProperty::from_archetype::<TextLogRows>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let format_property = ViewProperty::from_archetype::<TextLogFormat>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);

        let monospace_body = format_property.component_or_fallback::<Enabled>(
            &view_ctx,
            TextLogFormat::descriptor_monospace_body().component,
        )?;
        let columns = columns_property.component_array_or_fallback::<TextLogColumn>(
            &view_ctx,
            TextLogColumns::descriptor_text_log_columns().component,
        )?;

        let timeline_columns = columns_property.component_array_or_fallback::<TimelineColumn>(
            &view_ctx,
            TextLogColumns::descriptor_timeline_columns().component,
        )?;

        let levels = rows_property.component_array_or_fallback::<TextLogLevel>(
            &view_ctx,
            TextLogRows::descriptor_filter_by_log_level().component,
        )?;

        for te in &text.entries {
            if let Some(lvl) = &te.level {
                state.seen_levels.insert(lvl.to_string());
            }
        }

        // TODO(andreas): Should filter text entries in the part-system instead.
        // this likely requires a way to pass state into a context.
        let entries = text
            .entries
            .iter()
            .filter(|te| {
                te.level
                    .as_ref()
                    .is_none_or(|lvl| levels.iter().any(|l| l.as_str() == lvl.as_str()))
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
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    re_tracing::profile_scope!("render table");
                    table_ui(
                        ctx,
                        ui,
                        state,
                        &timeline_columns,
                        &columns,
                        **monospace_body,
                        &entries,
                        scroll_to_row,
                    );
                })
            })
        });
        state.latest_time = time;

        Ok(())
    }
}

// ---

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
#[expect(clippy::too_many_arguments)]
fn table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut TextViewState,
    timeline_columns: &[TimelineColumn],
    columns: &[TextLogColumn],
    monospace_body: bool,
    entries: &[&Entry],
    scroll_to_row: Option<usize>,
) {
    let tokens = ui.tokens();
    let table_style = re_ui::TableStyle::Dense;

    use egui_extras::Column;

    let (global_timeline, global_time) = (*ctx.time_ctrl.timeline_name(), ctx.time_ctrl.time_int());

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

    let mut new_column_sizes = Vec::new();
    let mut last_columns = state.last_columns_min_sizes.iter();

    let mut size_column = |column: Column, min_size: u32| {
        // If this isn't the same min size as before the order changed.
        let auto_resize = last_columns.next().is_some_and(|c| *c != min_size);

        new_column_sizes.push(min_size);

        column
            .at_least(min_size as f32)
            .auto_size_this_frame(auto_resize)
    };

    for col in timeline_columns {
        if *col.visible {
            table_builder = table_builder.column(size_column(Column::auto().clip(true), 32));
        }
    }

    for col in columns {
        if !*col.visible {
            continue;
        }

        let col = match col.kind {
            bp_datatypes::TextLogColumnKind::EntityPath => {
                size_column(Column::auto().clip(true), 32)
            }
            bp_datatypes::TextLogColumnKind::LogLevel => size_column(Column::auto(), 30),
            bp_datatypes::TextLogColumnKind::Body => size_column(Column::remainder(), 100),
        };

        table_builder = table_builder.column(col);
    }

    state.last_columns_min_sizes = new_column_sizes;

    table_builder
        .header(tokens.deprecated_table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            for col in timeline_columns {
                if !*col.visible {
                    continue;
                }

                header.col(|ui| {
                    timeline_button(ctx, ui, &TimelineName::new(&col.timeline));
                });
            }
            for col in columns {
                if !*col.visible {
                    continue;
                }
                header.col(|ui| {
                    column_name_ui(ui, &col.kind);
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

                for col in timeline_columns {
                    if !*col.visible {
                        continue;
                    }

                    let timeline = TimelineName::new(&col.timeline);

                    row.col(|ui| {
                        let row_time = entry
                            .timepoint
                            .get(&timeline)
                            .map(re_log_types::TimeInt::from)
                            .unwrap_or(re_log_types::TimeInt::STATIC);
                        item_ui::time_button(ctx, ui, &timeline, row_time);

                        if let Some(global_time) = global_time
                            && timeline == global_timeline
                        {
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
                    });
                }

                for col in columns {
                    if !*col.visible {
                        continue;
                    }

                    row.col(|ui| match col.kind {
                        bp_datatypes::TextLogColumnKind::EntityPath => {
                            item_ui::entity_path_button(
                                ctx,
                                &query,
                                ctx.recording(),
                                ui,
                                None,
                                &entry.entity_path,
                            );
                        }
                        bp_datatypes::TextLogColumnKind::LogLevel => {
                            if let Some(lvl) = &entry.level {
                                ui.label(level_to_rich_text(ui, lvl));
                            } else {
                                ui.label("-");
                            }
                        }
                        bp_datatypes::TextLogColumnKind::Body => {
                            let mut text = egui::RichText::new(entry.body.as_str());

                            if monospace_body {
                                text = text.monospace();
                            }
                            if let Some(color) = entry.color {
                                text = text.color(color);
                            }

                            ui.label(text);
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
}

fn column_name_ui(ui: &mut egui::Ui, column: &bp_datatypes::TextLogColumnKind) -> egui::Response {
    ui.strong(column.name())
}

/// We need this to be a custom ui to be able to use the view state to get seen text log levels.
///
/// This could potentially be avoided if we could add component ui's from this crate.
fn view_property_ui_rows(ctx: &ViewContext<'_>, ui: &mut egui::Ui) {
    let property = ViewProperty::from_archetype::<TextLogRows>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    );

    let reflection = ctx.viewer_ctx.reflection();
    let Some(reflection) = reflection.archetypes.get(&property.archetype_name) else {
        ui.error_label(format!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        ));
        return;
    };

    let query_ctx = property.query_context(ctx);

    let sub_prop_ui = |ui: &mut egui::Ui| {
        for field in &reflection.fields {
            if field
                .component_descriptor(property.archetype_name)
                .component
                == TextLogRows::descriptor_filter_by_log_level().component
            {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|_| {},
                    Some(&|ui| {
                        let Ok(state) = ctx.view_state.downcast_ref::<TextViewState>() else {
                            ui.error_label("Failed to get text log view state");
                            return;
                        };

                        let Ok(levels) = property.component_array_or_fallback::<TextLogLevel>(
                            ctx,
                            TextLogRows::descriptor_filter_by_log_level().component,
                        ) else {
                            ui.error_label("Failed to query text log levels component");
                            return;
                        };

                        let mut new_levels = state
                            .seen_levels
                            .iter()
                            .map(|s| {
                                let level_active = levels.iter().any(|l| l.as_str() == s);
                                (s.clone(), level_active)
                            })
                            .chain(
                                levels
                                    .iter()
                                    .filter(|lvl| !state.seen_levels.contains(lvl.as_str()))
                                    .map(|lvl| (lvl.as_str().to_owned(), true)),
                            )
                            .collect::<Vec<_>>();

                        let mut any_change = false;
                        for (lvl, active) in &mut new_levels {
                            any_change |= ui
                                .re_checkbox(active, level_to_rich_text(ui, lvl))
                                .changed();
                        }

                        if any_change {
                            let log_levels: Vec<_> = new_levels
                                .into_iter()
                                .filter(|(_, active)| *active)
                                .map(|(lvl, _)| TextLogLevel(lvl.into()))
                                .collect();

                            property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &TextLogRows::descriptor_filter_by_log_level(),
                                &log_levels,
                            );
                        }
                    }),
                );
            } else {
                re_view::view_property_component_ui(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                );
            }
        }
    };

    if reflection.fields.len() == 1 {
        sub_prop_ui(ui);
    } else {
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(property.archetype_name.full_name()),
                true,
                LabelContent::new(reflection.display_name),
                sub_prop_ui,
            );
    }
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
