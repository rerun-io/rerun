use parking_lot::RwLock;
use re_sdk::ComponentDescriptor;

use crate::{CComponentTypeHandle, CError, CErrorCode};

pub struct ComponentType {
    pub descriptor: ComponentDescriptor,
    pub datatype: arrow::datatypes::DataType,
}

#[derive(Default)]
pub struct ComponentTypeRegistry {
    next_id: CComponentTypeHandle,
    types: Vec<ComponentType>,
}

impl ComponentTypeRegistry {
    pub fn register(
        &mut self,
        descriptor: ComponentDescriptor,
        datatype: arrow::datatypes::DataType,
    ) -> CComponentTypeHandle {
        #[cfg(debug_assertions)]
        {
            for ty in &self.types {
                assert_ne!(
                    ty.descriptor, descriptor,
                    "Component type with the same descriptor already registered"
                );
            }
        }

        let id = self.next_id;
        self.next_id += 1;
        self.types.push(ComponentType {
            descriptor,
            datatype,
        });
        id
    }

    #[expect(clippy::result_large_err)]
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
pub static COMPONENT_TYPES: std::sync::LazyLock<RwLock<ComponentTypeRegistry>> =
    std::sync::LazyLock::new(RwLock::default);
