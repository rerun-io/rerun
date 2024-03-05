use re_log_types::{DataCell, DataRow, EntityPath, RowId, Time, TimePoint, Timeline};
use re_types::{components::InstanceKey, ComponentBatch, ComponentName};

use crate::{SystemCommand, SystemCommandSender as _, ViewerContext};

#[inline]
pub fn blueprint_timeline() -> Timeline {
    Timeline::new_temporal("blueprint")
}

/// The timepoint to use when writing an update to the blueprint.
#[inline]
pub fn blueprint_timepoint_for_writes() -> TimePoint {
    TimePoint::from([(blueprint_timeline(), Time::now().into())])
}

impl ViewerContext<'_> {
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
        let timepoint = blueprint_timepoint_for_writes();

        let data_row_result = if num_instances == 1 {
            let mut splat_cell: DataCell = [InstanceKey::SPLAT].into();
            splat_cell.compute_size_bytes();

            DataRow::from_cells(
                RowId::new(),
                timepoint.clone(),
                entity_path.clone(),
                num_instances,
                [splat_cell, data_cell],
            )
        } else {
            DataRow::from_cells(
                RowId::new(),
                timepoint.clone(),
                entity_path.clone(),
                num_instances,
                [data_cell],
            )
        };

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
        store: &re_data_store::DataStore,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        let Some(datatype) = store.lookup_datatype(&component_name) else {
            re_log::error_once!(
                "Tried to clear a component with unknown type: {}",
                component_name
            );
            return;
        };

        let timepoint = blueprint_timepoint_for_writes();
        let cell = DataCell::from_arrow_empty(component_name, datatype.clone());

        match DataRow::from_cells1(
            RowId::new(),
            entity_path.clone(),
            timepoint.clone(),
            cell.num_instances(),
            cell,
        ) {
            Ok(row) => self
                .command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    self.store_context.blueprint.store_id().clone(),
                    vec![row],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create DataRow for blueprint component: {}", err);
            }
        }
    }
}
