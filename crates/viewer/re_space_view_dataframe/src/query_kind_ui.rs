use std::collections::BTreeSet;

use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, TimeType, Timeline};
use re_types_core::{ComponentName, ComponentNameSet};
use re_ui::{list_item, UiExt};
use re_viewer_context::{TimeDragValue, ViewerContext};

use crate::view_query::QueryKind;

/// Helper to handle the UI for the various query kinds are they are shown to the user.
///
/// This struct is the "UI equivalent" of the [`QueryKind`] enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UiQueryKind {
    LatestAt {
        time: TimeInt,
    },
    TimeRangeAll {
        pov_entity: EntityPath,
        pov_component: ComponentName,
    },
    TimeRange {
        pov_entity: EntityPath,
        pov_component: ComponentName,
        from: TimeInt,
        to: TimeInt,
    },
}

impl UiQueryKind {
    /// Show the UI for the query kind selector.
    pub(crate) fn ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        time_drag_value: &TimeDragValue,
        timeline: &Timeline,
        all_entities: &BTreeSet<EntityPath>,
    ) -> bool {
        let orig_self = self.clone();

        ui.vertical(|ui| {
            //
            // LATEST-AT
            //

            ui.horizontal(|ui| {
                let mut is_latest_at = matches!(self, Self::LatestAt { .. });

                let mut changed = ui
                    .re_radio_value(&mut is_latest_at, true, "Latest-at")
                    .changed();

                if is_latest_at {
                    let mut time = if let Self::LatestAt { time } = self {
                        *time
                    } else {
                        TimeInt::MAX
                    }
                    .into();

                    changed |= match timeline.typ() {
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

            let mut changed = false;
            let mut is_time_range_all = matches!(self, Self::TimeRangeAll { .. });
            changed |= ui
                .re_radio_value(&mut is_time_range_all, true, "From –∞ to +∞")
                .changed();

            //
            // TIME RANGE CUSTOM
            //

            let mut is_time_range_custom = matches!(self, Self::TimeRange { .. });
            if ui
                .re_radio_value(&mut is_time_range_custom, true, "Define time range")
                .changed()
            {
                //TODO: fix that ugly hack
                is_time_range_all = false;
                changed = true;
            }

            //
            // EXTRA UI FOR THE TIME RANGE OPTIONS
            //

            if is_time_range_all || is_time_range_custom {
                ui.spacing_mut().indent = ui.spacing().icon_width + ui.spacing().icon_spacing;
                ui.indent("time_range_custom", |ui| {
                    ui.add_space(-4.0);

                    list_item::list_item_scope(ui, "time_range", |ui| {
                        //
                        // POV ENTITY
                        //

                        let current_entity = match self {
                            Self::TimeRangeAll { pov_entity, .. }
                            | Self::TimeRange { pov_entity, .. } => all_entities
                                .contains(pov_entity)
                                .then(|| pov_entity.clone()),
                            Self::LatestAt { .. } => None,
                        };

                        let mut pov_entity = current_entity
                            .clone()
                            .and_then(|entity| all_entities.contains(&entity).then_some(entity))
                            .or_else(|| all_entities.iter().next().cloned())
                            .unwrap_or_else(|| EntityPath::from("/"));
                        changed |= Some(&pov_entity) != current_entity.as_ref();

                        // let mut pov_entity =
                        //     current_entity.unwrap_or_else(|| EntityPath::from("/"));

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("PoV entity").value_fn(|ui, _| {
                                egui::ComboBox::new("pov_entity", "")
                                    .selected_text(pov_entity.to_string())
                                    .show_ui(ui, |ui| {
                                        for entity in all_entities {
                                            changed |= ui
                                                .selectable_value(
                                                    &mut pov_entity,
                                                    entity.clone(),
                                                    entity.to_string(),
                                                )
                                                .changed();
                                        }
                                    });
                            }),
                        );

                        //
                        // POV COMPONENT
                        //

                        let all_components = ctx
                            .recording_store()
                            .all_components_on_timeline(timeline, &pov_entity)
                            .unwrap_or_default();

                        let current_component = match self {
                            Self::TimeRangeAll { pov_component, .. }
                            | Self::TimeRange { pov_component, .. } => Some(*pov_component),
                            Self::LatestAt { .. } => None,
                        };

                        // The list of suggested components is build as follows:
                        // - consider all indicator components
                        // - for the matching archetypes, take all required components
                        // - keep those that are actually present
                        let suggested_components = || {
                            all_components
                                .iter()
                                .filter_map(|c| {
                                    c.indicator_component_archetype().and_then(
                                        |archetype_short_name| {
                                            ctx.reflection.archetype_reflection_from_short_name(
                                                &archetype_short_name,
                                            )
                                        },
                                    )
                                })
                                .flat_map(|archetype_reflection| {
                                    archetype_reflection
                                        .required_fields()
                                        .map(|field| field.component_name)
                                })
                                .filter(|c| all_components.contains(c))
                                .collect::<ComponentNameSet>()
                        };

                        // If the currently saved component, we auto-switch it to a reasonable one.
                        let mut pov_component = current_component
                            .and_then(|component| {
                                all_components.contains(&component).then_some(component)
                            })
                            .or_else(|| suggested_components().first().copied())
                            .unwrap_or_else(|| ComponentName::from("-"));
                        changed |= Some(pov_component) != current_component;

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("PoV component").value_fn(|ui, _| {
                                egui::ComboBox::new("pov_component", "")
                                    .selected_text(pov_component.short_name())
                                    .show_ui(ui, |ui| {
                                        for component in &all_components {
                                            changed |= ui
                                                .selectable_value(
                                                    &mut pov_component,
                                                    *component,
                                                    component.short_name(),
                                                )
                                                .changed();
                                        }
                                    });
                            }),
                        );

                        //
                        // TIME RANGE BOUNDARIES
                        //

                        if is_time_range_all {
                            if changed {
                                *self = Self::TimeRangeAll {
                                    pov_entity,
                                    pov_component,
                                };
                            }
                        } else {
                            let mut should_display_time_range = false;

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

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Start").value_fn(|ui, _| {
                                    let response = match timeline.typ() {
                                        TimeType::Time => {
                                            time_drag_value
                                                .temporal_drag_value_ui(
                                                    ui,
                                                    &mut from,
                                                    true,
                                                    None,
                                                    ctx.app_options.time_zone,
                                                )
                                                .0
                                        }
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut from, true, None),
                                    };

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                            );

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("End").value_fn(|ui, _| {
                                    let response = match timeline.typ() {
                                        TimeType::Time => {
                                            time_drag_value
                                                .temporal_drag_value_ui(
                                                    ui,
                                                    &mut to,
                                                    true,
                                                    Some(from),
                                                    ctx.app_options.time_zone,
                                                )
                                                .0
                                        }
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut to, true, Some(from)),
                                    };

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                            );

                            if changed {
                                *self = Self::TimeRange {
                                    pov_entity,
                                    pov_component,
                                    from: from.into(),
                                    to: to.into(),
                                };
                            }

                            if should_display_time_range {
                                let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
                                if time_ctrl.timeline() == timeline {
                                    time_ctrl.highlighted_range =
                                        Some(ResolvedTimeRange::new(from, to));
                                }
                            }
                        }
                    });
                });
            }
        });

        *self != orig_self
    }
}

impl From<QueryKind> for UiQueryKind {
    fn from(value: QueryKind) -> Self {
        match value {
            QueryKind::LatestAt { time } => Self::LatestAt { time },
            QueryKind::Range {
                pov_entity,
                pov_component,
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            } => Self::TimeRangeAll {
                pov_entity: pov_entity.clone(),
                pov_component,
            },
            QueryKind::Range {
                pov_entity,
                pov_component,
                from,
                to,
            } => Self::TimeRange {
                pov_entity,
                pov_component,
                from,
                to,
            },
        }
    }
}

impl From<UiQueryKind> for QueryKind {
    fn from(value: UiQueryKind) -> Self {
        match value {
            UiQueryKind::LatestAt { time } => Self::LatestAt { time },
            UiQueryKind::TimeRangeAll {
                pov_entity,
                pov_component,
            } => Self::Range {
                pov_entity,
                pov_component,
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            },
            UiQueryKind::TimeRange {
                pov_entity,
                pov_component,
                from,
                to,
            } => Self::Range {
                pov_entity,
                pov_component,
                from,
                to,
            },
        }
    }
}
