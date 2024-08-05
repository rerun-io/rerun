use egui::{NumExt, Response};
use re_entity_db::TimeHistogram;
use re_log_types::{TimeInt, TimeType, TimeZone, TimelineName};
use re_types::blueprint::components::DataframeViewMode;
use re_types::blueprint::datatypes::LatestAtQuery;
use re_types::blueprint::{archetypes, components, datatypes};
use re_types_core::{Archetype as _, ComponentName, Loggable as _};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    ComponentFallbackProvider, QueryContext, SpaceViewId, SpaceViewState,
    SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::{entity_path_for_view_property, ViewProperty};
use std::ops::RangeInclusive;

pub(crate) enum QueryMode {
    LatestAt {
        time: TimeInt,
    },
    Range {
        from: TimeInt,
        to: TimeInt,
        pov_components: Vec<ComponentName>,
    },
}

//TODO: fallback to the current timeline is nice but should be saved back to the blueprint such as
// to "freeze" the value

pub(crate) struct Query {
    pub(crate) timeline: TimelineName,
    pub(crate) mode: QueryMode,
    // None == all
    pub(crate) components: Option<Vec<ComponentName>>,
}

impl Query {
    pub(crate) fn try_from_blueprint(
        ctx: &ViewerContext<'_>,
        space_view_id: SpaceViewId,
        fallback_provider: &dyn ComponentFallbackProvider,
        state: &mut dyn SpaceViewState,
    ) -> Result<Self, SpaceViewSystemExecutionError> {
        let property = ViewProperty::from_archetype::<archetypes::DataframeQuery>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            space_view_id,
        );

        let timeline = TimelineName::from(
            property
                .component_or_fallback::<components::Timeline>(ctx, fallback_provider, state)?
                .0
                .as_str(),
        );

        let mode = property.component_or_fallback::<components::DataframeViewMode>(
            ctx,
            fallback_provider,
            state,
        )?;

        let mode = match mode {
            DataframeViewMode::LatestAt => {
                let time = property
                    .component_or_fallback::<components::LatestAtQueries>(
                        ctx,
                        fallback_provider,
                        state,
                    )?
                    .0
                    .into_iter()
                    .find(|q| q.timeline.as_str() == timeline)
                    .map(|q| q.time.into())
                    .unwrap_or_else(|| {
                        ctx.rec_cfg
                            .time_ctrl
                            .read()
                            .time_int()
                            .unwrap_or(TimeInt::MAX)
                    });

                QueryMode::LatestAt { time }
            }
            DataframeViewMode::TimeRange => {
                let (from, to) = property
                    .component_or_fallback::<components::TimeRangeQueries>(
                        ctx,
                        fallback_provider,
                        state,
                    )?
                    .0
                    .into_iter()
                    .find(|q| q.timeline.as_str() == timeline)
                    .map(|q| (q.start.into(), q.end.into()))
                    .unwrap_or((TimeInt::MIN, TimeInt::MAX));

                let pov_components = property
                    .component_or_fallback::<components::PointOfViewComponents>(
                        ctx,
                        fallback_provider,
                        state,
                    )?
                    .0
                     .0
                    .into_iter()
                    .map(|c| ComponentName::from(c.as_str()))
                    .collect();

                QueryMode::Range {
                    from,
                    to,
                    pov_components,
                }
            }
        };

        let components: Vec<_> = property
            .component_or_fallback::<components::QueryComponents>(ctx, fallback_provider, state)?
            .0
             .0
            .into_iter()
            .map(|c| ComponentName::from(c.as_str()))
            .collect();

