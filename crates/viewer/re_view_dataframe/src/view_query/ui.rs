use crate::view_query::Query;
use egui::PopupCloseBehavior;
use egui::containers::menu::{MenuButton, MenuConfig};
use re_chunk_store::ColumnDescriptor;
use re_log_types::{
    EntityPath, ResolvedTimeRange, TimeInt, TimeType, Timeline, TimelineName, TimestampFormat,
};
use re_sorbet::{ColumnSelector, ComponentColumnSelector};
use re_types::blueprint::components;
use re_ui::{TimeDragValue, UiExt as _, list_item};
use re_viewer_context::{ViewId, ViewSystemExecutionError, ViewerContext};
use std::collections::{BTreeSet, HashSet};

type ComponentSelectorSet = std::collections::BTreeSet<ComponentColumnSelector>;

// UI implementation
impl Query {
    pub(super) fn timeline_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        mut timeline_name: TimelineName,
    ) -> Result<(), ViewSystemExecutionError> {
        egui::Grid::new("dataframe_view_query_ui_timeline")
            .num_columns(2)
            .spacing(egui::vec2(8.0, 10.0))
            .show(ui, |ui| -> Result<_, ViewSystemExecutionError> {
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
        timeline: Option<&Timeline>,
    ) -> Result<(), ViewSystemExecutionError> {
        let time_drag_value_and_type = timeline.map(|timeline| {
            let time_drag_value =
                if let Some(times) = ctx.recording().time_histogram(timeline.name()) {
                    TimeDragValue::from_time_histogram(times)
                } else {
                    debug_assert!(
                        false,
                        "This should never happen because `timeline` should exist if not `None`"
                    );
                    TimeDragValue::from_time_range(0..=0)
                };

            (time_drag_value, timeline.typ())
        });

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
                        if let Some((time_drag_value, timeline_type)) = &time_drag_value_and_type {
                            let response = time_boundary_ui(
                                ui,
                                time_drag_value,
                                None,
                                *timeline_type,
                                ctx.app_options().timestamp_format,
                                &mut start,
                            );

                            changed |= response.changed();
                            should_display_time_range |=
                                response.hovered() || response.dragged() || response.has_focus();
                        } else {
                            ui.add_enabled(false, egui::Label::new("n/a"))
                                .on_disabled_hover_text(
                                    "Select an existing timeline to edit this property",
                                );
                        }
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
                        if let Some((time_drag_value, timeline_type)) = &time_drag_value_and_type {
                            let response = time_boundary_ui(
                                ui,
                                time_drag_value,
                                Some(start),
                                *timeline_type,
                                ctx.app_options().timestamp_format,
                                &mut end,
                            );

                            changed |= response.changed();
                            should_display_time_range |=
                                response.hovered() || response.dragged() || response.has_focus();
                        } else {
                            ui.add_enabled(false, egui::Label::new("n/a"))
                                .on_disabled_hover_text(
                                    "Select an existing timeline to edit this property",
                                );
                        }
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
            if Some(time_ctrl.timeline()) == timeline {
                time_ctrl.highlighted_range = Some(ResolvedTimeRange::new(start, end));
            }
        }

        Ok(())
    }

    pub(super) fn filter_is_not_null_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: Option<&TimelineName>,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        //
        // Read stuff
        //

        let original_filter_is_not_null = self.filter_is_not_null_raw()?;

        let (mut active, filter) = original_filter_is_not_null
            .as_ref()
            .map(|filter| {
                (
                    filter.active(),
                    ComponentColumnSelector::try_new_from_column_name(
                        &filter.entity_path(),
                        filter.component_name().as_str(),
                    )
                    .inspect_err(|err| {
                        re_log::warn!("could not parse component column selector: {err}");
                    })
                    .ok(),
                )
            })
            .unwrap_or((false, None));

        //
        // Filter active?
        //

        ui.add_enabled_ui(timeline.is_some(), |ui| {
            ui.re_checkbox(&mut active, "Filter rows where column is not null:")
                .on_disabled_hover_text("Select an existing timeline to edit this property");
        });

        //
        // Fallback UI if timeline is not found
        //

        let Some(timeline) = timeline else {
            ui.add_enabled_ui(false, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Entity").value_text(
                        filter
                            .as_ref()
                            .map(|f| f.entity_path.clone())
                            .unwrap_or_else(EntityPath::root)
                            .to_string(),
                    ),
                )
                .on_disabled_hover_text("Select an existing timeline to edit this property");

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Component").value_text(
                        filter
                            .as_ref()
                            .map(|f| f.qualified_archetype_field_name())
                            .unwrap_or_else(|| "-".to_owned()),
                    ),
                )
                .on_disabled_hover_text("Select an existing timeline to edit this property");
            });

            return Ok(());
        };

        //
        // Filter entity
        //

        let all_entities = all_pov_entities_for_view(ctx, view_id, timeline);

        let mut filter_entity = filter
            .as_ref()
            .and_then(|filter| {
                all_entities
                    .contains(&filter.entity_path)
                    .then_some(filter.entity_path.clone())
            })
            .or_else(|| all_entities.iter().next().cloned())
            .unwrap_or_else(EntityPath::root);

        //
        // Filter component
        //

        let all_components = ctx
            .recording_engine()
            .store()
            .all_components_on_timeline_sorted(timeline, &filter_entity)
            .unwrap_or_default();

        // TODO(#10065): This should be cleaned up.
        let all_components = all_components
            .into_iter()
            .map(|descr| ComponentColumnSelector::from_descriptor(filter_entity.clone(), &descr))
            .collect::<BTreeSet<_>>();

        // The list of suggested components is built as follows:
        // - consider all indicator components
        // - for the matching archetypes, take all required components
        // - keep those that are actually present
        let suggested_components = || {
            all_components
                .iter()
                .filter_map(|c| {
                    c.archetype_name.as_ref().and_then(|archetype_name| {
                        ctx.reflection()
                            .archetypes
                            .get(archetype_name)
                            .map(|archetype_reflection| (archetype_name, archetype_reflection))
                    })
                })
                .flat_map(|(archetype_name, archetype_reflection)| {
                    archetype_reflection.required_fields().map(|field| {
                        ComponentColumnSelector::from_descriptor(
                            filter_entity.clone(),
                            &field.component_descriptor(*archetype_name),
                        )
                    })
                })
                .filter(|c| all_components.iter().any(|descr| descr == c))
                .collect::<ComponentSelectorSet>()
        };

        // If the currently saved component, we auto-switch it to a reasonable one.
        let mut filter_component = filter
            .and_then(|component| {
                all_components
                    .iter()
                    .any(|descr| descr == &component)
                    .then_some(component)
            })
            .or_else(|| suggested_components().first().cloned());

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
                        .selected_text(
                            filter_component
                                .as_ref()
                                .map(|f| f.qualified_archetype_field_name())
                                .unwrap_or("-".to_owned()),
                        )
                        .show_ui(ui, |ui| {
                            for descr in all_components {
                                let label = descr.qualified_archetype_field_name();
                                ui.selectable_value(&mut filter_component, Some(descr), label);
                            }
                        });
                }),
            );
        });

        //
        // Save filter if changed
        //

        if let Some(filter_component) = filter_component {
            let filter_is_not_null = components::FilterIsNotNull::new(
                active,
                &filter_entity,
                filter_component.qualified_archetype_field_name().into(),
            );

            if original_filter_is_not_null.as_ref() != Some(&filter_is_not_null) {
                self.save_filter_is_not_null(ctx, &filter_is_not_null);
            }
        }

        Ok(())
    }

    pub(super) fn column_visibility_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: Option<&Timeline>,
        view_columns: Option<&[ColumnDescriptor]>,
    ) -> Result<(), ViewSystemExecutionError> {
        if let (Some(timeline), Some(view_columns)) = (timeline, view_columns) {
            self.column_visibility_ui_impl(ctx, ui, timeline, view_columns)
        } else {
            Self::column_visibility_ui_fallback(ui);
            Ok(())
        }
    }

    fn column_visibility_ui_impl(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        view_columns: &[ColumnDescriptor],
    ) -> Result<(), ViewSystemExecutionError> {
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

            // TODO(#9921): add support for showing Row ID column
            if false {
                let mut show_row_id = view_columns
                    .iter()
                    .any(|d| matches!(d, ColumnDescriptor::RowId(_)));
                if ui
                    .re_checkbox(&mut show_row_id, "RowID")
                    .on_disabled_hover_text("The query timeline must always be visible")
                    .changed()
                {
                    if show_row_id {
                        new_selected_columns.insert(ColumnSelector::RowId);
                    } else {
                        new_selected_columns.remove(&ColumnSelector::RowId);
                    }
                }
            }

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
                let is_query_timeline = time_column_descriptor.timeline() == *timeline;
                let is_enabled = !is_query_timeline;
                let mut is_visible =
                    is_query_timeline || selected_columns.contains(&column_selector);

                ui.add_enabled_ui(is_enabled, |ui| {
                    if ui
                        .re_checkbox(&mut is_visible, column.display_name())
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
                    .re_checkbox(&mut is_visible, column.display_name())
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
                MenuButton::new(&visible_count_label)
                    .config(
                        MenuConfig::default()
                            .close_behavior(PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| {
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

    fn column_visibility_ui_fallback(ui: &mut egui::Ui) {
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Columns").value_fn(
            |ui, _| {
                ui.add_enabled_ui(false, |ui| {
                    ui.label("n/a").on_disabled_hover_text(
                        "Select an existing timeline to edit this property",
                    );
                });
            },
        ));
    }

    pub(super) fn latest_at_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) -> Result<(), ViewSystemExecutionError> {
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
fn all_pov_entities_for_view(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    timeline: &TimelineName,
) -> BTreeSet<EntityPath> {
    let mut all_entities = BTreeSet::new();
    ctx.lookup_query_result(view_id).tree.visit(&mut |node| {
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
    timestamp_format: TimestampFormat,
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
        time_drag_value.drag_value_ui(
            ui,
            timeline_typ,
            time,
            true,
            low_bound_override,
            timestamp_format,
        )
    }
}

fn edit_timeline_name(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut TimelineName,
) -> egui::Response {
    let mut changed = false;
    let mut combobox_response = egui::ComboBox::from_id_salt(value.as_str())
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for &timeline in ctx.recording().timelines().keys() {
                let response = ui.selectable_value(value, timeline, timeline.as_str());
                changed |= response.changed();
            }
        });

    if changed {
        combobox_response.response.mark_changed();
    }

    combobox_response.response
}
