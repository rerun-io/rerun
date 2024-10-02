use std::collections::HashSet;

use crate::dataframe_ui::HideColumnAction;
use crate::view_query_v2::QueryV2;
use re_chunk_store::{ColumnDescriptor, ColumnSelector};
use re_log_types::{TimeInt, TimelineName};
use re_types::blueprint::{components, datatypes};
use re_viewer_context::{SpaceViewSystemExecutionError, ViewerContext};

// Accessors wrapping reads/writes to the blueprint store.
impl QueryV2 {
    /// Get the query timeline.
    ///
    /// This tries to read the timeline name from the blueprint. If missing or invalid, the current
    /// timeline is used and saved back to the blueprint.
    pub(crate) fn timeline(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> Result<re_log_types::Timeline, SpaceViewSystemExecutionError> {
        // read the timeline and make sure it actually exists
        let timeline = self
            .query_property
            .component_or_empty::<components::TimelineName>()?
            .and_then(|name| {
                ctx.recording()
                    .timelines()
                    .find(|timeline| timeline.name() == &TimelineName::from(name.as_str()))
                    .copied()
            });

        // if the timeline is unset, we "freeze" it to the current time panel timeline
        let save_timeline = timeline.is_none();
        let timeline = timeline.unwrap_or_else(|| *ctx.rec_cfg.time_ctrl.read().timeline());
        if save_timeline {
            self.save_timeline_name(ctx, timeline.name());
        }

        Ok(timeline)
    }

    /// Save the timeline to the one specified.
    ///
    /// Note: this resets the range filter timestamps to -inf/+inf as any other value might be
    /// invalidated.
    pub(super) fn save_timeline_name(&self, ctx: &ViewerContext<'_>, timeline_name: &TimelineName) {
        self.query_property
            .save_blueprint_component(ctx, &components::TimelineName::from(timeline_name.as_str()));

        // clearing the range filter is equivalent to setting it to the default -inf/+inf
        self.query_property
            .clear_blueprint_component::<components::FilterByRange>(ctx);
    }

    pub(crate) fn range_filter(&self) -> Result<(TimeInt, TimeInt), SpaceViewSystemExecutionError> {
        #[allow(clippy::map_unwrap_or)]
        Ok(self
            .query_property
            .component_or_empty::<components::FilterByRange>()?
            .map(|range_filter| (range_filter.start.into(), range_filter.end.into()))
            .unwrap_or((TimeInt::MIN, TimeInt::MAX)))
    }

    pub(super) fn save_range_filter(&self, ctx: &ViewerContext<'_>, start: TimeInt, end: TimeInt) {
        if (start, end) == (TimeInt::MIN, TimeInt::MAX) {
            self.query_property
                .clear_blueprint_component::<components::FilterByRange>(ctx);
        } else {
            self.query_property
                .save_blueprint_component(ctx, &components::FilterByRange::new(start, end));
        }
    }

    pub(crate) fn filter_by_event(
        &self,
    ) -> Result<Option<components::FilterByEvent>, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::FilterByEvent>()?)
    }

    pub(super) fn save_filter_by_event(
        &self,
        ctx: &ViewerContext<'_>,
        filter_by_event: &components::FilterByEvent,
    ) {
        self.query_property
            .save_blueprint_component(ctx, filter_by_event);
    }

    pub(crate) fn latest_at_enabled(&self) -> Result<bool, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::ApplyLatestAt>()?
            .map_or(false, |comp| *comp.0))
    }

    pub(crate) fn save_latest_at_enabled(&self, ctx: &ViewerContext<'_>, enabled: bool) {
        self.query_property
            .save_blueprint_component(ctx, &components::ApplyLatestAt(enabled.into()));
    }

    pub(super) fn save_selected_columns(
        &self,
        ctx: &ViewerContext<'_>,
        columns: impl IntoIterator<Item = ColumnSelector>,
    ) {
        let mut selected_columns = datatypes::SelectedColumns::default();
        for column in columns {
            match column {
                ColumnSelector::Control(_) => {}
                ColumnSelector::Time(desc) => {
                    selected_columns
                        .time_columns
                        .push(desc.timeline.as_str().into());
                }
                ColumnSelector::Component(desc) => {
                    let blueprint_component_descriptor =
                        datatypes::ComponentColumnSelector::new(&desc.entity_path, desc.component);

                    selected_columns
                        .component_columns
                        .push(blueprint_component_descriptor);
                }
            }
        }

        self.query_property
            .save_blueprint_component(ctx, &components::SelectedColumns(selected_columns));
    }

    pub(super) fn save_all_columns_selected(&self, ctx: &ViewerContext<'_>) {
        self.query_property
            .clear_blueprint_component::<components::SelectedColumns>(ctx);
    }

    pub(super) fn save_all_columns_unselected(&self, ctx: &ViewerContext<'_>) {
        self.query_property
            .save_blueprint_component(ctx, &components::SelectedColumns::default());
    }

    /// Given a schema, list the columns that should be visible, according to the blueprint.
    pub(crate) fn apply_column_visibility_to_schema(
        &self,
        ctx: &ViewerContext<'_>,
        schema: &[ColumnDescriptor],
    ) -> Result<Option<Vec<ColumnSelector>>, SpaceViewSystemExecutionError> {
        let selected_columns = self
            .query_property
            .component_or_empty::<components::SelectedColumns>()?;

        // no selected columns means all columns are visible
        let Some(datatypes::SelectedColumns {
            time_columns,
            component_columns,
        }) = selected_columns.as_deref()
        else {
            return Ok(None);
        };

        let selected_time_columns: HashSet<TimelineName> = time_columns
            .iter()
            .map(|timeline_name| timeline_name.as_str().into())
            .collect();
        let selected_component_columns = component_columns.iter().cloned().collect::<HashSet<_>>();

        let query_timeline_name = *self.timeline(ctx)?.name();
        let result = schema
            .iter()
            .filter(|column| match column {
                ColumnDescriptor::Control(_) => true,
                ColumnDescriptor::Time(desc) => {
                    // we always include the query timeline column because we need it for the dataframe ui
                    desc.timeline.name() == &query_timeline_name
                        || selected_time_columns.contains(desc.timeline.name())
                }
                ColumnDescriptor::Component(desc) => {
                    let blueprint_component_descriptor = components::ComponentColumnSelector::new(
                        &desc.entity_path,
                        desc.component_name,
                    );

                    selected_component_columns.contains(&blueprint_component_descriptor)
                }
            })
            .cloned()
            .map(ColumnSelector::from)
            .collect();

        Ok(Some(result))
    }

    pub(crate) fn handle_hide_column_actions(
        &self,
        ctx: &ViewerContext<'_>,
        schema: &[ColumnDescriptor],
        actions: Vec<HideColumnAction>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        if actions.is_empty() {
            return Ok(());
        }

        let mut selected_columns: Vec<_> = self
            .apply_column_visibility_to_schema(ctx, schema)?
            .map(|columns| columns.into_iter().collect())
            .unwrap_or_else(|| schema.iter().cloned().map(Into::into).collect());

        for action in actions {
            match action {
                HideColumnAction::HideTimeColumn { timeline_name } => {
                    selected_columns.retain(|column| match column {
                        ColumnSelector::Time(desc) => desc.timeline != timeline_name,
                        _ => true,
                    });
                }

                HideColumnAction::HideComponentColumn {
                    entity_path,
                    component_name,
                } => {
                    selected_columns.retain(|column| match column {
                        ColumnSelector::Component(desc) => {
                            desc.entity_path != entity_path || desc.component != component_name
                        }
                        _ => true,
                    });
                }
            }
        }

        self.save_selected_columns(ctx, selected_columns);

        Ok(())
    }
}