        Ok(Self {
            timeline,
            mode,
            components: if components.is_empty() {
                None
            } else {
                Some(components)
            },
        })
    }

    pub(crate) fn ui(
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        fallback_provider: &dyn ComponentFallbackProvider,
        state: &mut dyn SpaceViewState,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let name = archetypes::DataframeQuery::name();
        let Some(reflection) = ctx.reflection.archetypes.get(&name) else {
            // The `ArchetypeReflectionMarker` bound should make this impossible.
            re_log::warn_once!("Missing reflection data for archetype {name:?}.");
            //TODO(ab): we should have an error for that
            return Ok(());
        };

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

        let property = ViewProperty::from_archetype::<archetypes::DataframeQuery>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            space_view_id,
        );

        let current_mode = property.component_or_fallback::<components::DataframeViewMode>(
            ctx,
            fallback_provider,
            state,
        )?;

        let timeline_name = property
            .component_or_fallback::<components::Timeline>(ctx, fallback_provider, state)
            .map(|t| t.timeline_name())?;

        let Some(timeline) = ctx
            .recording()
            .timelines()
            .find(|t| t.name() == &timeline_name)
        else {
            re_log::warn_once!("Could not find timeline {:?}.", timeline_name.as_str());
            //TODO(ab): we should have an error for that
            return Ok(());
        };

        let inner_ui = |ui: &mut egui::Ui| -> Result<(), SpaceViewSystemExecutionError> {
            let component_results = ctx.blueprint_db().latest_at(
                ctx.blueprint_query,
                &blueprint_path,
                reflection.fields.iter().map(|field| field.component_name),
            );

            //
            // Timeline
            //

            let component_name = components::Timeline::name();
            ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Timeline").value_fn(
                |ui, _| {
                    ctx.component_ui_registry.singleline_edit_ui(
                        &query_context,
                        ui,
                        ctx.blueprint_db(),
                        &blueprint_path,
                        component_name,
                        component_results
                            .component_batch_raw(&component_name)
                            .as_deref(),
                        fallback_provider,
                    );
                },
            ));

            //
            // Mode
            //

            let component_name = components::DataframeViewMode::name();
            ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Mode").value_fn(
                |ui, _| {
                    ctx.component_ui_registry.singleline_edit_ui(
                        &query_context,
                        ui,
                        ctx.blueprint_db(),
                        &blueprint_path,
                        component_name,
                        component_results
                            .component_batch_raw(&component_name)
                            .as_deref(),
                        fallback_provider,
                    );
                },
            ));

            let time_spec = if let Some(time_histogram) = ctx.recording().time_histogram(&timeline)
            {
                TimelineSpec::from_time_histogram(time_histogram)
            } else {
                // shouldn't happen, `timeline` existence was already checked
                TimelineSpec::from_time_range(0..=0)
            };

            match current_mode {
                DataframeViewMode::LatestAt => {
                    //
                    // Latest At time
                    // TODO(ab): we can't use edit ui because we dont have the required context
                    //           there, aka the currently chosen timeline.
                    //

                    let mut latest_at_queries = property
                        .component_or_fallback::<components::LatestAtQueries>(
                            ctx,
                            fallback_provider,
                            state,
                        )?;

                    let mut latest_at_query = latest_at_queries
                        .query_for_timeline(timeline_name.as_str())
                        .cloned()
                        .unwrap_or_else(|| datatypes::LatestAtQuery {
                            timeline: timeline_name.as_str().into(),
                            time: TimeInt::MAX.into(),
                        });

                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("At time").value_fn(|ui, _| {
                            let resp = match timeline.typ() {
                                TimeType::Time => {
                                    time_spec
                                        .temporal_drag_value(
                                            ui,
                                            &mut latest_at_query.time,
                                            true,
                                            None,
                                            ctx.app_options.time_zone,
                                        )
                                        .0
                                }
                                TimeType::Sequence => time_spec.sequence_drag_value(
                                    ui,
                                    &mut latest_at_query.time,
                                    true,
                                    None,
                                ),
                            };

                            if resp.changed() {
                                latest_at_queries.set_query_for_timeline(
                                    timeline_name.as_str(),
                                    Some(latest_at_query),
                                );
                                ctx.save_blueprint_component(&blueprint_path, &latest_at_queries);
                            }
                        }),
                    );
                }
                DataframeViewMode::TimeRange => {
                    //
                    // Range times

                    let mut time_range_queries = property
                        .component_or_fallback::<components::TimeRangeQueries>(
                            ctx,
                            fallback_provider,
                            state,
                        )?;

                    let mut time_range_query = time_range_queries
                        .query_for_timeline(timeline_name.as_str())
                        .cloned()
                        .unwrap_or_else(|| datatypes::TimeRangeQuery {
                            timeline: timeline_name.as_str().into(),
                            start: TimeInt::MIN.into(),
                            end: TimeInt::MAX.into(),
                        });

                    let mut changed = false;
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("From").value_fn(|ui, _| {
                            let resp = match timeline.typ() {
                                TimeType::Time => {
                                    time_spec
                                        .temporal_drag_value(
                                            ui,
                                            &mut time_range_query.start,
                                            true,
                                            None,
                                            ctx.app_options.time_zone,
                                        )
                                        .0
                                }
                                TimeType::Sequence => time_spec.sequence_drag_value(
                                    ui,
                                    &mut time_range_query.start,
                                    true,
                                    None,
                                ),
                            };

                            changed |= resp.changed();
                        }),
                    );

                    let end_response = ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("To").value_fn(|ui, _| {
                            let resp = match timeline.typ() {
                                TimeType::Time => {
                                    time_spec
                                        .temporal_drag_value(
                                            ui,
                                            &mut time_range_query.end,
                                            true,
                                            None,
                                            ctx.app_options.time_zone,
                                        )
                                        .0
                                }
                                TimeType::Sequence => time_spec.sequence_drag_value(
                                    ui,
                                    &mut time_range_query.end,
                                    true,
                                    None,
                                ),
                            };

                            changed |= resp.changed();
                        }),
                    );

                    if changed {
                        time_range_queries
                            .set_query_for_timeline(timeline_name.as_str(), Some(time_range_query));
                        ctx.save_blueprint_component(&blueprint_path, &time_range_queries);
                    }

                    //
                    // Pov components
                    //
                    let component_name = components::PointOfViewComponents::name();
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("Point of view components").value_fn(
                            |ui, _| {
                                ctx.component_ui_registry.singleline_edit_ui(
                                    &query_context,
                                    ui,
                                    ctx.blueprint_db(),
                                    &blueprint_path,
                                    component_name,
                                    component_results
                                        .component_batch_raw(&component_name)
                                        .as_deref(),
                                    fallback_provider,
                                );
                            },
                        ),
                    );
                }
            }

            //
            // Query components
            //

            let component_name = components::QueryComponents::name();
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Query components").value_fn(|ui, _| {
                    ctx.component_ui_registry.singleline_edit_ui(
                        &query_context,
                        ui,
                        ctx.blueprint_db(),
                        &blueprint_path,
                        component_name,
                        component_results
                            .component_batch_raw(&component_name)
                            .as_deref(),
                        fallback_provider,
                    );
                }),
            );

            Ok(())
        };

        let result = ui
            .list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id("view_query"),
                true,
                list_item::LabelContent::new("Query"),
                |ui| inner_ui(ui),
            );

        result.body_response.map(|r| r.inner).unwrap_or(Ok(()))
    }
}

