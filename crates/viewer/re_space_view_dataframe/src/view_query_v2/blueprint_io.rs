use crate::view_query_v2::{EventColumn, QueryV2};
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_types::blueprint::{components, datatypes};
use re_types_core::ComponentName;
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
            self.set_timeline_name(ctx, timeline.name());
        }

        Ok(timeline)
    }

    /// Save the timeline to the one specified.
    ///
    /// Note: this resets the range filter timestamps to -inf/+inf as any other value might be
    /// invalidated.
    pub(super) fn set_timeline_name(&self, ctx: &ViewerContext<'_>, timeline_name: &TimelineName) {
        self.query_property
            .save_blueprint_component(ctx, &components::TimelineName::from(timeline_name.as_str()));

        // clearing the range filter is equivalent to setting it to the default -inf/+inf
        self.query_property
            .clear_blueprint_component::<components::RangeFilter>(ctx);
    }

    pub(crate) fn range_filter(&self) -> Result<(TimeInt, TimeInt), SpaceViewSystemExecutionError> {
        #[allow(clippy::map_unwrap_or)]
        Ok(self
            .query_property
            .component_or_empty::<components::RangeFilter>()?
            .map(|range_filter| (range_filter.start.into(), range_filter.end.into()))
            .unwrap_or((TimeInt::MIN, TimeInt::MAX)))
    }

    pub(super) fn set_range_filter(&self, ctx: &ViewerContext<'_>, start: TimeInt, end: TimeInt) {
        if (start, end) == (TimeInt::MIN, TimeInt::MAX) {
            self.query_property
                .clear_blueprint_component::<components::RangeFilter>(ctx);
        } else {
            self.query_property
                .save_blueprint_component(ctx, &components::RangeFilter::new(start, end));
        }
    }

    pub(crate) fn filter_by_event_active(&self) -> Result<bool, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::FilterByEventActive>()?
            .map_or(false, |comp| *comp.0))
    }

    pub(super) fn set_filter_by_event_active(&self, ctx: &ViewerContext<'_>, active: bool) {
        self.query_property
            .save_blueprint_component(ctx, &components::FilterByEventActive(active.into()));
    }

    pub(crate) fn filter_event_column(
        &self,
    ) -> Result<Option<EventColumn>, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::ComponentColumnSelector>()?
            .map(|comp| {
                let components::ComponentColumnSelector(datatypes::ComponentColumnSelector {
                    entity_path,
                    component,
                }) = comp;

                EventColumn {
                    entity_path: EntityPath::from(entity_path.as_str()),
                    component_name: ComponentName::from(component.as_str()),
                }
            }))
    }

    pub(super) fn set_filter_event_column(
        &self,
        ctx: &ViewerContext<'_>,
        event_column: EventColumn,
    ) {
        let EventColumn {
            entity_path,
            component_name,
        } = event_column;

        let component = components::ComponentColumnSelector::new(&entity_path, component_name);

        self.query_property
            .save_blueprint_component(ctx, &component);
    }

    pub(crate) fn latest_at(&self) -> Result<bool, SpaceViewSystemExecutionError> {
        Ok(self
            .query_property
            .component_or_empty::<components::ApplyLatestAt>()?
            .map_or(false, |comp| *comp.0))
    }

    pub(crate) fn set_latest_at(&self, ctx: &ViewerContext<'_>, latest_at: bool) {
        self.query_property
            .save_blueprint_component(ctx, &components::ApplyLatestAt(latest_at.into()));
    }
}
