use re_log_types::{
    external::arrow2::datatypes::DataType, DataCell, DataRow, EntityPath, RowId, Time, TimePoint,
    Timeline,
};
use re_types::{components::InstanceKey, ComponentName};

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
    /// Helper to save a component to the blueprint store.
    pub fn save_blueprint_component<'a, C>(&self, entity_path: &EntityPath, component: C)
    where
        C: re_types::Component + Clone + 'a,
        std::borrow::Cow<'a, C>: std::convert::From<C>,
    {
        let timepoint = blueprint_timepoint_for_writes();

        let mut splat_cell: DataCell = [InstanceKey::SPLAT].into();
        splat_cell.compute_size_bytes();

        let mut component: DataCell = [component].into();
        component.compute_size_bytes();

        match DataRow::from_cells(
            RowId::new(),
            timepoint.clone(),
            entity_path.clone(),
            1,
            [splat_cell, component],
        ) {
            Ok(row) => self
                .command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    self.store_context.blueprint.store_id().clone(),
                    vec![row],
                )),
            Err(err) => {
                // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!
                re_log::error_once!("Failed to create DataRow for blueprint component: {}", err);
            }
        }
    }

    /// Helper for `save_empty_blueprint_component` and `save_empty_blueprint_component_name`.
    fn save_empty_blueprint_component_impl(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
        datatype: DataType,
    ) {
        let timepoint = blueprint_timepoint_for_writes();
        let cell = DataCell::from_arrow_empty(component_name, datatype);

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
                // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!
                re_log::error_once!("Failed to create DataRow for blueprint component: {}", err);
            }
        }
    }

    /// Helper to save a component to the blueprint store.
    pub fn save_empty_blueprint_component<'a, C>(&self, entity_path: &EntityPath)
    where
        C: re_types::Component + 'a,
    {
        self.save_empty_blueprint_component_impl(
            entity_path,
            C::name(),
            C::arrow_datatype().clone(),
        );
    }

    /// Helper to save a component to the blueprint store.
    pub fn save_empty_blueprint_component_name(
        &self,
        store: &re_data_store::DataStore,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        if let Some(datatype) = store.lookup_datatype(&component_name) {
            self.save_empty_blueprint_component_impl(entity_path, component_name, datatype.clone());
        } else {
            re_log::error_once!(
                "Tried to clear a component with unknown type: {}",
                component_name
            );
        }
    }
}
