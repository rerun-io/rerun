use std::collections::BTreeSet;

use re_chunk_store::ColumnDescriptor;
use re_log_types::{
    EntityPath, ResolvedTimeRange, TimeInt, TimeType, TimeZone, Timeline, TimelineName,
};
use re_types_core::{ComponentName, ComponentNameSet};
use re_ui::{list_item, UiExt};
use re_viewer_context::{SpaceViewId, SpaceViewSystemExecutionError, TimeDragValue, ViewerContext};

use crate::view_query_v2::{EventColumn, QueryV2};

// UI implementation
impl QueryV2 {
    pub(super) fn timeline_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let mut timeline_name = *timeline.name();
        egui::Grid::new("dataframe_view_query_ui_timeline")
            .num_columns(2)
            .spacing(egui::vec2(8.0, 10.0))
            .show(ui, |ui| -> Result<_, SpaceViewSystemExecutionError> {
                ui.grid_left_hand_label("Timeline");

                if edit_timeline_name(ctx, ui, &mut timeline_name).changed() {
                    self.set_timeline_name(ctx, &timeline_name);
                }

                Ok(())
            })
            .inner
    }

    pub(super) fn filter_range_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let time_drag_value = if let Some(times) = ctx.recording().time_histogram(timeline) {
            TimeDragValue::from_time_histogram(times)
        } else {
            // This should never happen because `timeline` is guaranteed to be valid by `Self::timeline()`
            TimeDragValue::from_time_range(0..=0)
        };

        ui.label("Filter rows by time range:");
        let (mut start, mut end) = self.range_filter()?;

        let mut changed = false;
        let mut should_display_time_range = false;
        list_item::list_item_scope(ui, "dataframe_view_query_ui_range_filter", |ui| {
            let mut reset_start = false;

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Start")
                    .action_button_with_enabled(&re_ui::icons::RESET, start != TimeInt::MIN, || {
                        reset_start = true;
                    })
                    .value_fn(|ui, _| {
                        let response = time_boundary_ui(
                            ui,
                            &time_drag_value,
                            None,
                            timeline.typ(),
                            ctx.app_options.time_zone,
                            &mut start,
                        );

                        changed |= response.changed();
                        should_display_time_range |=
                            response.hovered() || response.dragged() || response.has_focus();
                    }),
            );

            if reset_start {
                start = TimeInt::MIN;
                changed = true;
            }

            let mut reset_to = false;

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("End")
                    .action_button_with_enabled(&re_ui::icons::RESET, end != TimeInt::MAX, || {
                        reset_to = true;
                    })
                    .value_fn(|ui, _| {
                        let response = time_boundary_ui(
                            ui,
                            &time_drag_value,
                            Some(start),
                            timeline.typ(),
                            ctx.app_options.time_zone,
                            &mut end,
                        );

                        changed |= response.changed();
                        should_display_time_range |=
                            response.hovered() || response.dragged() || response.has_focus();
                    }),
            );

            if reset_to {
                end = TimeInt::MAX;
                changed = true;
            }
        });

        if changed {
            self.set_range_filter(ctx, start, end);
        }

        if should_display_time_range {
            let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
            if time_ctrl.timeline() == timeline {
                time_ctrl.highlighted_range = Some(ResolvedTimeRange::new(start, end));
            }
        }

        Ok(())
    }

    pub(super) fn filter_event_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        //
        // Read stuff
        //

        let mut filter_by_event_active = self.filter_by_event_active()?;

        let original_event_column = self.filter_event_column()?;
        let (event_entity, event_component) =
            original_event_column.clone().map_or((None, None), |col| {
                (Some(col.entity_path), Some(col.component_name))
            });

        //
        // Filter active?
        //

        if ui
            .re_checkbox(&mut filter_by_event_active, "Filter by event from:")
            .changed()
        {
            self.set_filter_by_event_active(ctx, filter_by_event_active);
        }

        //
        // Event entity
        //

        let all_entities = all_pov_entities_for_space_view(ctx, space_view_id, timeline);

        let mut event_entity = event_entity
            .and_then(|entity| all_entities.contains(&entity).then_some(entity))
            .or_else(|| all_entities.iter().next().cloned())
            .unwrap_or_else(|| EntityPath::from("/"));

        ui.add_enabled_ui(filter_by_event_active, |ui| {
            ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Entity").value_fn(
                |ui, _| {
                    egui::ComboBox::new("pov_entity", "")
                        .selected_text(event_entity.to_string())
                        .show_ui(ui, |ui| {
                            for entity in all_entities {
                                let label = entity.to_string();
                                ui.selectable_value(&mut event_entity, entity, label);
                            }
                        });
                },
            ));
        });

        //
        // Event component
        //

        let all_components = ctx
            .recording_store()
            .all_components_on_timeline(timeline, &event_entity)
            .unwrap_or_default();

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
                            ctx.reflection
                                .archetype_reflection_from_short_name(&archetype_short_name)
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
        let mut event_component = event_component
            .and_then(|component| all_components.contains(&component).then_some(component))
            .or_else(|| suggested_components().first().copied())
            .unwrap_or_else(|| ComponentName::from("-"));

        ui.add_enabled_ui(filter_by_event_active, |ui| {
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Component").value_fn(|ui, _| {
                    egui::ComboBox::new("pov_component", "")
                        .selected_text(event_component.short_name())
                        .show_ui(ui, |ui| {
                            for component in all_components {
                                let label = component.short_name();
                                ui.selectable_value(&mut event_component, component, label);
                            }
                        });
                }),
            );
        });

        //
        // Save event if changed
        //

        let event_column = EventColumn {
            entity_path: event_entity,
            component_name: event_component,
        };

        if original_event_column.as_ref() != Some(&event_column) {
            self.set_filter_event_column(ctx, event_column);
        }

        Ok(())
    }

    pub(super) fn column_visibility_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        schema: &[ColumnDescriptor],
    ) -> Result<(), SpaceViewSystemExecutionError> {
        Ok(())
    }

    pub(super) fn latest_at_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        ui.label("Empty cells:");

        let mut latest_at = self.latest_at()?;
        let changed = {
            ui.re_radio_value(&mut latest_at, false, "Leave empty")
                .changed()
        } | {
            ui.re_radio_value(&mut latest_at, true, "Fill with latest-at values")
                .changed()
        };

        if changed {
            self.set_latest_at(ctx, latest_at);
        }

        Ok(())
    }
}

/// Gather all entities that can meaningfully be used as point-of-view for this view.
///
/// Meaning:
/// - the entity is part of this view
/// - the entity has any component on the chosen timeline
fn all_pov_entities_for_space_view(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    timeline: &Timeline,
) -> BTreeSet<EntityPath> {
    let mut all_entities = BTreeSet::new();
    ctx.lookup_query_result(space_view_id)
        .tree
        .visit(&mut |node| {
            if !node.data_result.tree_prefix_only {
                let comp_for_entity = ctx
                    .recording_store()
                    .all_components_on_timeline(timeline, &node.data_result.entity_path);
                if comp_for_entity.is_some_and(|components| !components.is_empty()) {
                    all_entities.insert(node.data_result.entity_path.clone());
                }
            }
            true
        });

    all_entities
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

fn edit_timeline_name(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut TimelineName,
) -> egui::Response {
    let mut changed = false;
    let mut combobox_response = egui::ComboBox::from_id_salt(&value)
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for timeline in ctx.recording().timelines() {
                let response =
                    ui.selectable_value(value, *timeline.name(), timeline.name().as_str());

                changed |= response.changed();
            }
        });

    if changed {
        combobox_response.response.mark_changed();
    }

    combobox_response.response
}
