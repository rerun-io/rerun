use re_log_types::{TimeInt, TimeType, TimelineName};
use re_types::blueprint::components::QueryKind;
use re_types::blueprint::{archetypes, components, datatypes};
use re_types_core::{Archetype as _, Loggable as _};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    QueryContext, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError, TimeDragValue,
    ViewerContext,
};
use re_viewport_blueprint::{entity_path_for_view_property, ViewProperty};

use crate::visualizer_system::EmptySystem;

/// The query mode for the dataframe view.
#[derive(Debug, Clone, Copy)]
pub(crate) enum QueryMode {
    LatestAt {
        time: TimeInt,
    },
    Range {
        from: TimeInt,
        to: TimeInt,
        //TODO(#7072): add PoV components
    },
    //TODO(#7067): add selected components
}

/// Helper for handling the dataframe view query blueprint.
pub(crate) enum Query {
    FollowTimeline,

    Override {
        timeline: TimelineName,
        mode: QueryMode,
    },
}

impl Query {
    pub(crate) fn try_from_blueprint(
        ctx: &ViewerContext<'_>,
        space_view_id: SpaceViewId,
    ) -> Result<Self, SpaceViewSystemExecutionError> {
        let property = ViewProperty::from_archetype::<archetypes::DataframeQuery>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            space_view_id,
        );

        // The presence (or not) of the timeline component determines if the view should follow the
        // time panel timeline/latest at query, or override it.
        let Some(timeline) = property
            .component_or_empty::<components::Timeline>()?
            .map(|t| t.timeline_name())
        else {
            return Ok(Self::FollowTimeline);
        };

        let mode = property
            .component_or_empty::<components::QueryKind>()?
            .unwrap_or(QueryKind::LatestAt);

        let mode = match mode {
            QueryKind::LatestAt => {
                let time = property
                    .component_or_empty::<components::LatestAtQueries>()?
                    .unwrap_or_default()
                    .query_for_timeline(&timeline)
                    .map_or(TimeInt::MAX, |q| q.time.into());

                QueryMode::LatestAt { time }
            }
            QueryKind::TimeRange => {
                let (from, to) = property
                    .component_or_empty::<components::TimeRangeQueries>()?
                    .unwrap_or_default()
                    .query_for_timeline(&timeline)
                    .map_or((TimeInt::MIN, TimeInt::MAX), |q| {
                        (q.start.into(), q.end.into())
                    });

                QueryMode::Range { from, to }
            }
        };

        Ok(Self::Override { timeline, mode })
    }

    /// Get the timeline name for the query
    #[inline]
    pub(crate) fn timeline_name(&self, ctx: &ViewerContext<'_>) -> TimelineName {
        match self {
            Self::FollowTimeline => *ctx.rec_cfg.time_ctrl.read().timeline().name(),
            Self::Override { timeline, .. } => *timeline,
        }
    }

    /// Get the mode for the query
    #[inline]
    pub(crate) fn mode(&self, ctx: &ViewerContext<'_>) -> QueryMode {
        match self {
            Self::FollowTimeline => {
                let time_ctrl = ctx.rec_cfg.time_ctrl.read();
                QueryMode::LatestAt {
                    time: time_ctrl.time_int().unwrap_or(TimeInt::MAX),
                }
            }
            Self::Override { mode, .. } => *mode,
        }
    }

    /// Save the query mode for the given timeline to the blueprint.
    pub(crate) fn save_mode_for_timeline(
        ctx: &ViewerContext<'_>,
        space_view_id: SpaceViewId,
        timeline_name: &TimelineName,
        query_mode: &QueryMode,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let property = ViewProperty::from_archetype::<archetypes::DataframeQuery>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            space_view_id,
        );

        match query_mode {
            QueryMode::LatestAt { time } => {
                let mut latest_at_queries = property
                    .component_or_empty::<components::LatestAtQueries>()?
                    .unwrap_or_default();

                latest_at_queries.set_query_for_timeline(datatypes::LatestAtQuery {
                    timeline: timeline_name.as_str().into(),
                    time: (*time).into(),
                });

                ctx.save_blueprint_component(&property.blueprint_store_path, &latest_at_queries);
                ctx.save_blueprint_component(&property.blueprint_store_path, &QueryKind::LatestAt);
            }
            QueryMode::Range { from, to } => {
                let mut time_range_queries = property
                    .component_or_empty::<components::TimeRangeQueries>()?
                    .unwrap_or_default();

                time_range_queries.set_query_for_timeline(datatypes::TimeRangeQuery {
                    timeline: timeline_name.as_str().into(),
                    start: (*from).into(),
                    end: (*to).into(),
                });

                ctx.save_blueprint_component(&property.blueprint_store_path, &time_range_queries);
                ctx.save_blueprint_component(&property.blueprint_store_path, &QueryKind::TimeRange);
            }
        };

        Ok(())
    }
}

