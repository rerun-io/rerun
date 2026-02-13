use std::collections::HashSet;

use re_chunk_store::ColumnDescriptor;
use re_log_types::{AbsoluteTimeRange, Timeline, TimelineName};
use re_sdk_types::blueprint::archetypes::DataframeQuery;
use re_sdk_types::blueprint::{components, datatypes};
use re_sorbet::{ColumnSelector, ComponentColumnSelector};
use re_viewer_context::{ViewSystemExecutionError, ViewerContext};

use crate::dataframe_ui::HideColumnAction;
use crate::view_query::Query;

// Accessors wrapping reads/writes to the blueprint store.
impl Query {
    /// Get the query timeline name.
    ///
    /// This dis-regards whether a timeline actually exists with this name.
    pub(crate) fn timeline_name(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> Result<re_log_types::TimelineName, ViewSystemExecutionError> {
        let timeline_name = self
            .query_property
            .component_or_empty::<components::TimelineName>(
                DataframeQuery::descriptor_timeline().component,
            )?;

        // if the timeline is unset, we "freeze" it to the current time panel timeline
        if let Some(timeline_name) = timeline_name {
            Ok(timeline_name.into())
        } else {
            let timeline_name = *ctx.time_ctrl.timeline_name();
            self.save_timeline_name(ctx, &timeline_name);

            Ok(timeline_name)
        }
    }

    /// Get the query timeline.
    ///
    /// This returns the query timeline if it actually exists, or `None` otherwise.
    pub fn timeline(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> Result<Option<Timeline>, ViewSystemExecutionError> {
        let timeline_name = self.timeline_name(ctx)?;

        Ok(ctx.recording().timelines().get(&timeline_name).copied())
    }

    /// Save the timeline to the one specified.
    ///
    /// Note: this resets the range filter timestamps to -inf/+inf as any other value might be
    /// invalidated.
    pub fn save_timeline_name(&self, ctx: &ViewerContext<'_>, timeline_name: &TimelineName) {
        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_timeline(),
            &components::TimelineName::from(timeline_name.as_str()),
        );

        // clearing the range filter is equivalent to setting it to the default -inf/+inf
        self.query_property
            .clear_blueprint_component(ctx, DataframeQuery::descriptor_filter_by_range());
    }

    pub fn filter_by_range(&self) -> Result<AbsoluteTimeRange, ViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::FilterByRange>(
                DataframeQuery::descriptor_filter_by_range().component,
            )?
            .map(|range_filter| AbsoluteTimeRange::new(range_filter.start, range_filter.end))
            .unwrap_or(AbsoluteTimeRange::EVERYTHING))
    }

    pub fn save_filter_by_range(&self, ctx: &ViewerContext<'_>, range: AbsoluteTimeRange) {
        if range == AbsoluteTimeRange::EVERYTHING {
            self.query_property
                .clear_blueprint_component(ctx, DataframeQuery::descriptor_filter_by_range());
        } else {
            self.query_property.save_blueprint_component(
                ctx,
                &DataframeQuery::descriptor_filter_by_range(),
                &components::FilterByRange::new(range.min(), range.max()),
            );
        }
    }

    /// Get the filter column for the filter-is-not-null feature, if active.
    pub fn filter_is_not_null(
        &self,
    ) -> Result<Option<ComponentColumnSelector>, ViewSystemExecutionError> {
        Ok(self
            .filter_is_not_null_raw()?
            .filter(|filter_is_not_null| filter_is_not_null.active())
            .map(|filter| filter.column_selector()))
    }

    /// Get the raw [`components::FilterIsNotNull`] struct (for ui purposes).
    pub fn filter_is_not_null_raw(
        &self,
    ) -> Result<Option<components::FilterIsNotNull>, ViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::FilterIsNotNull>(
                DataframeQuery::descriptor_filter_is_not_null().component,
            )?)
    }

    pub fn save_filter_is_not_null(
        &self,
        ctx: &ViewerContext<'_>,
        filter_is_not_null: &components::FilterIsNotNull,
    ) {
        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_filter_is_not_null(),
            filter_is_not_null,
        );
    }

    pub fn latest_at_enabled(&self) -> Result<bool, ViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::ApplyLatestAt>(
                DataframeQuery::descriptor_apply_latest_at().component,
            )?
            .is_some_and(|comp| *comp.0))
    }

    pub fn save_latest_at_enabled(&self, ctx: &ViewerContext<'_>, enabled: bool) {
        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_apply_latest_at(),
            &components::ApplyLatestAt(enabled.into()),
        );
    }

    pub fn save_selected_columns(
        &self,
        ctx: &ViewerContext<'_>,
        columns: impl IntoIterator<Item = ColumnSelector>,
    ) {
        let mut selected_columns = datatypes::SelectedColumns::default();
        for column in columns {
            match column {
                ColumnSelector::RowId => {
                    // selected_columns.row_id = true.into(); // TODO(#9921)
                }

                ColumnSelector::Time(desc) => {
                    selected_columns
                        .time_columns
                        .push(desc.timeline.as_str().into());
                }

                ColumnSelector::Component(selector) => {
                    let blueprint_component_selector = datatypes::ComponentColumnSelector::new(
                        &selector.entity_path,
                        selector.component,
                    );

                    selected_columns
                        .component_columns
                        .push(blueprint_component_selector);
                }
            }
        }

        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_select(),
            &components::SelectedColumns(selected_columns),
        );
    }

    pub fn save_all_columns_selected(&self, ctx: &ViewerContext<'_>) {
        self.query_property
            .clear_blueprint_component(ctx, DataframeQuery::descriptor_select());
    }

    pub fn save_all_columns_unselected(&self, ctx: &ViewerContext<'_>) {
        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_select(),
            &components::SelectedColumns::default(),
        );
    }

    /// Given some view columns, list the columns that should be visible (aka "selected columns"),
    /// according to the blueprint.
    ///
    /// This operates by filtering the view columns based on the blueprint specified columns.
    ///
    /// Returns `Ok(None)` if all columns should be displayed (aka a column selection isn't provided
    /// in the blueprint).
    pub fn apply_column_visibility_to_view_columns(
        &self,
        ctx: &ViewerContext<'_>,
        view_columns: &[ColumnDescriptor],
    ) -> Result<Option<Vec<ColumnSelector>>, ViewSystemExecutionError> {
        let selected_columns = self
            .query_property
            .component_or_empty::<components::SelectedColumns>(
                DataframeQuery::descriptor_select().component,
            )?;

        // no selected columns means all columns are visible
        let Some(datatypes::SelectedColumns {
            // row_id, // TODO(#9921)
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
            .map(|selector| selector.column_selector().column_name())
            .collect::<HashSet<_>>();

        let query_timeline_name = self.timeline_name(ctx)?;
        let result = view_columns
            .iter()
            .filter(|column| match column {
                ColumnDescriptor::RowId(_) => false, // TODO(#9921)

                ColumnDescriptor::Time(desc) => {
                    // we always include the query timeline column because we need it for the dataframe ui
                    desc.timeline_name() == query_timeline_name
                        || selected_time_columns.contains(&desc.timeline_name())
                }

                ColumnDescriptor::Component(desc) => selected_component_columns
                    .contains(&desc.column_name(re_sorbet::BatchType::Dataframe)),
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
    ) -> Result<(), ViewSystemExecutionError> {
        if actions.is_empty() {
            return Ok(());
        }

        let mut selected_columns: Vec<_> = self
            .apply_column_visibility_to_view_columns(ctx, view_columns)?
            .map(|columns| columns.into_iter().collect())
            .unwrap_or_else(|| view_columns.iter().cloned().map(Into::into).collect());

        for action in actions {
            match action {
                HideColumnAction::RowId => {
                    selected_columns.retain(|column| column != &ColumnSelector::RowId);
                }

                HideColumnAction::Time { timeline_name } => {
                    selected_columns.retain(|column| {
                        if let ColumnSelector::Time(desc) = column {
                            desc.timeline != timeline_name
                        } else {
                            true
                        }
                    });
                }

                HideColumnAction::Component { entity_path, descr } => {
                    selected_columns.retain(|column| {
                        if let ColumnSelector::Component(selector) = column {
                            selector.entity_path != entity_path
                                || selector.component != descr.component.to_string()
                        } else {
                            true
                        }
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
    use re_test_context::TestContext;
    use re_viewer_context::ViewId;

    use super::Query;

    /// Simple test to demo round-trip testing using [`TestContext::run_and_handle_system_commands`].
    #[test]
    fn test_latest_at_enabled() {
        let test_context = TestContext::new();

        let view_id = ViewId::random();

        test_context.run_in_egui_central_panel(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            query.save_latest_at_enabled(ctx, true);
        });

        egui::__run_test_ctx(|egui_ctx| {
            test_context.handle_system_commands(egui_ctx);
        });

        test_context.run_in_egui_central_panel(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            assert!(query.latest_at_enabled().unwrap());
        });
    }
}
