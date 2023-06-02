use ahash::HashMap;

use crate::ScenePart;

#[derive(Debug, thiserror::Error)]
pub enum ScenePartCollectionLookupError {
    #[error("Type not found in collection")]
    TypeNotFound,

    #[error("Failed to downcast type.")]
    DowncastFailure,
}

/// Collections of scene parts.
#[derive(Default)]
pub struct ScenePartCollection(HashMap<std::any::TypeId, Box<dyn ScenePart>>);

impl ScenePartCollection {
    pub fn get<T: ScenePart>(&self) -> Result<&T, ScenePartCollectionLookupError> {
        self.0
            .get(&std::any::TypeId::of::<T>())
            .ok_or(ScenePartCollectionLookupError::TypeNotFound)?
            .as_any()
            .downcast_ref::<T>()
            .ok_or(ScenePartCollectionLookupError::DowncastFailure)
    }

    pub fn get_mut<T: ScenePart>(&mut self) -> Result<&mut T, ScenePartCollectionLookupError> {
        self.0
            .get_mut(&std::any::TypeId::of::<T>())
            .ok_or(ScenePartCollectionLookupError::TypeNotFound)?
            .as_any_mut()
            .downcast_mut::<T>()
            .ok_or(ScenePartCollectionLookupError::DowncastFailure)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn ScenePart>> {
        self.0.values()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn ScenePart>> {
        self.0.values_mut()
    }
}

macro_rules! scene_part_collection_from_tuple {
    ($($idx:tt => $name:ident),*) => {
        impl<$($name: ScenePart),*> From<($($name,)*)> for ScenePartCollection {
            #[allow(unused_mut)]
            fn from(_value: ($($name,)*)) -> Self {
                let mut map = HashMap::<std::any::TypeId, Box<dyn ScenePart>>::default();
                $(
                    map.insert(_value.$idx.as_any().type_id(), Box::new(_value.$idx));
                )*
                Self(map)
            }
        }
    };
}

scene_part_collection_from_tuple!();
scene_part_collection_from_tuple!(0 => T0);
scene_part_collection_from_tuple!(0 => T0, 1 => T1);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6, 7 => T7);
scene_part_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6, 7 => T7, 8 => T8);
