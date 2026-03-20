use std::ops::RangeInclusive;

use re_chunk_store::ChunkStore;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, Timeline, TimestampFormat};
use re_types_core::ComponentIdentifier;
use re_ui::{TimeDragValue, UiExt as _};

#[derive(Debug)]
pub(crate) enum ChunkListQueryMode {
    LatestAt(TimeInt),
    Range(AbsoluteTimeRange),
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
        component: ComponentIdentifier,
        query: ChunkListQueryMode,
    },
}

impl ChunkListMode {
    fn defaults(
        &self,
        chunk_store: &ChunkStore,
    ) -> Option<(Timeline, EntityPath, ComponentIdentifier)> {
        let all_timelines = chunk_store.schema().timelines();
        let all_entities = chunk_store.all_entities_sorted();
        let all_components = chunk_store.all_components_sorted();

        let current_timeline = match self {
            Self::All => all_timelines.values().next().copied()?,
            Self::Query { timeline, .. } => *timeline,
        };
        let current_entity = match self {
            Self::All => all_entities.first().cloned()?,
            Self::Query { entity_path, .. } => entity_path.clone(),
        };
        let current_component = match self {
            Self::All => *all_components.iter().next()?,
            Self::Query { component, .. } => *component,
        };

        Some((current_timeline, current_entity, current_component))
    }

    pub(crate) fn selector_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
    ) -> Option<()> {
        let (current_timeline, current_entity, current_component) = self.defaults(chunk_store)?;

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
                    component: current_component,
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
                    query: ChunkListQueryMode::Range(AbsoluteTimeRange::EVERYTHING),
                    entity_path: current_entity,
                    component: current_component,
                };
            }
        });

        Some(())
    }

    pub(crate) fn query_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        format: TimestampFormat,
    ) -> Option<()> {
        let all_timelines = chunk_store.schema().timelines();
        let all_entities = chunk_store.all_entities_sorted();
        let all_components = chunk_store.all_components_sorted();
        let (current_timeline, current_entity, current_component) = self.defaults(chunk_store)?;

        let Self::Query {
            timeline: query_timeline,
            component: query_component,
            entity_path: query_entity,
            query,
        } = self
        else {
            return Some(());
        };

        ui.horizontal(|ui| {
            ui.horizontal(|ui| {
                ui.label("Timeline:");
                egui::ComboBox::new("timeline", "")
                    .selected_text(current_timeline.name().as_str())
                    .show_ui(ui, |ui| {
                        for &timeline in all_timelines.values() {
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
                //TODO(ab): this should be a text edit with auto-complete (like view origin)
                egui::ComboBox::new("component", "")
                    .selected_text(current_component.as_str())
                    .height(500.0)
                    .show_ui(ui, |ui| {
                        for component_type in all_components {
                            if ui.button(component_type.as_str()).clicked() {
                                *query_component = component_type;
                            }
                        }
                    });
            });

            let time_drag_value =
                if let Some(time_range) = chunk_store.time_range(query_timeline.name()) {
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
                    ui.label("At:");
                    time_drag_value.drag_value_ui(ui, time_typ, time, true, None, format);
                }
                ChunkListQueryMode::Range(range) => {
                    let (mut min, mut max) = (range.min(), range.max());

                    ui.label("From:");
                    time_drag_value.drag_value_ui(ui, time_typ, &mut min, true, None, format);

                    ui.label("To:");
                    time_drag_value.drag_value_ui(ui, time_typ, &mut max, true, Some(min), format);

                    range.set_min(min);
                    range.set_max(max);
                }
            }
        });

        Some(())
    }
}