pub(crate) fn query_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &dyn SpaceViewState,
    space_view_id: SpaceViewId,
) -> Result<(), SpaceViewSystemExecutionError> {
    let property = ViewProperty::from_archetype::<archetypes::DataframeQuery>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        space_view_id,
    );

    // The existence of a timeline component determines if we are in follow time panel or
    // override mode.
    let timeline_component = property.component_or_empty::<components::Timeline>()?;
    let timeline_name = timeline_component.as_ref().map(|t| t.timeline_name());

    let timeline = timeline_name.and_then(|timeline_name| {
        ctx.recording()
            .timelines()
            .find(|t| t.name() == &timeline_name)
    });

    let mut override_query = timeline.is_some();
    let changed = ui.selectable_toggle(|ui| {
        ui.selectable_value(&mut override_query, false, "Follow timeline")
            .changed()
            || ui
                .selectable_value(&mut override_query, true, "Override")
                .changed()
    });

    if changed {
        if override_query {
            let time_ctrl = ctx.rec_cfg.time_ctrl.read();
            let timeline = time_ctrl.timeline();

            // UX least surprising behavior: when switching from "follow" to "override", we ensure
            // that the override configuration defaults to the current timeline configuration, so
            // the table content remains stable.
            property.save_blueprint_component(
                ctx,
                &components::Timeline::from(timeline.name().as_str()),
            );
            Query::save_mode_for_timeline(
                ctx,
                space_view_id,
                timeline.name(),
                &QueryMode::LatestAt {
                    time: time_ctrl.time_int().unwrap_or(TimeInt::MAX),
                },
            )?;
        } else {
            property.reset_blueprint_component::<components::Timeline>(ctx);
        }
    }

    if override_query {
        override_ui(ctx, ui, state, space_view_id, &property)
    } else {
        Ok(())
    }
}

fn override_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &dyn SpaceViewState,
    space_view_id: SpaceViewId,
    property: &ViewProperty<'_>,
) -> Result<(), SpaceViewSystemExecutionError> {
    let name = archetypes::DataframeQuery::name();
    let Some(reflection) = ctx.reflection.archetypes.get(&name) else {
        // The `ArchetypeReflectionMarker` bound should make this impossible.
        re_log::warn_once!("Missing reflection data for archetype {name:?}.");
        return Ok(());
    };

    let timeline = property
        .component_or_empty::<components::Timeline>()?
        .map(|t| t.timeline_name())
        .and_then(|timeline_name| {
            ctx.recording()
                .timelines()
                .find(|t| t.name() == &timeline_name)
                .copied()
        })
        .unwrap_or(*ctx.rec_cfg.time_ctrl.read().timeline());
    let timeline_name = timeline.name();

    let blueprint_path =
        entity_path_for_view_property(space_view_id, ctx.blueprint_db().tree(), name);
    let query_context = QueryContext {
        viewer_ctx: ctx,
        target_entity_path: &blueprint_path,
        archetype_name: Some(name),
        query: ctx.blueprint_query,
        view_state: state,
        view_ctx: None,
    };

    let component_results = ctx.blueprint_db().latest_at(
        ctx.blueprint_query,
        &blueprint_path,
        reflection.fields.iter().map(|field| field.component_name),
    );

    ui.selection_grid("dataframe_view_query_ui")
        .show(ui, |ui| {
            ui.grid_left_hand_label("Timeline");

            let component_name = components::Timeline::name();

            //TODO(ab, andreas): ideally it would be _much_ easier to call if a fallback provider is not needed
            ctx.component_ui_registry.singleline_edit_ui(
                &query_context,
                ui,
                ctx.blueprint_db(),
                &blueprint_path,
                component_name,
                component_results
                    .component_batch_raw(&component_name)
                    .as_deref(),
                // we don't need to provide a fallback here as the timeline should be present by definition
                &EmptySystem {},
            );

            ui.end_row();

            ui.grid_left_hand_label("Showing");

            let query = Query::try_from_blueprint(ctx, space_view_id)?;
            let mut ui_query_mode: UiQueryMode = query.mode(ctx).into();
            let time_drag_value = if let Some(times) = ctx.recording().time_histogram(&timeline) {
                TimeDragValue::from_time_histogram(times)
            } else {
                TimeDragValue::from_time_range(0..=0)
            };
            let changed = ui_query_mode.ui(ctx, ui, &time_drag_value, timeline.typ());
            if changed {
                Query::save_mode_for_timeline(
                    ctx,
                    space_view_id,
                    timeline_name,
                    &ui_query_mode.into(),
                )?;
            }

            Ok(())
        })
        .inner
}

