use std::hash::BuildHasher as _;

use once_cell::sync::Lazy;

use re_log_types::{EntityPath, EntityPathPart};

pub trait BlueprintIdRegistry {
    fn registry_name() -> &'static str;
    fn registry_path() -> &'static EntityPath;
}

/// A unique id for a type of Blueprint contents.
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
pub struct BlueprintId<T: BlueprintIdRegistry> {
    id: uuid::Uuid,
    #[serde(skip)]
    _registry: std::marker::PhantomData<T>,
}

impl<T: BlueprintIdRegistry> BlueprintId<T> {
    pub fn invalid() -> Self {
        Self {
            id: uuid::Uuid::nil(),
            _registry: std::marker::PhantomData,
        }
    }

    pub fn random() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            _registry: std::marker::PhantomData,
        }
    }

    pub fn from_entity_path(path: &EntityPath) -> Self {
        if !path.is_child_of(T::registry_path()) {
            return Self::invalid();
        }

        path.last()
            .and_then(|last| uuid::Uuid::parse_str(last.unescaped_str()).ok())
            .map_or(Self::invalid(), |id| Self {
                id,
                _registry: std::marker::PhantomData,
            })
    }

    pub fn hashed_from_str(s: &str) -> Self {
        use std::hash::{Hash as _, Hasher as _};

        let salt1: u64 = 0x307b_e149_0a3a_5552;
        let salt2: u64 = 0x6651_522f_f510_13a4;

        let hash1 = {
            let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
            salt1.hash(&mut hasher);
            s.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
            salt2.hash(&mut hasher);
            s.hash(&mut hasher);
            hasher.finish()
        };

        let uuid = uuid::Uuid::from_u64_pair(hash1, hash2);

        uuid.into()
    }

    pub fn gpu_readback_id(self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash(self.id).hash64()
    }

    #[inline]
    pub fn as_entity_path(&self) -> EntityPath {
        T::registry_path()
            .iter()
            .cloned()
            .chain(std::iter::once(EntityPathPart::new(self.id.to_string())))
            .collect()
    }

    #[inline]
    pub fn registry() -> &'static EntityPath {
        T::registry_path()
    }

    #[inline]
    pub fn registry_part() -> &'static EntityPathPart {
        &T::registry_path().as_slice()[0]
    }

    #[inline]
    pub fn uuid(&self) -> uuid::Uuid {
        self.id
    }

    #[inline]
    pub fn hash(&self) -> u64 {
        re_log_types::hash::Hash64::hash(self.id).hash64()
    }
}

impl<T: BlueprintIdRegistry> From<uuid::Uuid> for BlueprintId<T> {
    #[inline]
    fn from(id: uuid::Uuid) -> Self {
        Self {
            id,
            _registry: std::marker::PhantomData,
        }
    }
}

impl<T: BlueprintIdRegistry> From<re_types::datatypes::Uuid> for BlueprintId<T> {
    #[inline]
    fn from(id: re_types::datatypes::Uuid) -> Self {
        Self {
            id: id.into(),
            _registry: std::marker::PhantomData,
        }
    }
}

impl<T: BlueprintIdRegistry> From<BlueprintId<T>> for re_types::datatypes::Uuid {
    #[inline]
    fn from(id: BlueprintId<T>) -> Self {
        id.id.into()
    }
}

impl<T: BlueprintIdRegistry> std::fmt::Display for BlueprintId<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", T::registry_name(), self.id.simple())
    }
}

impl<T: BlueprintIdRegistry> std::fmt::Debug for BlueprintId<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", T::registry_name(), self.id.simple())
    }
}

// ----------------------------------------------------------------------------
/// Helper to define a new [`BlueprintId`] type.
macro_rules! define_blueprint_id_type {
    ($type:ident, $registry:ident, $registry_name:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $registry;

        impl $registry {
            const REGISTRY: &'static str = $registry_name;
        }

        impl BlueprintIdRegistry for $registry {
            fn registry_name() -> &'static str {
                stringify!($type)
            }

            fn registry_path() -> &'static EntityPath {
                static REGISTRY_PATH: Lazy<EntityPath> = Lazy::new(|| $registry::REGISTRY.into());
                &REGISTRY_PATH
            }
        }

        pub type $type = BlueprintId<$registry>;
    };
}

// ----------------------------------------------------------------------------
// Definitions for the different [`BlueprintId`] types.
define_blueprint_id_type!(ViewId, ViewIdRegistry, "view");
define_blueprint_id_type!(ContainerId, ContainerIdRegistry, "container");

// ----------------------------------------------------------------------------
// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blueprint_id() {
        let id = ViewId::random();
        let path = id.as_entity_path();
        assert!(path.is_child_of(&EntityPath::parse_forgiving("view/")));

        let id = ContainerId::random();
        let path = id.as_entity_path();
        assert!(path.is_child_of(&EntityPath::parse_forgiving("container/")));

        let roundtrip = ContainerId::from_entity_path(&id.as_entity_path());
        assert_eq!(roundtrip, id);

        let crossed = ContainerId::from_entity_path(&ViewId::random().as_entity_path());
        assert_eq!(crossed, ContainerId::invalid());
    }
}
