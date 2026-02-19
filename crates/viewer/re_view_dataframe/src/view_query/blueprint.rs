use std::collections::HashSet;

use re_chunk_store::ColumnDescriptor;
use re_log_types::{AbsoluteTimeRange, EntityPath, Timeline, TimelineName};
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
    /// If no column selection is provided in the blueprint, all columns are returned.
    pub fn visible_column_selectors(
        &self,
        ctx: &ViewerContext<'_>,
        view_columns: &[ColumnDescriptor],
    ) -> Result<Vec<ColumnSelector>, ViewSystemExecutionError> {
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
            return Ok(view_columns
                .iter()
                .cloned()
                .map(ColumnSelector::from)
                .collect());
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

        Ok(result)
    }

    /// Apply entity ordering and column visibility, returning the reordered view columns
    /// and the query selection.
    pub fn apply_column_selection(
        &self,
        ctx: &ViewerContext<'_>,
        view_columns: &[ColumnDescriptor],
    ) -> Result<(Vec<ColumnDescriptor>, Vec<ColumnSelector>), ViewSystemExecutionError> {
        // Step 1: Reorder columns by entity if an order is set, otherwise keep as-is.
        let view_columns = match self.entity_order()? {
            Some(order) => reorder_columns_by_entity(view_columns, &order),
            None => view_columns.to_vec(),
        };

        // Step 2: Apply column visibility.
        let selection = self.visible_column_selectors(ctx, &view_columns)?;

        Ok((view_columns, selection))
    }

    /// Get the entity path order from the blueprint.
    ///
    /// Returns `None` if no order is set (default order).
    fn entity_order(&self) -> Result<Option<Vec<EntityPath>>, ViewSystemExecutionError> {
        let column_order = self
            .query_property
            .component_or_empty::<components::ColumnOrder>(
                DataframeQuery::descriptor_entity_order().component,
            )?;

        Ok(column_order.map(|order| {
            order
                .0
                .iter()
                .map(|ep| EntityPath::from(ep.to_string()))
                .collect()
        }))
    }

    /// Save the entity path order to the blueprint.
    pub fn save_entity_order(&self, ctx: &ViewerContext<'_>, entity_paths: &[EntityPath]) {
        let order = components::ColumnOrder(
            entity_paths
                .iter()
                .map(|ep| ep.to_string().into())
                .collect(),
        );
        self.query_property.save_blueprint_component(
            ctx,
            &DataframeQuery::descriptor_entity_order(),
            &order,
        );
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

        let mut selected_columns = self.visible_column_selectors(ctx, view_columns)?;

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

/// Reorder view columns so that component columns are grouped by entity path
/// in the given order. Time/RowId columns come first, then ordered entities,
/// then any entities not in the order list (in their original order).
fn reorder_columns_by_entity(
    view_columns: &[ColumnDescriptor],
    entity_order: &[EntityPath],
) -> Vec<ColumnDescriptor> {
    use std::collections::BTreeMap;

    // Collect non-component columns (time, rowid) that go first.
    let mut result: Vec<ColumnDescriptor> = view_columns
        .iter()
        .filter(|col| !matches!(col, ColumnDescriptor::Component(_)))
        .cloned()
        .collect();

    // Group component columns by entity path, preserving order within each group.
    let mut entity_groups: BTreeMap<EntityPath, Vec<ColumnDescriptor>> = BTreeMap::new();
    // Track insertion order for entities not in the explicit order.
    let mut seen_order: Vec<EntityPath> = Vec::new();

    for col in view_columns {
        if let ColumnDescriptor::Component(desc) = col {
            let ep = desc.entity_path.clone();
            if !entity_groups.contains_key(&ep) {
                seen_order.push(ep.clone());
            }
            entity_groups.entry(ep).or_default().push(col.clone());
        }
    }

    // First, add entities that are in the explicit order.
    let ordered_set: HashSet<&EntityPath> = entity_order.iter().collect();
    for ep in entity_order {
        if let Some(cols) = entity_groups.remove(ep) {
            result.extend(cols);
        }
    }

    // Then add remaining entities in their original order.
    for ep in &seen_order {
        if !ordered_set.contains(ep)
            && let Some(cols) = entity_groups.remove(ep)
        {
            result.extend(cols);
        }
    }

    result
}

#[cfg(test)]
mod test {
    use re_chunk_store::ColumnDescriptor;
    use re_log_types::EntityPath;
    use re_sorbet::ComponentColumnDescriptor;
    use re_test_context::TestContext;
    use re_types_core::ComponentIdentifier;
    use re_viewer_context::ViewId;

    use super::{Query, reorder_columns_by_entity};

    fn make_component_column(entity: &str, component: &str) -> ColumnDescriptor {
        ColumnDescriptor::Component(ComponentColumnDescriptor {
            entity_path: entity.into(),
            component: ComponentIdentifier::from(component),
            store_datatype: arrow::datatypes::DataType::Null,
            component_type: None,
            archetype: None,
            is_static: false,
            is_tombstone: false,
            is_semantically_empty: false,
        })
    }

    fn entity_path_of(col: &ColumnDescriptor) -> &EntityPath {
        match col {
            ColumnDescriptor::Component(desc) => &desc.entity_path,
            _ => panic!("expected component column"),
        }
    }

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

    #[test]
    fn test_entity_order_round_trip() {
        let test_context = TestContext::new();
        let view_id = ViewId::random();

        let order = vec![EntityPath::from("/entity/b"), EntityPath::from("/entity/a")];

        test_context.run_in_egui_central_panel(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            query.save_entity_order(ctx, &order);
        });

        egui::__run_test_ctx(|egui_ctx| {
            test_context.handle_system_commands(egui_ctx);
        });

        test_context.run_in_egui_central_panel(|ctx, _| {
            let query = Query::from_blueprint(ctx, view_id);
            let read_order = query.entity_order().unwrap().unwrap();
            assert_eq!(read_order, order);
        });
    }

    #[test]
    fn test_apply_entity_order_basic() {
        let columns = vec![
            make_component_column("/entity/a", "Position3D"),
            make_component_column("/entity/a", "Color"),
            make_component_column("/entity/b", "Position3D"),
            make_component_column("/entity/b", "Color"),
        ];

        let order = vec![EntityPath::from("/entity/b"), EntityPath::from("/entity/a")];

        let result = reorder_columns_by_entity(&columns, &order);

        assert_eq!(result.len(), 4);
        assert_eq!(entity_path_of(&result[0]).to_string(), "/entity/b");
        assert_eq!(entity_path_of(&result[1]).to_string(), "/entity/b");
        assert_eq!(entity_path_of(&result[2]).to_string(), "/entity/a");
        assert_eq!(entity_path_of(&result[3]).to_string(), "/entity/a");
    }

    #[test]
    fn test_apply_entity_order_partial() {
        // Only /entity/c is in the order; /entity/a and /entity/b should be appended.
        let columns = vec![
            make_component_column("/entity/a", "Position3D"),
            make_component_column("/entity/b", "Position3D"),
            make_component_column("/entity/c", "Position3D"),
        ];

        let order = vec![EntityPath::from("/entity/c")];

        let result = reorder_columns_by_entity(&columns, &order);

        assert_eq!(result.len(), 3);
        assert_eq!(entity_path_of(&result[0]).to_string(), "/entity/c");
        assert_eq!(entity_path_of(&result[1]).to_string(), "/entity/a");
        assert_eq!(entity_path_of(&result[2]).to_string(), "/entity/b");
    }

    #[test]
    fn test_apply_entity_order_unknown_ignored() {
        let columns = vec![
            make_component_column("/entity/a", "Position3D"),
            make_component_column("/entity/b", "Position3D"),
        ];

        // /entity/unknown is not present in columns and should be ignored.
        let order = vec![
            EntityPath::from("/entity/unknown"),
            EntityPath::from("/entity/b"),
            EntityPath::from("/entity/a"),
        ];

        let result = reorder_columns_by_entity(&columns, &order);

        assert_eq!(result.len(), 2);
        assert_eq!(entity_path_of(&result[0]).to_string(), "/entity/b");
        assert_eq!(entity_path_of(&result[1]).to_string(), "/entity/a");
    }

    #[test]
    fn test_apply_entity_order_none() {
        // Empty order = original order preserved.
        let columns = vec![
            make_component_column("/entity/a", "Position3D"),
            make_component_column("/entity/b", "Position3D"),
            make_component_column("/entity/c", "Position3D"),
        ];

        let result = reorder_columns_by_entity(&columns, &[]);

        assert_eq!(result.len(), 3);
        assert_eq!(entity_path_of(&result[0]).to_string(), "/entity/a");
        assert_eq!(entity_path_of(&result[1]).to_string(), "/entity/b");
        assert_eq!(entity_path_of(&result[2]).to_string(), "/entity/c");
    }
}