// --

/// Helper to handle the various query modes as they are offered in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiQueryMode {
    LatestAt { time: TimeInt },
    TimeRangeAll,
    TimeRange { from: TimeInt, to: TimeInt },
}

impl UiQueryMode {
    /// Show the UI for the query mode selector.
    fn ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        time_drag_value: &TimeDragValue,
        time_type: TimeType,
    ) -> bool {
        let orig_self = *self;

        ui.vertical(|ui| {
            //
            // LATEST AT
            //

            ui.horizontal(|ui| {
                let mut is_latest_at = matches!(self, Self::LatestAt { .. });

                let mut changed = ui
                    .re_radio_value(&mut is_latest_at, true, "Latest at")
                    .changed();

                if is_latest_at {
                    let mut time = if let Self::LatestAt { time } = self {
                        *time
                    } else {
                        TimeInt::MAX
                    }
                    .into();

                    changed |= match time_type {
                        TimeType::Time => time_drag_value
                            .temporal_drag_value_ui(
                                ui,
                                &mut time,
                                true,
                                None,
                                ctx.app_options.time_zone,
                            )
                            .0
                            .changed(),
                        TimeType::Sequence => time_drag_value
                            .sequence_drag_value_ui(ui, &mut time, true, None)
                            .changed(),
                    };

                    if changed {
                        *self = Self::LatestAt { time: time.into() };
                    }
                }
            });

            //
            // TIME RANGE ALL
            //

            ui.horizontal(|ui| {
                let mut is_time_range_all = matches!(self, Self::TimeRangeAll);
                if ui
                    .re_radio_value(&mut is_time_range_all, true, "From –∞ to +∞")
                    .changed()
                    && is_time_range_all
                {
                    *self = Self::TimeRangeAll;
                }
            });

            //
            // TIME RANGE CUSTOM
            //

            ui.vertical(|ui| {
                let mut is_time_range_custom = matches!(self, Self::TimeRange { .. });
                let mut changed = ui
                    .re_radio_value(&mut is_time_range_custom, true, "Define time range")
                    .changed();

                if is_time_range_custom {
                    ui.spacing_mut().indent = ui.spacing().icon_width + ui.spacing().icon_spacing;
                    ui.indent("time_range_custom", |ui| {
                        let mut from = if let Self::TimeRange { from, .. } = self {
                            (*from).into()
                        } else {
                            (*time_drag_value.range.start()).into()
                        };

                        let mut to = if let Self::TimeRange { to, .. } = self {
                            (*to).into()
                        } else {
                            (*time_drag_value.range.end()).into()
                        };

                        list_item::list_item_scope(ui, "time_range_custom_scope", |ui| {
                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Start").value_fn(|ui, _| {
                                    changed |= match time_type {
                                        TimeType::Time => time_drag_value
                                            .temporal_drag_value_ui(
                                                ui,
                                                &mut from,
                                                true,
                                                None,
                                                ctx.app_options.time_zone,
                                            )
                                            .0
                                            .changed(),
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut from, true, None)
                                            .changed(),
                                    };
                                }),
                            );

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("End").value_fn(|ui, _| {
                                    changed |= match time_type {
                                        TimeType::Time => time_drag_value
                                            .temporal_drag_value_ui(
                                                ui,
                                                &mut to,
                                                true,
                                                Some(from),
                                                ctx.app_options.time_zone,
                                            )
                                            .0
                                            .changed(),
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut to, true, Some(from))
                                            .changed(),
                                    };
                                }),
                            );
                        });

                        if changed {
                            *self = Self::TimeRange {
                                from: from.into(),
                                to: to.into(),
                            };
                        }
                    });
                }
            });
        });

        *self != orig_self
    }
}

impl From<QueryMode> for UiQueryMode {
    fn from(value: QueryMode) -> Self {
        match value {
            QueryMode::LatestAt { time } => Self::LatestAt { time },
            QueryMode::Range {
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            } => Self::TimeRangeAll,
            QueryMode::Range { from, to } => Self::TimeRange { from, to },
        }
    }
}

impl From<UiQueryMode> for QueryMode {
    fn from(value: UiQueryMode) -> Self {
        match value {
            UiQueryMode::LatestAt { time } => Self::LatestAt { time },
            UiQueryMode::TimeRangeAll => Self::Range {
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            },
            UiQueryMode::TimeRange { from, to } => Self::Range { from, to },
        }
    }
}
