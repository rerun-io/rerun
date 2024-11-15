use std::collections::HashSet;

use crate::dataframe_ui::HideColumnAction;
use crate::view_query::Query;
use re_chunk_store::{ColumnDescriptor, ColumnSelector, ComponentColumnSelector};
use re_log_types::{EntityPath, ResolvedTimeRange, TimelineName};
use re_types::blueprint::{components, datatypes};
use re_viewer_context::{SpaceViewSystemExecutionError, ViewerContext};

// Accessors wrapping reads/writes to the blueprint store.
impl Query {
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

    pub(crate) fn filter_by_range(
        &self,
    ) -> Result<ResolvedTimeRange, SpaceViewSystemExecutionError> {
        #[allow(clippy::map_unwrap_or)]
        Ok(self
            .query_property
            .component_or_empty::<components::FilterByRange>()?
            .map(|range_filter| (ResolvedTimeRange::new(range_filter.start, range_filter.end)))
            .unwrap_or(ResolvedTimeRange::EVERYTHING))
    }

    pub(super) fn save_filter_by_range(&self, ctx: &ViewerContext<'_>, range: ResolvedTimeRange) {
        if range == ResolvedTimeRange::EVERYTHING {
            self.query_property
                .clear_blueprint_component::<components::FilterByRange>(ctx);
        } else {
            self.query_property.save_blueprint_component(
                ctx,
                &components::FilterByRange::new(range.min(), range.max()),
            );
        }
    }

    /// Get the filter column for the filter-is-not-null feature, if active.
    pub(crate) fn filter_is_not_null(
        &self,
    ) -> Result<Option<ComponentColumnSelector>, SpaceViewSystemExecutionError> {
        Ok(self
            .filter_is_not_null_raw()?
            .filter(|filter_is_not_null| filter_is_not_null.active())
            .map(|filter| {
                ComponentColumnSelector::new_for_component_name(
                    filter.entity_path(),
                    filter.component_name(),
                )
            }))
    }

    /// Get the raw [`components::FilterIsNotNull`] struct (for ui purposes).
    pub(super) fn filter_is_not_null_raw(
        &self,
    ) -> Result<Option<components::FilterIsNotNull>, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::FilterIsNotNull>()?)
    }

    pub(super) fn save_filter_is_not_null(
        &self,
        ctx: &ViewerContext<'_>,
        filter_is_not_null: &components::FilterIsNotNull,
    ) {
        self.query_property
            .save_blueprint_component(ctx, filter_is_not_null);
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
                ColumnSelector::Time(desc) => {
                    selected_columns
                        .time_columns
                        .push(desc.timeline.as_str().into());
                }

                ColumnSelector::Component(selector) => {
                    let blueprint_component_selector = datatypes::ComponentColumnSelector::new(
                        &selector.entity_path,
                        &selector.component_name,
                    );

                    selected_columns
                        .component_columns
                        .push(blueprint_component_selector);
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

    /// Given some view columns, list the columns that should be visible (aka "selected columns"),
    /// according to the blueprint.
    ///
    /// This operates by filtering the view columns based on the blueprint specified columns.
    ///
    /// Returns `Ok(None)` if all columns should be displayed (aka a column selection isn't provided
    /// in the blueprint).
    pub(crate) fn apply_column_visibility_to_view_columns(
        &self,
        ctx: &ViewerContext<'_>,
        view_columns: &[ColumnDescriptor],
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
            // select all columns
            return Ok(None);
        };

        let selected_time_columns: HashSet<TimelineName> = time_columns
            .iter()
            .map(|timeline_name| timeline_name.as_str().into())
            .collect();
        let selected_component_columns = component_columns
            .iter()
            .map(|selector| {
                (
                    EntityPath::from(selector.entity_path.as_str()),
                    selector.component.as_str(),
                )
            })
            .collect::<HashSet<_>>();

        let query_timeline_name = *self.timeline(ctx)?.name();
        let result = view_columns
            .iter()
            .filter(|column| match column {
                ColumnDescriptor::Time(desc) => {
                    // we always include the query timeline column because we need it for the dataframe ui
                    desc.timeline.name() == &query_timeline_name
                        || selected_time_columns.contains(desc.timeline.name())
                }

                ColumnDescriptor::Component(desc) => {
                    // Check against both the full name and short name, as the user might have used
                    // the latter in the blueprint API.
                    //
                    // TODO(ab): this means that if the user chooses `"/foo/bar:Scalar"`, it will
                    // select both `rerun.components.Scalar` and `Scalar`, should both of these
                    // exist.
                    selected_component_columns
                        .contains(&(desc.entity_path.clone(), desc.component_name.full_name()))
                        || selected_component_columns
                            .contains(&(desc.entity_path.clone(), desc.component_name.short_name()))
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
        view_columns: &[ColumnDescriptor],
        actions: Vec<HideColumnAction>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        if actions.is_empty() {
            return Ok(());
        }

        let mut selected_columns: Vec<_> = self
            .apply_column_visibility_to_view_columns(ctx, view_columns)?
            .map(|columns| columns.into_iter().collect())
            .unwrap_or_else(|| view_columns.iter().cloned().map(Into::into).collect());

        for action in actions {
            match action {
                HideColumnAction::HideTimeColumn { timeline_name } => {
                    selected_columns.retain(|column| match column {
                        ColumnSelector::Time(desc) => desc.timeline != timeline_name,
                        ColumnSelector::Component(_) => true,
                    });
                }

                HideColumnAction::HideComponentColumn {
                    entity_path,
                    component_name,
                } => {
                    selected_columns.retain(|column| match column {
                        ColumnSelector::Component(selector) => {
                            selector.entity_path != entity_path
                                || !component_name.matches(&selector.component_name)
                        }
                        ColumnSelector::Time(_) => true,
                    });
                }
            }
        }

        self.save_selected_columns(ctx, selected_columns);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::Query;
    use re_viewer_context::test_context::TestContext;
    use re_viewer_context::SpaceViewId;

    /// Simple test to demo round-trip testing using [`TestContext::run_and_handle_system_commands`].
    #[test]
    fn test_latest_at_enabled() {
        let mut test_context = TestContext::default();

        let view_id = SpaceViewId::random();

        test_context.run_and_handle_system_commands(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            query.save_latest_at_enabled(ctx, true);
        });

        test_context.run(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            assert!(query.latest_at_enabled().unwrap());
        });
    }
}
