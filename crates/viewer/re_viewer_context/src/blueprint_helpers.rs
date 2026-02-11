use arrow::array::ArrayRef;
use re_chunk::{ComponentIdentifier, LatestAtQuery, RowId, TimelineName};
use re_chunk_store::external::re_chunk::Chunk;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, TimeInt, TimePoint, Timeline};
use re_sdk_types::{AsComponents, ComponentBatch, ComponentDescriptor, SerializedComponentBatch};

use crate::{CommandSender, StoreContext, SystemCommand, SystemCommandSender as _, ViewerContext};

#[inline]
pub fn blueprint_timeline() -> TimelineName {
    TimelineName::new("blueprint")
}

/// The timepoint to use when writing an update to the blueprint.
pub fn blueprint_timepoint_for_writes(blueprint: &re_entity_db::EntityDb) -> TimePoint {
    let timeline = Timeline::new_sequence(blueprint_timeline());

    let max_time = blueprint
        .time_range_for(timeline.name())
        .map(|range| range.max.as_i64())
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

/// Helper trait for writing & reading blueprints.
pub trait BlueprintContext {
    fn command_sender(&self) -> &CommandSender;

    fn current_blueprint(&self) -> &EntityDb;

    fn default_blueprint(&self) -> Option<&EntityDb>;

    fn blueprint_query(&self) -> &LatestAtQuery;

    fn save_blueprint_archetype(&self, entity_path: EntityPath, components: &dyn AsComponents) {
        self.save_blueprint_archetypes(entity_path, std::iter::once(components));
    }

    fn save_blueprint_archetypes<'a>(
        &self,
        entity_path: EntityPath,
        archetypes: impl IntoIterator<Item = &'a dyn AsComponents>,
    ) {
        let blueprint = self.current_blueprint();
        let timepoint = blueprint_timepoint_for_writes(blueprint);

        let mut chunk_builder = Chunk::builder(entity_path);
        for archetype in archetypes {
            chunk_builder = chunk_builder.with_archetype_auto_row(timepoint.clone(), archetype);
        }

        let chunk = match chunk_builder.build() {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {err}");
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::AppendToStore(
                blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    fn save_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: &ComponentDescriptor,
        component_batch: &dyn ComponentBatch,
    ) {
        let Some(serialized) = component_batch.serialized(component_descr.clone()) else {
            re_log::warn!("could not serialize components with descriptor `{component_descr}`");
            return;
        };

        self.save_serialized_blueprint_component(entity_path, serialized);
    }

    fn save_static_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: &ComponentDescriptor,
        component_batch: &dyn ComponentBatch,
    ) {
        let Some(serialized) = component_batch.serialized(component_descr.clone()) else {
            re_log::warn!("could not serialize components with descriptor `{component_descr}`");
            return;
        };

        self.save_serialized_static_blueprint_component(entity_path, serialized);
    }

    fn save_serialized_static_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_batch: SerializedComponentBatch,
    ) {
        let blueprint = self.current_blueprint();

        let chunk = match Chunk::builder(entity_path)
            .with_serialized_batch(RowId::new(), TimePoint::STATIC, component_batch)
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {err}");
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::AppendToStore(
                blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    fn save_serialized_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_batch: SerializedComponentBatch,
    ) {
        let blueprint = self.current_blueprint();
        let timepoint = blueprint_timepoint_for_writes(blueprint);

        let chunk = match Chunk::builder(entity_path)
            .with_serialized_batch(RowId::new(), timepoint.clone(), component_batch)
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint components: {err}");
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::AppendToStore(
                blueprint.store_id().clone(),
                vec![chunk],
            ));
    }

    fn save_blueprint_array(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
        array: ArrayRef,
    ) {
        let blueprint = self.current_blueprint();
        let timepoint = blueprint_timepoint_for_writes(blueprint);
        self.append_array_to_store(
            blueprint.store_id().clone(),
            timepoint,
            entity_path,
            component_descr,
            array,
        );
    }

    fn save_static_blueprint_array(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
        array: ArrayRef,
    ) {
        let blueprint = self.current_blueprint();
        self.append_array_to_store(
            blueprint.store_id().clone(),
            TimePoint::STATIC,
            entity_path,
            component_descr,
            array,
        );
    }

    /// Append an array to the given store.
    fn append_array_to_store(
        &self,
        store_id: StoreId,
        timepoint: TimePoint,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
        array: ArrayRef,
    ) {
        let chunk = match Chunk::builder(entity_path)
            .with_row(RowId::new(), timepoint, [(component_descr, array)])
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk: {err}");
                return;
            }
        };

        self.command_sender()
            .send_system(SystemCommand::AppendToStore(store_id, vec![chunk]));
    }

    /// Queries a raw component from the default blueprint.
    fn raw_latest_at_in_default_blueprint(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<ArrayRef> {
        self.default_blueprint()?
            .latest_at(self.blueprint_query(), entity_path, [component])
            .get(component)?
            .component_batch_raw(component)
    }

    /// Resets a blueprint component to the value it had in the default blueprint.
    fn reset_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        if let Some(default_value) =
            self.raw_latest_at_in_default_blueprint(&entity_path, component_descr.component)
        {
            self.save_blueprint_array(entity_path, component_descr, default_value);
        } else {
            self.clear_blueprint_component(entity_path, component_descr);
        }
    }

    /// Resets a static blueprint component to the value it had in the default blueprint.
    fn reset_static_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        if let Some(default_value) =
            self.raw_latest_at_in_default_blueprint(&entity_path, component_descr.component)
        {
            self.save_static_blueprint_array(entity_path, component_descr, default_value);
        } else {
            self.clear_static_blueprint_component(entity_path, component_descr);
        }
    }

    /// Clears a component in the blueprint store by logging an empty array if it exists.
    fn clear_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        let blueprint = self.current_blueprint();
        let component = component_descr.component;

        let Some(datatype) = blueprint
            .latest_at(self.blueprint_query(), &entity_path, [component])
            .get(component)
            .and_then(|unit| {
                unit.component_batch_raw(component)
                    .map(|array| array.data_type().clone())
            })
        else {
            // There's no component at this path yet, so there's nothing to clear.
            return;
        };

        let timepoint = blueprint_timepoint_for_writes(blueprint);
        let chunk = Chunk::builder(entity_path)
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
                .send_system(SystemCommand::AppendToStore(
                    blueprint.store_id().clone(),
                    vec![chunk],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint component: {err}");
            }
        }
    }

    fn clear_static_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        let blueprint = self.current_blueprint();
        let component = component_descr.component;

        let Some(datatype) = blueprint
            .latest_at(self.blueprint_query(), &entity_path, [component])
            .get(component)
            .and_then(|unit| {
                unit.component_batch_raw(component)
                    .map(|array| array.data_type().clone())
            })
        else {
            // There's no component at this path yet, so there's nothing to clear.
            return;
        };

        let chunk = Chunk::builder(entity_path)
            .with_row(
                RowId::new(),
                TimePoint::STATIC,
                [(
                    component_descr,
                    re_chunk::external::arrow::array::new_empty_array(&datatype),
                )],
            )
            .build();

        match chunk {
            Ok(chunk) => self
                .command_sender()
                .send_system(SystemCommand::AppendToStore(
                    blueprint.store_id().clone(),
                    vec![chunk],
                )),
            Err(err) => {
                re_log::error_once!("Failed to create Chunk for blueprint component: {err}");
            }
        }
    }
}

impl BlueprintContext for ViewerContext<'_> {
    fn command_sender(&self) -> &CommandSender {
        self.command_sender()
    }

    fn current_blueprint(&self) -> &EntityDb {
        self.store_context.blueprint
    }

    fn default_blueprint(&self) -> Option<&EntityDb> {
        self.store_context.default_blueprint
    }

    fn blueprint_query(&self) -> &LatestAtQuery {
        self.blueprint_query
    }
}
