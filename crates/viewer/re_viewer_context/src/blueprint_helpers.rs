use arrow::array::ArrayRef;
use re_chunk::{RowId, TimelineName};
use re_chunk_store::external::re_chunk::Chunk;
use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline};
use re_types::{
    AsComponents, ComponentBatch, ComponentDescriptor, ComponentName, SerializedComponentBatch,
};

use crate::{StoreContext, SystemCommand, SystemCommandSender as _, ViewerContext};

#[inline]
pub fn blueprint_timeline() -> TimelineName {
    TimelineName::new("blueprint")
}

/// The timepoint to use when writing an update to the blueprint.
pub fn blueprint_timepoint_for_writes(blueprint: &re_entity_db::EntityDb) -> TimePoint {
    let timeline = Timeline::new_sequence(blueprint_timeline());

    let max_time = blueprint
        .time_histogram(timeline.name())
        .and_then(|times| times.max_key())
        .unwrap_or(0)
        .saturating_add(1);

    TimePoint::from([(timeline, TimeInt::new_temporal(max_time))])
}

impl StoreContext<'_> {
    /// The timepoint to use when writing an update to the blueprint.
    #[inline]
    pub fn blueprint_timepoint_for_writes(&self) -> TimePoint {
        blueprint_timepoint_for_writes(self.blueprint)
    }
}

impl ViewerContext<'_> {
    pub fn save_blueprint_archetype(
        &self,
        entity_path: &EntityPath,
        components: &dyn AsComponents,
    ) {
        let timepoint = self.store_context.blueprint_timepoint_for_writes();

        let chunk = match Chunk::builder(entity_path.clone())
            .with_archetype(RowId::new(), timepoint.clone(), components)
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {}", err);
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::UpdateBlueprint(
                self.store_context.blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    pub fn save_blueprint_component(
        &self,
        entity_path: &EntityPath,
        component_batch: &dyn ComponentBatch,
    ) {
        match component_batch.try_serialized() {
            Ok(serialized) => self.save_serialized_blueprint_component(entity_path, serialized),
            Err(err) => {
                re_log::error_once!("Failed to serialize component batch: {}", err);
            }
        }
    }

    pub fn save_serialized_blueprint_component(
        &self,
        entity_path: &EntityPath,
        component_batch: SerializedComponentBatch,
    ) {
        let timepoint = self.store_context.blueprint_timepoint_for_writes();

        let chunk = match Chunk::builder(entity_path.clone())
            .with_serialized_batch(RowId::new(), timepoint.clone(), component_batch)
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {}", err);
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::UpdateBlueprint(
                self.store_context.blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    pub fn save_blueprint_array(
        &self,
        entity_path: &EntityPath,
        component_descr: ComponentDescriptor,
        array: ArrayRef,
    ) {
        let timepoint = self.store_context.blueprint_timepoint_for_writes();

        let chunk = match Chunk::builder(entity_path.clone())
            .with_row(RowId::new(), timepoint.clone(), [(component_descr, array)])
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {}", err);
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::UpdateBlueprint(
                self.store_context.blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    /// Helper to save a component to the blueprint store.
    pub fn save_empty_blueprint_component<'a, C>(&self, entity_path: &EntityPath)
    where
        C: re_types::Component + 'a,
    {
        let empty: [C; 0] = [];
        self.save_blueprint_component(entity_path, &empty);
    }

    /// Queries a raw component from the default blueprint.
    pub fn raw_latest_at_in_default_blueprint(
        &self,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
    ) -> Option<ArrayRef> {
        self.store_context
            .default_blueprint
            .and_then(|default_blueprint| {
                default_blueprint
                    .latest_at(self.blueprint_query, entity_path, [component_descr])
                    .get(component_descr)
                    .and_then(|default_value| default_value.component_batch_raw(component_descr))
            })
    }

    /// Resets a blueprint component to the value it had in the default blueprint.
    pub fn reset_blueprint_component(
        &self,
        entity_path: &EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        if let Some(default_value) =
            self.raw_latest_at_in_default_blueprint(entity_path, &component_descr)
        {
            self.save_blueprint_array(entity_path, component_descr, default_value);
        } else {
            self.clear_blueprint_component(entity_path, component_descr);
        }
    }

    /// Clears a component in the blueprint store by logging an empty array if it exists.
    // TODO(#6889): Remove in favor of `clear_blueprint_component`.
    pub fn clear_blueprint_component_by_name(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        let blueprint = &self.store_context.blueprint;
        if let Some(descr) = blueprint
            .storage_engine()
            .store()
            .entity_component_descriptors_with_name(entity_path, component_name)
            .into_iter()
            .next()
        {
            self.clear_blueprint_component(entity_path, descr);
        }
    }

    /// Clears a component in the blueprint store by logging an empty array if it exists.
    pub fn clear_blueprint_component(
        &self,
        entity_path: &EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        let blueprint = &self.store_context.blueprint;

        let Some(datatype) = blueprint
            .latest_at(self.blueprint_query, entity_path, [&component_descr])
            .get(&component_descr)
            .and_then(|unit| {
                unit.component_batch_raw(&component_descr)
                    .map(|array| array.data_type().clone())
            })
        else {
            // There's no component at this path yet, so there's nothing to clear.
            return;
        };

        let timepoint = self.store_context.blueprint_timepoint_for_writes();
        let chunk = Chunk::builder(entity_path.clone())
            .with_row(
                RowId::new(),
                timepoint,
                [(
                    component_descr,
                    re_chunk::external::arrow::array::new_empty_array(&datatype),
                )],
            )
            .build();

        match chunk {
            Ok(chunk) => self
                .command_sender()
                .send_system(SystemCommand::UpdateBlueprint(
                    blueprint.store_id().clone(),
                    vec![chunk],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint component: {}", err);
            }
        }
    }
}
