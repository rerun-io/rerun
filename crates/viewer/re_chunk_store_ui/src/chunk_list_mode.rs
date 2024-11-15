use std::ops::RangeInclusive;

use re_chunk_store::external::re_chunk::ComponentName;
use re_chunk_store::ChunkStore;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, TimeType, TimeZone, Timeline};
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
    /// Show all chunks.
    All,

    /// Only show chunks that are relevant to a given latest-at or range query.
    Query {
        timeline: Timeline,
        entity_path: EntityPath,
        component_name: ComponentName,
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
                    .on_hover_text("Display all chunks")
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
                        "Latest-at",
                    )
                    .on_hover_text("Display chunks relevant to the provided latest-at query")
                    .clicked()
                {
                    *self = Self::Query {
                        timeline: current_timeline,
                        entity_path: current_entity.clone(),
                        component_name: current_component,
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
                    .on_hover_text("Display chunks relevant to the provided range query")
                    .clicked()
                {
                    *self = Self::Query {
                        timeline: current_timeline,
                        query: ChunkListQueryMode::Range(ResolvedTimeRange::EVERYTHING),
                        entity_path: current_entity.clone(),
                        component_name: current_component,
                    };
                }
            });

            let Self::Query {
                timeline: query_timeline,
                component_name: query_component,
                entity_path: query_entity,
                query,
            } = self
            else {
                // No query, we're done here
                return;
            };

            ui.horizontal(|ui| {
                ui.label("timeline:");
                egui::ComboBox::new("timeline", "")
                    .selected_text(current_timeline.name().as_str())
                    .show_ui(ui, |ui| {
                        for timeline in all_timelines {
                            if ui.button(timeline.name().as_str()).clicked() {
                                *query_timeline = timeline;
                            }
                        }
                    });

                ui.label("entity:");
                egui::ComboBox::new("entity_path", "")
                    .selected_text(current_entity.to_string())
                    .show_ui(ui, |ui| {
                        for entpath in all_entities {
                            if ui.button(entpath.to_string()).clicked() {
                                *query_entity = entpath.clone();
                            }
                        }
                    });

                ui.label("component:");
                //TODO(ab): this should be a text edit with auto-complete (like view origin)
                egui::ComboBox::new("component_name", "")
                    .selected_text(current_component.short_name())
                    .height(500.0)
                    .show_ui(ui, |ui| {
                        for component_name in all_components {
                            if ui.button(component_name.short_name()).clicked() {
                                *query_component = component_name;
                            }
                        }
                    });
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

            match query {
                ChunkListQueryMode::LatestAt(time) => {
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
                ChunkListQueryMode::Range(range) => {
                    let (mut min, mut max) = (range.min(), range.max());
                    ui.label("from:");
                    match time_typ {
                        TimeType::Time => {
                            time_drag_value
                                .temporal_drag_value_ui(ui, &mut min, true, None, time_zone);
                            ui.label("to:");
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
                            ui.label("to:");
                            time_drag_value.sequence_drag_value_ui(ui, &mut max, true, Some(min));
                        }
                    };
                    range.set_min(min);
                    range.set_max(max);
                }
            }
        });

        Some(())
    }
}
