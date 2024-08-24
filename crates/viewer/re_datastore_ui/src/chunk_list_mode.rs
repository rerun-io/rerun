use std::ops::RangeInclusive;

use re_chunk_store::external::re_chunk::ComponentName;
use re_chunk_store::ChunkStore;
use re_log_types::external::re_types_core::datatypes::TimeInt;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeType, TimeZone, Timeline};
use re_ui::UiExt;
use re_viewer_context::TimeDragValue;

#[derive(Debug)]
pub(crate) enum ChunkListQueryMode {
    LatestAt(TimeInt),
    Range(ResolvedTimeRange),
}

#[derive(Debug, Default)]
pub(crate) enum ChunkListMode {
    #[default]
    All,
    Query {
        timeline: Timeline,
        entity_path: EntityPath,
        component_name: ComponentName,

        component_name_filter: String,

        query: ChunkListQueryMode,
    },
}

impl ChunkListMode {
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        time_zone: TimeZone,
    ) -> Option<()> {
        let all_timelines = chunk_store.all_timelines();
        let all_entities = chunk_store.all_entities();
        let all_components = chunk_store.all_components();

        let current_timeline = match self {
            Self::All => all_timelines.first().copied()?,
            Self::Query { timeline, .. } => *timeline,
        };
        let current_entity = match self {
            Self::All => all_entities.first().cloned()?,
            Self::Query { entity_path, .. } => entity_path.clone(),
        };
        let current_component = match self {
            Self::All => all_components.first().copied()?,
            Self::Query { component_name, .. } => *component_name,
        };

        ui.horizontal(|ui| {
            ui.selectable_toggle(|ui| {
                if ui
                    .selectable_label(matches!(self, Self::All), "All")
                    .clicked()
                {
                    *self = Self::All;
                }

                if ui
                    .selectable_label(
                        matches!(
                            self,
                            Self::Query {
                                query: ChunkListQueryMode::LatestAt(..),
                                ..
                            }
                        ),
                        "Latest at",
                    )
                    .clicked()
                {
                    *self = Self::Query {
                        timeline: current_timeline,
                        entity_path: current_entity.clone(),
                        component_name: current_component,
                        component_name_filter: String::new(),
                        query: ChunkListQueryMode::LatestAt(TimeInt::MAX),
                    };
                }

                if ui
                    .selectable_label(
                        matches!(
                            self,
                            Self::Query {
                                query: ChunkListQueryMode::Range(..),
                                ..
                            }
                        ),
                        "Range",
                    )
                    .clicked()
                {
                    *self = Self::Query {
                        timeline: current_timeline,
                        query: ChunkListQueryMode::Range(ResolvedTimeRange::EVERYTHING),
                        entity_path: current_entity.clone(),
                        component_name: current_component,
                        component_name_filter: String::new(),
                    };
                }
            });

            //TODO: more compact way to do that?
            let (query_timeline, query_component, query_entity, component_name_filter) = match self
            {
                Self::All => {
                    // No query, we're done here
                    return;
                }
                Self::Query {
                    timeline,
                    component_name,
                    entity_path,
                    component_name_filter,
                    ..
                } => (timeline, component_name, entity_path, component_name_filter),
            };

            ui.horizontal(|ui| {
                ui.label("Timeline:");
                egui::ComboBox::new("timeline", "")
                    .selected_text(current_timeline.name().as_str())
                    .show_ui(ui, |ui| {
                        for timeline in all_timelines {
                            if ui.button(timeline.name().as_str()).clicked() {
                                *query_timeline = timeline;
                            }
                        }
                    });

                ui.label("Entity:");
                egui::ComboBox::new("entity_path", "")
                    .selected_text(current_entity.to_string())
                    .show_ui(ui, |ui| {
                        for entpath in all_entities {
                            if ui.button(entpath.to_string()).clicked() {
                                *query_entity = entpath.clone();
                            }
                        }
                    });

                ui.label("Component:");
                let resp = egui::ComboBox::new("component_name", "")
                    .selected_text(current_component.short_name())
                    .height(500.0)
                    .show_ui(ui, |ui| {
                        //TODO: this doesn't really work well, make a custom one.
                        ui.add_space(3.0);
                        ui.text_edit_singleline(component_name_filter)
                            .request_focus();

                        for compname in all_components {
                            if !component_name_filter.is_empty()
                                && !compname
                                    .short_name()
                                    .to_lowercase()
                                    .contains(&component_name_filter.to_lowercase())
                            {
                                continue;
                            }

                            if ui.button(compname.short_name()).clicked() {
                                *query_component = compname;
                            }
                        }
                    });
                if resp.response.clicked() {
                    *component_name_filter = String::new();
                }
            });

            let time_drag_value = if let Some(time_range) = chunk_store.time_range(query_timeline) {
                TimeDragValue::from_time_range(RangeInclusive::new(
                    time_range.min().as_i64(),
                    time_range.max().as_i64(),
                ))
            } else {
                TimeDragValue::from_time_range(0..=0)
            };
            let time_typ = query_timeline.typ();

            match self {
                Self::Query {
                    query: ChunkListQueryMode::LatestAt(time),
                    ..
                } => {
                    ui.label("at:");
                    match time_typ {
                        TimeType::Time => {
                            time_drag_value.temporal_drag_value_ui(ui, time, true, None, time_zone);
                        }
                        TimeType::Sequence => {
                            time_drag_value.sequence_drag_value_ui(ui, time, true, None);
                        }
                    };
                }
                Self::Query {
                    query: ChunkListQueryMode::Range(range),
                    ..
                } => {
                    let (mut min, mut max) = (range.min().into(), range.max().into());
                    ui.label("Range:");
                    match time_typ {
                        TimeType::Time => {
                            time_drag_value
                                .temporal_drag_value_ui(ui, &mut min, true, None, time_zone);

                            time_drag_value.temporal_drag_value_ui(
                                ui,
                                &mut max,
                                true,
                                Some(min),
                                time_zone,
                            );
                        }
                        TimeType::Sequence => {
                            time_drag_value.sequence_drag_value_ui(ui, &mut min, true, None);
                            time_drag_value.sequence_drag_value_ui(ui, &mut max, true, Some(min));
                        }
                    };
                    range.set_min(min);
                    range.set_max(max);
                }
                _ => {}
            }
        });

        Some(())
    }
}
