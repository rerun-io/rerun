use ahash::HashMap;
use nohash_hasher::{IntMap, IntSet};
use re_data_store::{StoreDiff, StoreSubscriber};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{
    components::{DisconnectedSpace, PinholeProjection},
    Loggable,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SubSpaceDimensionality {
    /// We don't know if this space is in a 2D or 3D space.
    ///
    /// This is the most common case and happens whenever there's no projection that
    /// establishes a clear distinction between 2D and 3D spaces.
    ///
    /// Note that this can both mean "there are both 2D and 3D relationships" as well as
    /// "there are not spatial relationships at all".
    Unknown,

    /// The space is definitively a 2D space.
    ///
    /// This conclusion is usually reached by the presence of a projection operation.
    TwoD,

    /// The space is definitively a 3D space.
    ///
    /// This conclusion is usually reached by the presence of a projection operation.
    ThreeD,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SubSpaceConnection {
    Disconnected,
    Pinhole,
}

/// Within a subspace all
pub struct SubSpace {
    /// The transform root of this subspace.
    pub origin: EntityPath,

    pub space_type: SubSpaceDimensionality,

    /// All entities that are part of this subspace.
    ///
    /// Contains the origin entity as well unless the origin is the root and nothing was logged to it directly.
    pub entities: IntSet<EntityPath>,

    /// Origin paths of child spaces.
    ///
    /// This implies that there is a either an explicit disconnect or
    /// a projection at the origin of the child space.
    /// How it is connected is implied by `parent_space` in the child space.
    pub child_spaces: IntSet<EntityPath>,

    /// Origin of the parent space if any and how it's connected.
    pub parent_space: Option<(EntityPathHash, SubSpaceConnection)>,
    //
    // TODO(andreas): We could (and should) add here additional transform hierarchy information within this space.
}

impl SubSpace {
    /// Splits out a subspace into a new subspace with the given entity path as origin.
    ///
    /// The given entity path must be a descendant of the current subspace's origin, but does
    /// not need to be part of [`SubSpace::entities`].
    /// There musn't be any other subspaces with the given entity path as origin already.
    #[must_use]
    fn split(
        &mut self,
        new_space_origin: &EntityPath,
        connection: SubSpaceConnection,
        subspace_origin_per_entity: &mut IntMap<EntityPathHash, EntityPathHash>,
    ) -> SubSpace {
        debug_assert!(new_space_origin.is_descendant_of(&self.origin));

        self.update_dimensionality(new_space_origin, connection);

        // Determine the space type of the new space and update the current space's space type if necessary.
        let new_space_type = match connection {
            SubSpaceConnection::Pinhole => SubSpaceDimensionality::TwoD,
            SubSpaceConnection::Disconnected => SubSpaceDimensionality::Unknown,
        };

        let mut new_space = SubSpace {
            origin: new_space_origin.clone(),
            space_type: new_space_type,
            entities: std::iter::once(new_space_origin.clone()).collect(),
            child_spaces: Default::default(),
            parent_space: Some((self.origin.hash(), connection)),
        };

        let is_new_child_space = self.child_spaces.insert(new_space_origin.clone());
        debug_assert!(is_new_child_space);

        // Transfer entities from self to the new space if they're children of the new space.
        self.entities.retain(|e| {
            if e.is_child_of(new_space_origin) {
                subspace_origin_per_entity.insert(e.hash(), new_space.origin.hash());
                new_space.entities.insert(e.clone());
                false
            } else {
                true
            }
        });

        new_space
    }

    /// Updates dimensionality based on a new connection to a child space.
    fn update_dimensionality(
        &mut self,
        child_path: &EntityPath,
        connection_to_child: SubSpaceConnection,
    ) {
        match connection_to_child {
            SubSpaceConnection::Pinhole => {
                match self.space_type {
                    SubSpaceDimensionality::Unknown => {
                        self.space_type = SubSpaceDimensionality::ThreeD;
                    }
                    SubSpaceDimensionality::TwoD => {
                        // For the moment the only way to get a 2D space is by defining a pinhole,
                        // but in the future other projections may also cause a space to be defined as 2D space.
                        re_log::warn_once!("There was already a pinhole logged at {:?}.
The new pinhole at {:?} is nested under it, implying an invalid projection from a 2D space to a 2D space.", self.origin, child_path);
                        // We keep 2D.
                    }
                    SubSpaceDimensionality::ThreeD => {
                        // Already 3D.
                    }
                }
            }
            SubSpaceConnection::Disconnected => {
                // Does not affect dimensionality.
            }
        };
    }
}

pub struct SpatialTopologyStoreSubscriber {
    topologies: HashMap<StoreId, SpatialTopology>,
}

impl StoreSubscriber for SpatialTopologyStoreSubscriber {
    fn name(&self) -> String {
        "SpatialTopologyStoreSubscriber".to_owned()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[re_data_store::StoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            if event.diff.kind != re_data_store::StoreDiffKind::Addition {
                // Topology is only additive, don't care about removals.
                continue;
            }

            self.topologies
                .entry(event.store_id.clone())
                .or_default()
                .on_store_diff(&event.diff);
        }
    }
}

/// Topological information about a store.
///
/// Describes how 2D & 3D spaces are connected/disconnected.
///
/// Used to determine whether 2D/3D visualizers are applicable and to inform
/// space view generation heuristics.
///
/// Spatial topology is time independent but may change as new data comes in.
/// Generally, the assumption is that topological cuts stay constant over time.
pub struct SpatialTopology {
    subspaces: IntMap<EntityPathHash, SubSpace>,

    /// Maps each entity to the origin of a subspace.
    subspace_origin_per_entity: IntMap<EntityPathHash, EntityPathHash>,
}

impl Default for SpatialTopology {
    fn default() -> Self {
        Self {
            subspaces: std::iter::once((
                EntityPath::root().hash(),
                SubSpace {
                    origin: EntityPath::root(),
                    space_type: SubSpaceDimensionality::Unknown,
                    entities: IntSet::default(),
                    child_spaces: IntSet::default(),
                    parent_space: None,
                },
            ))
            .collect(),

            subspace_origin_per_entity: Default::default(),
        }
    }
}

impl SpatialTopology {
    fn on_store_diff(&mut self, diff: &StoreDiff) {
        re_tracing::profile_function!();
        // Does this add a new space?
        let subspace_connection = if diff.cells.keys().any(|c| c == &DisconnectedSpace::name()) {
            Some(SubSpaceConnection::Pinhole)
        } else if diff.cells.keys().any(|c| c == &PinholeProjection::name()) {
            Some(SubSpaceConnection::Disconnected)
        } else {
            None
        };

        // Is there already a space with this entity?
        if let Some(subspace_origin) = self
            .subspace_origin_per_entity
            .get(&diff.entity_path.hash())
            .cloned()
        {
            // In that case, this causes only changes if there's a change in connection.
            if let Some(new_connection) = subspace_connection {
                self.update_space_with_new_connection(
                    &diff.entity_path,
                    subspace_origin,
                    new_connection,
                );
            }
        } else {
            self.add_new_entity(&diff.entity_path, subspace_connection);
        };
    }

    fn update_space_with_new_connection(
        &mut self,
        entity_path: &EntityPath,
        subspace_origin: EntityPathHash,
        new_connection: SubSpaceConnection,
    ) {
        let subspace = self
            .subspaces
            .get_mut(&subspace_origin)
            .expect("Subspace origin not part of origin->subspace map.");

        if &subspace.origin == entity_path {
            // If this is the origin of a space we can't split it.
            // Instead we have to update connectivity dimensionality.
            if let Some((parent_origin, connection_to_parent)) = subspace.parent_space.as_mut() {
                // Disconnect is the most pervasive connection, so we always update to that.
                match new_connection {
                    SubSpaceConnection::Disconnected => {
                        *connection_to_parent = SubSpaceConnection::Disconnected;
                    }
                    SubSpaceConnection::Pinhole => {
                        // Keep disconnected spaces disconnected.
                    }
                }

                // Stop borrowing self.subspaces.
                let parent_origin = *parent_origin;

                self.subspaces
                    .get_mut(&parent_origin)
                    .expect("Parent origin not part of origin->subspace map.")
                    .update_dimensionality(entity_path, new_connection);
            }
        } else {
            // Split the existing subspace.
            let new_subspace = subspace.split(
                entity_path,
                new_connection,
                &mut self.subspace_origin_per_entity,
            );
            self.subspaces
                .insert(new_subspace.origin.hash(), new_subspace);
        }
    }

    /// Adds a new entity to the spatial topology that wasn't known before.
    fn add_new_entity(
        &mut self,
        entity_path: &EntityPath,
        subspace_connection: Option<SubSpaceConnection>,
    ) {
        let subspace =
            Self::find_subspace_rec_mut(&mut self.subspaces, &EntityPath::root(), entity_path);

        if let Some(connection) = subspace_connection {
            let new_subspace = subspace.split(
                entity_path,
                connection,
                &mut self.subspace_origin_per_entity,
            );
            self.subspaces
                .insert(new_subspace.origin.hash(), new_subspace);
        } else {
            // Add entity to the existing space.
            subspace.entities.insert(entity_path.clone());
            let origin_hash = subspace.origin.hash();
            self.subspace_origin_per_entity
                .insert(entity_path.hash(), origin_hash);
        };
    }

    /// Finds subspace an entity path belongs to by recursively walking down the hierarchy.
    ///
    /// Only use this when we haven't yet established a subspace for this entity path.
    fn find_subspace_rec_mut<'a>(
        subspaces: &'a mut IntMap<EntityPathHash, SubSpace>,
        subspace_origin: &EntityPath,
        path: &EntityPath,
    ) -> &'a mut SubSpace {
        debug_assert!(path.is_child_of(subspace_origin));

        let subspace = subspaces
            .get(&subspace_origin.hash())
            .expect("Subspace origin not part of origin->subspace map.");

        debug_assert!(&subspace.origin == subspace_origin);

        for child_space_origin in &subspace.child_spaces {
            if path.is_child_of(child_space_origin) {
                // Clone to lift borrow on self.
                let child_space_origin = child_space_origin.clone();
                return Self::find_subspace_rec_mut(subspaces, &child_space_origin, path);
            }
        }

        // Need to query subspace again since otherwise we'd have a mutable borrow on self while trying to do a recursive call.
        return subspaces.get_mut(&subspace_origin.hash()).unwrap(); // Unwrap is safe since we succeeded ist just earlier.
    }
}
