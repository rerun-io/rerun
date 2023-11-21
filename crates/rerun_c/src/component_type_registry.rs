use once_cell::sync::Lazy;
use parking_lot::RwLock;
use re_sdk::ComponentName;

use crate::CComponentTypeHandle;

pub struct ComponentType {
    pub name: ComponentName,
    pub datatype: arrow2::datatypes::DataType,
}

#[derive(Default)]
pub struct ComponentTypeRegistry {
    next_id: CComponentTypeHandle,
    types: Vec<ComponentType>,
}

impl ComponentTypeRegistry {
    pub fn register(
        &mut self,
        name: ComponentName,
        datatype: arrow2::datatypes::DataType,
    ) -> CComponentTypeHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.types.push(ComponentType { name, datatype });
        id
    }

    pub fn get(&self, id: CComponentTypeHandle) -> Option<&ComponentType> {
        self.types.get(id as usize)
    }
}

/// All registered component types.
pub static COMPONENT_TYPES: Lazy<RwLock<ComponentTypeRegistry>> = Lazy::new(RwLock::default);