// =================================================================================================
// TODO: this is copied from visible_time_range_ui.rs. It should be extracted and cleaned-up. Also
//       there is time histogram stuff here that is bound to be removed/fixed.

/// Compute and store various information about a timeline related to how the UI should behave.
#[derive(Debug)]
struct TimelineSpec {
    /// Actual range of logged data on the timelines (excluding timeless data).
    range: RangeInclusive<i64>,

    /// For timelines with large offsets (e.g. `log_time`), this is a rounded time just before the
    /// first logged data, which can be used as offset in the UI.
    base_time: Option<i64>,

    // used only for temporal timelines
    /// For temporal timelines, this is a nice unit factor to use.
    unit_factor: i64,

    /// For temporal timelines, this is the unit symbol to display.
    unit_symbol: &'static str,

    /// This is a nice range of absolute times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    abs_range: RangeInclusive<i64>,

    /// This is a nice range of relative times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    rel_range: RangeInclusive<i64>,
}

impl TimelineSpec {
    fn from_time_histogram(times: &TimeHistogram) -> Self {
        Self::from_time_range(
            times.min_key().unwrap_or_default()..=times.max_key().unwrap_or_default(),
        )
    }

    fn from_time_range(range: RangeInclusive<i64>) -> Self {
        let span = range.end() - range.start();
        let base_time = time_range_base_time(*range.start(), span);
        let (unit_symbol, unit_factor) = unit_from_span(span);

        // `abs_range` is used by the DragValue when editing an absolute time, its bound expended to
        // nearest unit to minimize glitches.
        let abs_range =
            round_down(*range.start(), unit_factor)..=round_up(*range.end(), unit_factor);

        // `rel_range` is used by the DragValue when editing a relative time offset. It must have
        // enough margin either side to accommodate for all possible values of current time.
        let rel_range = round_down(-span, unit_factor)..=round_up(2 * span, unit_factor);

        Self {
            range,
            base_time,
            unit_factor,
            unit_symbol,
            abs_range,
            rel_range,
        }
    }

