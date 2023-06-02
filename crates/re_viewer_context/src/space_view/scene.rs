use std::any::{Any, TypeId};

use ahash::HashMap;

use crate::{ArchetypeDefinition, SceneQuery, SpaceViewHighlights, SpaceViewState, ViewerContext};

#[derive(Debug, thiserror::Error)]
pub enum SceneItemCollectionLookupError {
    #[error("Type not found in collection")]
    TypeNotFound,

    #[error("Failed to downcast type.")]
    DowncastFailure,
}

// TODO(andreas): Use tinyvec for these.

/// Collection of scene contexts.
///
/// New type pattern to support adding From impls.
#[derive(Default)]
pub struct SceneContextCollection(HashMap<TypeId, Box<dyn SceneContext>>);

impl SceneContextCollection {
    pub fn get<T: Any>(&self) -> Result<&T, SceneItemCollectionLookupError> {
        self.0
            .get(&TypeId::of::<T>())
            .ok_or(SceneItemCollectionLookupError::TypeNotFound)?
            .as_any()
            .downcast_ref::<T>()
            .ok_or(SceneItemCollectionLookupError::DowncastFailure)
    }

    pub fn get_mut<T: Any>(&mut self) -> Result<&mut T, SceneItemCollectionLookupError> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .ok_or(SceneItemCollectionLookupError::TypeNotFound)?
            .as_any_mut()
            .downcast_mut::<T>()
            .ok_or(SceneItemCollectionLookupError::DowncastFailure)
    }
}

macro_rules! scene_context_collection_from_tuple {
    ($($idx:tt => $name:ident),*) => {
        impl<$($name: SceneContext),*> From<($($name,)*)> for SceneContextCollection {
            #[allow(unused_mut)]
            fn from(_value: ($($name,)*)) -> Self {
                let mut map = HashMap::<std::any::TypeId, Box<dyn SceneContext>>::default();
                $(
                    map.insert(_value.$idx.as_any().type_id(), Box::new(_value.$idx));
                )*
                Self(map)
            }
        }
    };
}

scene_context_collection_from_tuple!();
scene_context_collection_from_tuple!(0 => T0);
scene_context_collection_from_tuple!(0 => T0, 1 => T1);
scene_context_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2);
scene_context_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3);
scene_context_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4);

/// Collections of scene elements.
///
/// New type pattern to support adding From impls.
#[derive(Default)]
pub struct SceneElementCollection(HashMap<TypeId, Box<dyn SceneElement>>);

impl SceneElementCollection {
    pub fn get<T: Any>(&self) -> Result<&T, SceneItemCollectionLookupError> {
        self.0
            .get(&TypeId::of::<T>())
            .ok_or(SceneItemCollectionLookupError::TypeNotFound)?
            .as_any()
            .downcast_ref::<T>()
            .ok_or(SceneItemCollectionLookupError::DowncastFailure)
    }

    pub fn get_mut<T: Any>(&mut self) -> Result<&mut T, SceneItemCollectionLookupError> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .ok_or(SceneItemCollectionLookupError::TypeNotFound)?
            .as_any_mut()
            .downcast_mut::<T>()
            .ok_or(SceneItemCollectionLookupError::DowncastFailure)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn SceneElement>> {
        self.0.values()
    }
}

macro_rules! scene_element_collection_from_tuple {
    ($($idx:tt => $name:ident),*) => {
        impl<$($name: SceneElement),*> From<($($name,)*)> for SceneElementCollection {
            #[allow(unused_mut)]
            fn from(_value: ($($name,)*)) -> Self {
                let mut map = HashMap::<std::any::TypeId, Box<dyn SceneElement>>::default();
                $(
                    map.insert(_value.$idx.as_any().type_id(), Box::new(_value.$idx));
                )*
                Self(map)
            }
        }
    };
}

scene_element_collection_from_tuple!();
scene_element_collection_from_tuple!(0 => T0);
scene_element_collection_from_tuple!(0 => T0, 1 => T1);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6, 7 => T7);
scene_element_collection_from_tuple!(0 => T0, 1 => T1, 2 => T2, 3 => T3, 4 => T4, 5 => T5, 6 => T6, 7 => T7, 8 => T8);

/// A scene is a collection of scene contexts and elements, as well as a collection of highlights.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
#[derive(Default)]
pub struct Scene {
    pub contexts: SceneContextCollection,
    pub elements: SceneElementCollection,
    pub highlights: SpaceViewHighlights, // TODO(wumpf): Consider making this a scene context - problem: populate can't create it.
}

impl Scene {
    /// List of all archetypes this scene queries for its elements.
    pub fn supported_element_archetypes(&self) -> Vec<ArchetypeDefinition> {
        self.elements.0.values().map(|e| e.archetype()).collect()
    }

    /// Populates the scene for a given query.
    pub fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        highlights: SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_function!();

        self.highlights = highlights;

        // TODO(andreas): Both loops are great candidates for parallelization.
        for context in self.contexts.0.values_mut() {
            // TODO(andreas): Restrict the query with the archetype somehow, ideally making it trivial to do the correct thing.
            context.populate(ctx, query, space_view_state);
        }
        self.elements
            .0
            .values_mut()
            .flat_map(|element| {
                // TODO(andreas): Restrict the query with the archetype somehow, ideally making it trivial to do the correct thing.
                element.populate(
                    ctx,
                    query,
                    space_view_state,
                    &self.contexts,
                    &self.highlights,
                )
            })
            .collect()
    }
}

/// Element of a scene derived from a single archetype query.
pub trait SceneElement: Any {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        contexts: &SceneContextCollection,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// TODO: Doc
pub trait SceneContext: Any {
    /// Each scene context may query several archetypes.
    ///
    /// This lists all components out that the context queries.
    /// A context may also query no archetypes at all and prepare caches or viewer related data instead.
    fn archetypes(&self) -> Vec<ArchetypeDefinition>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
    );

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
