use std::collections::{BTreeSet, HashSet};

use re_chunk_store::{ColumnDescriptor, ColumnSelector};
use re_log_types::{
    EntityPath, ResolvedTimeRange, TimeInt, TimeType, TimeZone, Timeline, TimelineName,
};
use re_types::blueprint::components;
use re_types_core::{ComponentName, ComponentNameSet};
use re_ui::{list_item, UiExt};
use re_viewer_context::{SpaceViewId, SpaceViewSystemExecutionError, TimeDragValue, ViewerContext};

use crate::view_query::Query;

// UI implementation
impl Query {
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
                    self.save_timeline_name(ctx, &timeline_name);
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
            debug_assert!(false, "This should never happen because `timeline` is guaranteed to be valid by `Self::timeline()`");
            TimeDragValue::from_time_range(0..=0)
        };

        ui.label("Filter rows by time range:");
        let range = self.filter_by_range()?;
        let (mut start, mut end) = (range.min(), range.max());

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
            self.save_filter_by_range(ctx, ResolvedTimeRange::new(start, end));
        }

        if should_display_time_range {
            let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
            if time_ctrl.timeline() == timeline {
                time_ctrl.highlighted_range = Some(ResolvedTimeRange::new(start, end));
            }
        }

        Ok(())
    }

    pub(super) fn filter_is_not_null_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        //
        // Read stuff
        //

        let original_filter_is_not_null = self.filter_is_not_null_raw()?;

        let (mut active, filter_entity, filter_component) = original_filter_is_not_null
            .as_ref()
            .map(|filter| {
                (
                    filter.active(),
                    Some(filter.entity_path()),
                    Some(filter.component_name()),
                )
            })
            .unwrap_or((false, None, None));

        //
        // Filter active?
        //

        ui.re_checkbox(&mut active, "Filter rows where column is not null:");

        //
        // Filter entity
        //

        let all_entities = all_pov_entities_for_space_view(ctx, space_view_id, timeline);

        let mut filter_entity = filter_entity
            .and_then(|entity| all_entities.contains(&entity).then_some(entity))
            .or_else(|| all_entities.iter().next().cloned())
            .unwrap_or_else(|| EntityPath::from("/"));

        //
        // Filter component
        //

        let all_components = ctx
            .recording_engine()
            .store()
            .all_components_on_timeline(timeline, &filter_entity)
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
        let mut filter_component = filter_component
            .and_then(|component| all_components.contains(&component).then_some(component))
            .or_else(|| suggested_components().first().copied())
            .unwrap_or_else(|| ComponentName::from("-"));

        //
        // UI for filter entity and component
        //

        ui.add_enabled_ui(active, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Entity").value_fn(
                |ui, _| {
                    egui::ComboBox::new("pov_entity", "")
                        .selected_text(filter_entity.to_string())
                        .show_ui(ui, |ui| {
                            for entity in all_entities {
                                let label = entity.to_string();
                                ui.selectable_value(&mut filter_entity, entity, label);
                            }
                        });
                },
            ));

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Component").value_fn(|ui, _| {
                    egui::ComboBox::new("pov_component", "")
                        .selected_text(filter_component.short_name())
                        .show_ui(ui, |ui| {
                            for component in all_components {
                                let label = component.short_name();
                                ui.selectable_value(&mut filter_component, component, label);
                            }
                        });
                }),
            );
        });

        //
        // Save filter if changed
        //

        let filter_is_not_null =
            components::FilterIsNotNull::new(active, &filter_entity, filter_component);

        if original_filter_is_not_null.as_ref() != Some(&filter_is_not_null) {
            self.save_filter_is_not_null(ctx, &filter_is_not_null);
        }

        Ok(())
    }

    pub(super) fn column_visibility_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        view_columns: &[ColumnDescriptor],
    ) -> Result<(), SpaceViewSystemExecutionError> {
        // Gather our selected columns.
        let selected_columns: HashSet<_> = self
            .apply_column_visibility_to_view_columns(ctx, view_columns)?
            .map(|columns| columns.into_iter().collect())
            .unwrap_or_else(|| view_columns.iter().cloned().map(Into::into).collect());

        let visible_count = selected_columns.len();
        let hidden_count = view_columns.len() - visible_count;
        let visible_count_label = format!("{visible_count} visible, {hidden_count} hidden");

        let mut new_selected_columns = selected_columns.clone();

        let modal_ui = |ui: &mut egui::Ui| {
            //
            // Summary toggle
            //

            let indeterminate = visible_count != 0 && hidden_count != 0;
            let mut all_enabled = hidden_count == 0;

            if ui
                .checkbox_indeterminate(&mut all_enabled, &visible_count_label, indeterminate)
                .changed()
            {
                if all_enabled {
                    self.save_all_columns_selected(ctx);
                } else {
                    self.save_all_columns_unselected(ctx);
                }
            }

            ui.add_space(12.0);

            //
            // Time columns
            //

            let mut first = true;
            for column in view_columns {
                let ColumnDescriptor::Time(time_column_descriptor) = column else {
                    continue;
                };

                if first {
                    ui.add_space(6.0);
                    ui.label("Timelines");
                    first = false;
                }

                let column_selector: ColumnSelector = column.clone().into();

                // The query timeline is always active because it's necessary for the dataframe ui
                // (for tooltips).
                let is_query_timeline = time_column_descriptor.timeline.name() == timeline.name();
                let is_enabled = !is_query_timeline;
                let mut is_visible =
                    is_query_timeline || selected_columns.contains(&column_selector);

                ui.add_enabled_ui(is_enabled, |ui| {
                    if ui
                        .re_checkbox(&mut is_visible, column.short_name())
                        .on_disabled_hover_text("The query timeline must always be visible")
                        .changed()
                    {
                        if is_visible {
                            new_selected_columns.insert(column_selector);
                        } else {
                            new_selected_columns.remove(&column_selector);
                        }
                    }
                });
            }

            //
            // Component columns
            //

            let mut current_entity = None;
            for column in view_columns {
                let ColumnDescriptor::Component(component_column_descriptor) = column else {
                    continue;
                };

                if Some(&component_column_descriptor.entity_path) != current_entity.as_ref() {
                    current_entity = Some(component_column_descriptor.entity_path.clone());
                    ui.add_space(6.0);
                    ui.label(component_column_descriptor.entity_path.to_string());
                }

                let column_selector: ColumnSelector = column.clone().into();
                let mut is_visible = selected_columns.contains(&column_selector);

                if ui
                    .re_checkbox(&mut is_visible, column.short_name())
                    .changed()
                {
                    if is_visible {
                        new_selected_columns.insert(column_selector);
                    } else {
                        new_selected_columns.remove(&column_selector);
                    }
                }
            }
        };

        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Columns").value_fn(
            |ui, _| {
                egui::menu::menu_button(ui, &visible_count_label, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, modal_ui)
                });
            },
        ));

        // save changes of column visibility
        if new_selected_columns != selected_columns {
            if new_selected_columns.len() == view_columns.len() {
                // length match is a guaranteed match because the `selected_columns` sets are built
                // from filtering out the view columns
                self.save_all_columns_selected(ctx);
            } else {
                self.save_selected_columns(ctx, new_selected_columns);
            }
        }

        Ok(())
    }

    pub(super) fn latest_at_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        ui.label("Empty cells:");

        let mut latest_at = self.latest_at_enabled()?;
        let changed = {
            ui.re_radio_value(&mut latest_at, false, "Leave empty")
                .changed()
        } | {
            ui.re_radio_value(&mut latest_at, true, "Fill with latest-at values")
                .changed()
        };

        if changed {
            self.save_latest_at_enabled(ctx, latest_at);
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
                    .recording_engine()
                    .store()
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
