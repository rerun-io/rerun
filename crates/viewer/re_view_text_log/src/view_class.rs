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

    last_columns: Vec<TextLogColumn>,
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
                columns.push(datatypes::TextLogColumn::Body.into());

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

        // We need a custom UI here because we use arrays, whih component UI doesn't support.
        ui.list_item_scope("text_log_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state);
            view_property_ui_columns(&ctx, ui);
            view_property_ui_rows(&ctx, ui);
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

        let view_ctx = self.view_context(ctx, query.view_id, state);
        let columns = columns_property.component_array_or_fallback::<TextLogColumn>(
            &view_ctx,
            TextLogColumns::descriptor_columns().component,
        )?;
        let levels = rows_property.component_array_or_fallback::<TextLogLevel>(
            &view_ctx,
            TextLogRows::descriptor_log_levels().component,
        )?;

        let reset_column_widths = if columns != state.last_columns {
            state.last_columns = columns.clone();
            true
        } else {
            false
        };

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
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    re_tracing::profile_scope!("render table");
                    table_ui(
                        ctx,
                        ui,
                        state,
                        &columns,
                        reset_column_widths,
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
fn table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &TextViewState,
    columns: &[TextLogColumn],
    reset_column_widths: bool,
    entries: &[&Entry],
    scroll_to_row: Option<usize>,
) {
    let tokens = ui.tokens();
    let table_style = re_ui::TableStyle::Dense;

    use egui_extras::Column;

    let (global_timeline, global_time) = (*ctx.time_ctrl.timeline(), ctx.time_ctrl.time_int());

    let mut table_builder = egui_extras::TableBuilder::new(ui)
        .resizable(true)
        .vscroll(true)
        .auto_shrink([false; 2]) // expand to take up the whole View
        .min_scrolled_height(0.0) // we can go as small as we need to be in order to fit within the view!
        .max_scroll_height(f32::INFINITY) // Fill up whole height
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));

    if reset_column_widths {
        table_builder.reset();
    }
    if let Some(scroll_to_row) = scroll_to_row {
        table_builder = table_builder.scroll_to_row(scroll_to_row, Some(egui::Align::Center));
    }

    let mut body_clip_rect = None;
    let mut current_time_y = None; // where to draw the current time indicator cursor

    for col in columns {
        match **col {
            datatypes::TextLogColumn::Timeline(_) | datatypes::TextLogColumn::EntityPath => {
                table_builder = table_builder.column(Column::auto().clip(true).at_least(32.0));
            }
            datatypes::TextLogColumn::LogLevel => {
                table_builder = table_builder.column(Column::auto().at_least(30.0));
            }
            datatypes::TextLogColumn::Body => {
                table_builder = table_builder.column(Column::remainder().at_least(100.0));
            }
        }
    }

    table_builder
        .header(tokens.deprecated_table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            for c in columns {
                header.col(|ui| column_name_ui(ctx, ui, c));
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

                for col in columns {
                    row.col(|ui| {
                        match &col.0 {
                            datatypes::TextLogColumn::Timeline(name) => {
                                let timeline = TimelineName::new(name);
                                let row_time = entry
                                    .timepoint
                                    .get(&timeline)
                                    .map(re_log_types::TimeInt::from)
                                    .unwrap_or(re_log_types::TimeInt::STATIC);
                                item_ui::time_button(ctx, ui, &timeline, row_time);

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
                                    ctx,
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
                            datatypes::TextLogColumn::Body => {
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
}

fn column_kind_name(column: &TextLogColumn) -> &'static str {
    match &column.0 {
        datatypes::TextLogColumn::Timeline(_) => "Timeline",
        datatypes::TextLogColumn::EntityPath => "Entity Path",
        datatypes::TextLogColumn::LogLevel => "Level",
        datatypes::TextLogColumn::Body => "Body",
    }
}

fn column_name_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, column: &TextLogColumn) {
    match &column.0 {
        datatypes::TextLogColumn::Timeline(name) => {
            item_ui::timeline_button(ctx, ui, &TimelineName::new(name));
        }
        _ => {
            ui.strong(column_kind_name(column));
        }
    }
}

fn view_property_ui_columns(ctx: &ViewContext<'_>, ui: &mut egui::Ui) {
    let property = ViewProperty::from_archetype::<TextLogColumns>(
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
                == TextLogColumns::descriptor_columns().component
            {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|_| {},
                    Some(&|ui| {
                        let Ok(mut columns) = property
                            .component_array_or_fallback::<TextLogColumn>(
                                ctx,
                                TextLogColumns::descriptor_columns().component,
                            )
                        else {
                            ui.error_label("Failed to query columns component");
                            return;
                        };
                        let mut any_change = false;
                        let mut remove = Vec::new();
                        let res = egui_dnd::dnd(ui, "text_log_columns_dnd").show(
                            columns.iter_mut().enumerate(),
                            |ui, (idx, column), handle, _state| {
                                ui.horizontal(|ui| {
                                    handle.ui(ui, |ui| {
                                        ui.small_icon(
                                            &re_ui::icons::DND_HANDLE,
                                            Some(ui.visuals().text_color()),
                                        );
                                    });

                                    egui::containers::Sides::new().shrink_left().show(
                                        ui,
                                        |ui| {
                                            column_definition_ui(ctx, ui, column, &mut any_change);
                                        },
                                        |ui| {
                                            if ui
                                                .small_icon_button(
                                                    &re_ui::icons::REMOVE,
                                                    "remove column",
                                                )
                                                .on_hover_text("Remove column")
                                                .clicked()
                                            {
                                                remove.push(idx);
                                            }
                                        },
                                    )
                                });
                            },
                        );

                        if res.is_drag_finished() {
                            res.update_vec(&mut columns);
                            any_change = true;
                        }
                        // Skip removing if we dragged.
                        else if !remove.is_empty() {
                            any_change = true;
                            for i in remove.into_iter().rev() {
                                columns.remove(i);
                            }
                        }

                        if ui
                            .small_icon_button(&re_ui::icons::ADD, "add column")
                            .on_hover_text("Add column")
                            .clicked()
                        {
                            let fallback_columns =
                                re_viewer_context::typed_array_fallback_for::<TextLogColumn>(
                                    &query_ctx,
                                    TextLogColumns::descriptor_columns().component,
                                );

                            let new_column = fallback_columns
                                .into_iter()
                                .find(|c| !columns.contains(c))
                                .or_else(|| columns.last().cloned())
                                .unwrap_or(TextLogColumn(
                                    re_types::datatypes::TextLogColumn::EntityPath,
                                ));

                            columns.push(new_column);
                            any_change = true;
                        }

                        if any_change {
                            property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &TextLogColumns::descriptor_columns(),
                                &columns,
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

fn column_definition_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    column: &mut TextLogColumn,
    any_change: &mut bool,
) {
    egui::ComboBox::from_id_salt("column_types")
        .selected_text(column_kind_name(column))
        .show_ui(ui, |ui| {
            let timeline = if let datatypes::TextLogColumn::Timeline(name) = &column.0 {
                name.as_str().to_owned()
            } else {
                ctx.viewer_ctx.time_ctrl.timeline().name().to_string()
            };
            let mut selectable_value = |value: datatypes::TextLogColumn| {
                let text = column_kind_name(&TextLogColumn(value.clone()));
                *any_change |= ui.selectable_value(&mut column.0, value, text).changed();
            };
            selectable_value(datatypes::TextLogColumn::Timeline(datatypes::Utf8::from(
                timeline,
            )));

            selectable_value(datatypes::TextLogColumn::EntityPath);

            selectable_value(datatypes::TextLogColumn::LogLevel);
            selectable_value(datatypes::TextLogColumn::Body);
        });

    if let datatypes::TextLogColumn::Timeline(name) = &mut column.0 {
        egui::ComboBox::from_id_salt("column_timeline_name")
            .selected_text(name.as_str())
            .show_ui(ui, |ui| {
                for timeline in ctx.recording().times_per_timeline().timelines() {
                    *any_change |= ui
                        .selectable_value(
                            name,
                            datatypes::Utf8::from(timeline.name().as_str()),
                            timeline.name().as_str(),
                        )
                        .changed();
                }
            });
    }
}

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
                == TextLogRows::descriptor_log_levels().component
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
                            TextLogRows::descriptor_log_levels().component,
                        ) else {
                            ui.error_label("Failed to query text log levels component");
                            return;
                        };

                        let mut new_levels = state
                            .seen_levels
                            .iter()
                            .map(|s| {
                                let text_log_level = TextLogLevel::from(s.as_str());
                                let level_active = levels.contains(&text_log_level);
                                (text_log_level, level_active)
                            })
                            .chain(
                                levels
                                    .iter()
                                    .filter(|lvl| !state.seen_levels.contains(lvl.as_str()))
                                    .map(|lvl| (lvl.clone(), true)),
                            )
                            .collect::<Vec<_>>();

                        let mut any_change = false;
                        for (lvl, active) in &mut new_levels {
                            any_change |= ui
                                .re_checkbox(active, level_to_rich_text(ui, lvl.as_str()))
                                .changed();
                        }

                        if any_change {
                            property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &TextLogRows::descriptor_log_levels(),
                                &new_levels
                                    .into_iter()
                                    .filter(|(_, active)| *active)
                                    .map(|(lvl, _)| lvl)
                                    .collect::<Vec<_>>(),
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