    fn sequence_drag_value(
        &self,
        ui: &mut egui::Ui,
        value: &mut re_types_core::datatypes::TimeInt,
        absolute: bool,
        low_bound_override: Option<re_types_core::datatypes::TimeInt>,
    ) -> Response {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        // speed must be computed before messing with time_range for consistency
        let span = time_range.end() - time_range.start();
        let speed = (span as f32 * 0.005).at_least(1.0);

        if let Some(low_bound_override) = low_bound_override {
            time_range = low_bound_override.0.at_least(*time_range.start())..=*time_range.end();
        }

        ui.add(
            egui::DragValue::new(&mut value.0)
                .range(time_range)
                .speed(speed),
        )
    }

    /// Show a temporal drag value.
    ///
    /// Feature rich:
    /// - scale to the proper units
    /// - display the base time if any
    /// - etc.
    ///
    /// Returns a tuple of the [`egui::DragValue`]'s [`egui::Response`], and the base time label's
    /// [`egui::Response`], if any.
    fn temporal_drag_value(
        &self,
        ui: &mut egui::Ui,
        value: &mut re_types_core::datatypes::TimeInt,
        absolute: bool,
        low_bound_override: Option<re_types_core::datatypes::TimeInt>,
        time_zone_for_timestamps: TimeZone,
    ) -> (Response, Option<Response>) {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        let factor = self.unit_factor as f32;
        let offset = if absolute {
            self.base_time.unwrap_or(0)
        } else {
            0
        };

        // speed must be computed before messing with time_range for consistency
        let speed = (time_range.end() - time_range.start()) as f32 / factor * 0.005;

        if let Some(low_bound_override) = low_bound_override {
            time_range = low_bound_override.0.at_least(*time_range.start())..=*time_range.end();
        }

        let mut time_unit = (value.0 - offset) as f32 / factor;

        let time_range = (*time_range.start() - offset) as f32 / factor
            ..=(*time_range.end() - offset) as f32 / factor;

        let base_time_response = if absolute {
            self.base_time.map(|base_time| {
                ui.label(format!(
                    "{} + ",
                    TimeType::Time.format(
                        re_types_core::datatypes::TimeInt(base_time),
                        time_zone_for_timestamps
                    )
                ))
            })
        } else {
            None
        };

        let drag_value_response = ui.add(
            egui::DragValue::new(&mut time_unit)
                .range(time_range)
                .speed(speed)
                .suffix(self.unit_symbol),
        );

        *value = re_types_core::datatypes::TimeInt((time_unit * factor).round() as i64 + offset);

        (drag_value_response, base_time_response)
    }
}

fn unit_from_span(span: i64) -> (&'static str, i64) {
    if span / 1_000_000_000 > 0 {
        ("s", 1_000_000_000)
    } else if span / 1_000_000 > 0 {
        ("ms", 1_000_000)
    } else if span / 1_000 > 0 {
        ("μs", 1_000)
    } else {
        ("ns", 1)
    }
}

/// Value of the start time over time span ratio above which an explicit offset is handled.
static SPAN_TO_START_TIME_OFFSET_THRESHOLD: i64 = 10;

fn time_range_base_time(min_time: i64, span: i64) -> Option<i64> {
    if min_time <= 0 {
        return None;
    }

    if span.saturating_mul(SPAN_TO_START_TIME_OFFSET_THRESHOLD) < min_time {
        let factor = if span / 1_000_000 > 0 {
            1_000_000_000
        } else if span / 1_000 > 0 {
            1_000_000
        } else {
            1_000
        };

        Some(min_time - (min_time % factor))
    } else {
        None
    }
}

fn round_down(value: i64, factor: i64) -> i64 {
    value - (value.rem_euclid(factor))
}

fn round_up(value: i64, factor: i64) -> i64 {
    let val = round_down(value, factor);

    if val == value {
        val
    } else {
        val + factor
    }
}
