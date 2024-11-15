use once_cell::sync::Lazy;
use parking_lot::RwLock;
use re_sdk::ComponentName;

use crate::{CComponentTypeHandle, CError, CErrorCode};

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
        #[cfg(debug_assertions)]
        {
            for ty in &self.types {
                assert_ne!(
                    ty.name, name,
                    "Component type with the same name already registered"
                );
            }
        }

        let id = self.next_id;
        self.next_id += 1;
        self.types.push(ComponentType { name, datatype });
        id
    }

    #[allow(clippy::result_large_err)]
    pub fn get(&self, id: CComponentTypeHandle) -> Result<&ComponentType, CError> {
        self.types.get(id as usize).ok_or_else(|| {
            CError::new(
                CErrorCode::InvalidComponentTypeHandle,
                &format!("Invalid component type handle: {id}"),
            )
        })
    }
}

/// All registered component types.
pub static COMPONENT_TYPES: Lazy<RwLock<ComponentTypeRegistry>> = Lazy::new(RwLock::default);
