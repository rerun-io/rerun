use re_log_types::{DataCell, DataRow, EntityPath, RowId, TimeInt, TimePoint, Timeline};
use re_types::{AsComponents, ComponentBatch, ComponentName};

use crate::{StoreContext, SystemCommand, SystemCommandSender as _, ViewerContext};

#[inline]
pub fn blueprint_timeline() -> Timeline {
    Timeline::new_sequence("blueprint")
}

impl StoreContext<'_> {
    /// The timepoint to use when writing an update to the blueprint.
    #[inline]
    pub fn blueprint_timepoint_for_writes(&self) -> TimePoint {
        let timeline = blueprint_timeline();

        let max_time = self
            .blueprint
            .times_per_timeline()
            .get(&timeline)
            .and_then(|times| times.last_key_value())
            .map_or(0, |(time, _)| time.as_i64())
            .saturating_add(1);

        TimePoint::from([(timeline, TimeInt::new_temporal(max_time))])
    }
}

impl ViewerContext<'_> {
    pub fn save_blueprint_archetype(&self, entity_path: EntityPath, components: &dyn AsComponents) {
        let timepoint = self.store_context.blueprint_timepoint_for_writes();

        let data_row =
            match DataRow::from_archetype(RowId::new(), timepoint.clone(), entity_path, components)
            {
                Ok(data_cell) => data_cell,
                Err(err) => {
                    re_log::error_once!(
                        "Failed to create DataRow for blueprint components: {}",
                        err
                    );
                    return;
                }
            };

        self.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                self.store_context.blueprint.store_id().clone(),
                vec![data_row],
            ));
    }

    /// Helper to save a component batch to the blueprint store.
    pub fn save_blueprint_component(
        &self,
        entity_path: &EntityPath,
        components: &dyn ComponentBatch,
    ) {
        let mut data_cell = match DataCell::from_component_batch(components) {
            Ok(data_cell) => data_cell,
            Err(err) => {
                re_log::error_once!(
                    "Failed to create DataCell for blueprint components: {}",
                    err
                );
                return;
            }
        };
        data_cell.compute_size_bytes();

        let num_instances = components.num_instances() as u32;
        let timepoint = self.store_context.blueprint_timepoint_for_writes();

        re_log::trace!(
            "Writing {} components of type {:?} to {:?}",
            num_instances,
            components.name(),
            entity_path
        );

        let data_row_result = DataRow::from_cells(
            RowId::new(),
            timepoint.clone(),
            entity_path.clone(),
            [data_cell],
        );

        match data_row_result {
            Ok(row) => self
                .command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    self.store_context.blueprint.store_id().clone(),
                    vec![row],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create DataRow for blueprint components: {}", err);
            }
        }
    }

    /// Helper to save a component to the blueprint store.
    pub fn save_empty_blueprint_component<'a, C>(&self, entity_path: &EntityPath)
    where
        C: re_types::Component + 'a,
    {
        let empty: [C; 0] = [];
        self.save_blueprint_component(entity_path, &empty);
    }

    /// Helper to save a component to the blueprint store.
    pub fn save_empty_blueprint_component_name(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        let blueprint = &self.store_context.blueprint;
        let Some(datatype) = blueprint.store().lookup_datatype(&component_name) else {
            re_log::error_once!(
                "Tried to clear a component with unknown type: {}",
                component_name
            );
            return;
        };

        let timepoint = self.store_context.blueprint_timepoint_for_writes();
        let cell = DataCell::from_arrow_empty(component_name, datatype.clone());

        match DataRow::from_cells1(RowId::new(), entity_path.clone(), timepoint.clone(), cell) {
            Ok(row) => self
                .command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    blueprint.store_id().clone(),
                    vec![row],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create DataRow for blueprint component: {}", err);
            }
        }
    }
}
