use std::collections::BTreeSet;

use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, TimeType, TimeZone, Timeline};
use re_types_core::{ComponentName, ComponentNameSet};
use re_ui::{list_item, UiExt};
use re_viewer_context::{TimeDragValue, ViewerContext};

/// The query kind for the dataframe view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QueryKind {
    LatestAt {
        time: TimeInt,
    },
    Range {
        pov_entity: EntityPath,
        pov_component: ComponentName,
        from: TimeInt,
        to: TimeInt,
    },
    //TODO(#7067): add selected components
}

impl QueryKind {
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
                };

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
                    *self = Self::LatestAt { time };
                }
            }
        });

        //
        // TIME RANGE CUSTOM
        //

        let mut is_time_range_custom = matches!(self, Self::Range { .. });
        let mut changed = ui
            .re_radio_value(&mut is_time_range_custom, true, "Define time range")
            .changed();

        //
        // EXTRA UI FOR THE TIME RANGE OPTIONS
        //

        if is_time_range_custom {
            ui.spacing_mut().indent = ui.spacing().icon_width + ui.spacing().icon_spacing;
            ui.indent("time_range_custom", |ui| {
                ui.add_space(-4.0);

                list_item::list_item_scope(ui, "time_range", |ui| {
                    //
                    // TIME RANGE BOUNDARIES
                    //

                    let mut should_display_time_range = false;

                    let mut from = if let Self::Range { from, .. } = self {
                        *from
                    } else {
                        TimeInt::MIN
                    };

                    let mut to = if let Self::Range { to, .. } = self {
                        *to
                    } else {
                        TimeInt::MAX
                    };

                    // all time boundaries to not be aligned to the pov entity/component
                    list_item::list_item_scope(ui, "time_range_boundaries", |ui| {
                        let mut reset_from = false;

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Start")
                                .action_button_with_enabled(
                                    &re_ui::icons::RESET,
                                    from != TimeInt::MIN,
                                    || {
                                        reset_from = true;
                                    },
                                )
                                .value_fn(|ui, _| {
                                    let response = time_boundary_ui(
                                        ui,
                                        time_drag_value,
                                        None,
                                        timeline.typ(),
                                        ctx.app_options.time_zone,
                                        &mut from,
                                    );

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                        );

                        if reset_from {
                            from = TimeInt::MIN;
                            changed = true;
                        }

                        let mut reset_to = false;

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("End")
                                .action_button_with_enabled(
                                    &re_ui::icons::RESET,
                                    to != TimeInt::MAX,
                                    || {
                                        reset_to = true;
                                    },
                                )
                                .value_fn(|ui, _| {
                                    let response = time_boundary_ui(
                                        ui,
                                        time_drag_value,
                                        Some(from),
                                        timeline.typ(),
                                        ctx.app_options.time_zone,
                                        &mut to,
                                    );

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                        );

                        if reset_to {
                            to = TimeInt::MAX;
                            changed = true;
                        }
                    });

                    if should_display_time_range {
                        let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
                        if time_ctrl.timeline() == timeline {
                            time_ctrl.highlighted_range = Some(ResolvedTimeRange::new(from, to));
                        }
                    }

                    //
                    // POV ENTITY
                    //

                    let current_entity = match self {
                        Self::Range { pov_entity, .. } => all_entities
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
                        Self::Range { pov_component, .. } => Some(*pov_component),
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
                                c.indicator_component_archetype()
                                    .and_then(|archetype_short_name| {
                                        ctx.reflection.archetype_reflection_from_short_name(
                                            &archetype_short_name,
                                        )
                                    })
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

                    if changed {
                        *self = Self::Range {
                            pov_entity,
                            pov_component,
                            from,
                            to,
                        };
                    }
                });
            });
        }

        *self != orig_self
    }
}

fn time_boundary_ui(
    ui: &mut egui::Ui,
    time_drag_value: &TimeDragValue,
    low_bound_override: Option<TimeInt>,
    timeline_typ: TimeType,
    time_zone: TimeZone,
    time: &mut TimeInt,
) -> egui::Response {
    if *time == TimeInt::MAX {
        let mut response = ui.button("+∞").on_hover_text("Click to edit");
        if response.clicked() {
            *time = time_drag_value.max_time();
            response.mark_changed();
        }
        response
    } else if *time == TimeInt::MIN {
        let mut response = ui.button("–∞").on_hover_text("Click to edit");
        if response.clicked() {
            *time = time_drag_value.min_time();
            response.mark_changed();
        }
        response
    } else {
        match timeline_typ {
            TimeType::Time => {
                time_drag_value
                    .temporal_drag_value_ui(ui, time, true, low_bound_override, time_zone)
                    .0
            }

            TimeType::Sequence => {
                time_drag_value.sequence_drag_value_ui(ui, time, true, low_bound_override)
            }
        }
    }
}
