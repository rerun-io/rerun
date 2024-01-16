use once_cell::sync::OnceCell;

use ahash::HashMap;
use nohash_hasher::{IntMap, IntSet};
use re_data_store::{StoreSubscriber, StoreSubscriberHandle};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{
    components::{DisconnectedSpace, PinholeProjection},
    Loggable,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

bitflags::bitflags! {
    #[derive(PartialEq, Eq, Debug, Copy, Clone)]
    pub struct SubSpaceConnectionFlags: u8 {
        const Disconnected = 0b0000001;
        const Pinhole = 0b0000010;
    }
}

/// Within a subspace all
pub struct SubSpace {
    /// The transform root of this subspace.
    pub origin: EntityPath,

    pub dimensionality: SubSpaceDimensionality,

    /// All entities that were logged at any point in time and are part of this subspace.
    ///
    /// Contains the origin entity as well unless the origin is the root and nothing was logged to it directly.
    ///
    /// Note that we this is merely here to speed up queries.
    /// Instead, we could check if an entity is equal to or a descendent of the
    /// origin and not equal or descendent of any child space.
    pub entities: IntSet<EntityPath>,

    /// Origin paths of child spaces.
    ///
    /// This implies that there is a either an explicit disconnect or
    /// a projection at the origin of the child space.
    /// How it is connected is implied by `parent_space` in the child space.
    ///
    /// Any path in `child_spaces` is *not* contained in `entities`.
    /// This implies that the camera itself is not part of its 3D space even when it may still have a 3D transform.
    /// TODO: are we sure about this? why not have it be part of entities _and_ the child space?
    pub child_spaces: IntSet<EntityPath>,

    /// Origin of the parent space if any and how it's connected.
    pub parent_space: Option<(EntityPathHash, SubSpaceConnectionFlags)>,
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
        connections: SubSpaceConnectionFlags,
        subspace_origin_per_entity: &mut IntMap<EntityPathHash, EntityPathHash>,
    ) -> SubSpace {
        debug_assert!(new_space_origin.is_descendant_of(&self.origin));

        self.update_dimensionality(new_space_origin, connections);

        // Determine the space type of the new space and update the current space's space type if necessary.
        let new_space_type = if connections.contains(SubSpaceConnectionFlags::Pinhole) {
            SubSpaceDimensionality::TwoD
        } else {
            SubSpaceDimensionality::Unknown
        };

        let mut new_space = SubSpace {
            origin: new_space_origin.clone(),
            dimensionality: new_space_type,
            entities: std::iter::once(new_space_origin.clone()).collect(),
            child_spaces: Default::default(),
            parent_space: Some((self.origin.hash(), connections)),
        };

        let is_new_child_space = self.child_spaces.insert(new_space_origin.clone());
        debug_assert!(is_new_child_space);

        // Transfer entities from self to the new space if they're children of the new space.
        self.entities.retain(|e| {
            if e.is_descendant_of(new_space_origin) || e == new_space_origin {
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
        new_connections_to_child: SubSpaceConnectionFlags,
    ) {
        if new_connections_to_child.contains(SubSpaceConnectionFlags::Pinhole) {
            match self.dimensionality {
                SubSpaceDimensionality::Unknown => {
                    self.dimensionality = SubSpaceDimensionality::ThreeD;
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
    }
}

#[derive(Default)]
struct SpatialTopologyStoreSubscriber {
    topologies: HashMap<StoreId, SpatialTopology>,
}

impl SpatialTopologyStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> StoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<re_data_store::StoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| {
            re_data_store::DataStore::register_subscriber(
                Box::<SpatialTopologyStoreSubscriber>::default(),
            )
        })
    }
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

            // Possible optimization:
            // only update topologies if an entity is logged the first time or a new relevant component was added.
            self.topologies
                .entry(event.store_id.clone())
                .or_default()
                .on_store_diff(&event.diff.entity_path, event.diff.cells.keys());
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
                    dimensionality: SubSpaceDimensionality::Unknown,
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
    /// Accesses the spatial topology for a given store.
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&SpatialTopology) -> T) -> Option<T> {
        re_data_store::DataStore::with_subscriber_once(
            SpatialTopologyStoreSubscriber::subscription_handle(),
            move |topology_subscriber: &SpatialTopologyStoreSubscriber| {
                topology_subscriber.topologies.get(store_id).map(f)
            },
        )
        .flatten()
    }

    /// Returns the subspace an entity belongs to.
    pub fn subspace_for_entity(&self, entity: &EntityPath) -> &SubSpace {
        // Try the fast track first - we ahve this for all entities that were ever logged.
        if let Some(subspace) = self
            .subspace_origin_per_entity
            .get(&entity.hash())
            .and_then(|origin_hash| self.subspaces.get(origin_hash))
        {
            subspace
        } else {
            // Otherwise, we have to walk the hierarchy.
            self.find_subspace_rec(&EntityPath::root(), entity)
        }
    }

    /// Returns the subspace for a given origin.
    ///
    /// None if the origin doesn't identify its own subspace.
    pub fn subspace_for_origin(&self, origin: EntityPathHash) -> Option<&SubSpace> {
        self.subspaces.get(&origin)
    }

    fn on_store_diff<'a>(
        &mut self,
        entity_path: &EntityPath,
        added_components: impl Iterator<Item = &'a re_types::ComponentName>,
    ) {
        re_tracing::profile_function!();

        let mut new_subspace_connections = SubSpaceConnectionFlags::empty();
        for added_component in added_components {
            if added_component == &DisconnectedSpace::name() {
                new_subspace_connections.insert(SubSpaceConnectionFlags::Disconnected);
            } else if added_component == &PinholeProjection::name() {
                new_subspace_connections.insert(SubSpaceConnectionFlags::Pinhole);
            };
        }

        // Is there already a space with this entity?
        if let Some(subspace_origin) = self
            .subspace_origin_per_entity
            .get(&entity_path.hash())
            .cloned()
        {
            // In that case, this causes only changes if there's a change in connection.
            if !new_subspace_connections.is_empty() {
                self.update_space_with_new_connections(
                    entity_path,
                    subspace_origin,
                    new_subspace_connections,
                );
            }
        } else {
            self.add_new_entity(entity_path, new_subspace_connections);
        };
    }

    fn update_space_with_new_connections(
        &mut self,
        entity_path: &EntityPath,
        subspace_origin: EntityPathHash,
        new_connections: SubSpaceConnectionFlags,
    ) {
        let subspace = self
            .subspaces
            .get_mut(&subspace_origin)
            .expect("Subspace origin not part of origin->subspace map.");

        if &subspace.origin == entity_path {
            // If this is the origin of a space we can't split it.
            // Instead we have to update connectivity dimensionality.
            if let Some((parent_origin, connection_to_parent)) = subspace.parent_space.as_mut() {
                connection_to_parent.insert(new_connections);

                // Stop borrowing self.subspaces.
                let parent_origin = *parent_origin;

                self.subspaces
                    .get_mut(&parent_origin)
                    .expect("Parent origin not part of origin->subspace map.")
                    .update_dimensionality(entity_path, new_connections);
            }
        } else {
            // Split the existing subspace.
            let new_subspace = subspace.split(
                entity_path,
                new_connections,
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
        subspace_connections: SubSpaceConnectionFlags,
    ) {
        let subspace =
            Self::find_subspace_rec_mut(&mut self.subspaces, &EntityPath::root(), entity_path);

        let space_origin_hash;
        if subspace_connections.is_empty() {
            // Add entity to the existing space.
            subspace.entities.insert(entity_path.clone());
            space_origin_hash = subspace.origin.hash();
        } else {
            let new_subspace = subspace.split(
                entity_path,
                subspace_connections,
                &mut self.subspace_origin_per_entity,
            );
            space_origin_hash = new_subspace.origin.hash();
            self.subspaces
                .insert(new_subspace.origin.hash(), new_subspace);
        };

        self.subspace_origin_per_entity
            .insert(entity_path.hash(), space_origin_hash);
    }

    /// Finds subspace an entity path belongs to by recursively walking down the hierarchy.
    ///
    /// Only use this when we haven't yet established a subspace for this entity path.
    fn find_subspace_rec_mut<'a>(
        subspaces: &'a mut IntMap<EntityPathHash, SubSpace>,
        subspace_origin: &EntityPath,
        path: &EntityPath,
    ) -> &'a mut SubSpace {
        debug_assert!(path.is_descendant_of(subspace_origin) || path == subspace_origin);

        let subspace = subspaces
            .get(&subspace_origin.hash())
            .expect("Subspace origin not part of origin->subspace map.");

        debug_assert!(&subspace.origin == subspace_origin);

        for child_space_origin in &subspace.child_spaces {
            if path == child_space_origin || path.is_descendant_of(child_space_origin) {
                // Clone to lift borrow on self.
                let child_space_origin = child_space_origin.clone();
                return Self::find_subspace_rec_mut(subspaces, &child_space_origin, path);
            }
        }

        // Need to query subspace again since otherwise we'd have a mutable borrow on self while trying to do a recursive call.
        return subspaces.get_mut(&subspace_origin.hash()).unwrap(); // Unwrap is safe since we succeeded ist just earlier.
    }

    /// Finds subspace an entity path belongs to by recursively walking down the hierarchy.
    fn find_subspace_rec(&self, subspace_origin: &EntityPath, path: &EntityPath) -> &SubSpace {
        let subspace = self
            .subspaces
            .get(&subspace_origin.hash())
            .expect("Subspace origin not part of origin->subspace map.");

        for child_space_origin in &subspace.child_spaces {
            if path == child_space_origin || path.is_descendant_of(child_space_origin) {
                return self.find_subspace_rec(child_space_origin, path);
            }
        }

        subspace
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::EntityPath;
    use re_types::{
        components::{DisconnectedSpace, PinholeProjection},
        ComponentName, Loggable as _,
    };

    use crate::spatial_topology::{SubSpaceConnectionFlags, SubSpaceDimensionality};

    use super::SpatialTopology;

    #[test]
    fn no_splits() {
        let mut topo = SpatialTopology::default();

        // Initialized with root space.
        assert_eq!(topo.subspaces.len(), 1);
        assert_eq!(topo.subspace_origin_per_entity.len(), 0);

        // Add a simple tree without any splits for now.
        add_diff(&mut topo, "robo", &[]);
        add_diff(&mut topo, "robo/arm", &[]);
        add_diff(&mut topo, "robo/eyes/cam", &[]);

        // Check that all entities are in the same space.
        check_paths_in_space(&topo, &["robo", "robo/arm", "robo/eyes/cam"], "/");

        // .. and that space has no children and no parent.
        let subspace = topo.subspace_for_entity(&"robo".into());
        assert_eq!(subspace.child_spaces.len(), 0);
        assert!(subspace.parent_space.is_none());

        // If we make up entities that weren't logged we get the closest space
        assert_eq!(
            topo.subspace_for_entity(&EntityPath::root()).origin,
            EntityPath::root()
        );
        assert_eq!(
            topo.subspace_for_entity(&"robo/eyes".into()).origin,
            EntityPath::root()
        );
    }

    #[test]
    fn valid_splits() {
        let mut topo = SpatialTopology::default();

        // Two cameras, one delayed for later.
        add_diff(&mut topo, "robo", &[]);
        add_diff(&mut topo, "robo/arm", &[]);
        add_diff(
            &mut topo,
            "robo/eyes/left/cam",
            &[PinholeProjection::name()],
        );
        add_diff(&mut topo, "robo/eyes/right/cam", &[]);
        add_diff(&mut topo, "robo/eyes/left/cam/annotation", &[]);
        add_diff(&mut topo, "robo/eyes/right/cam/annotation", &[]);
        {
            check_paths_in_space(
                &topo,
                &[
                    "robo",
                    "robo/arm",
                    "robo/eyes/right/cam",
                    "robo/eyes/right/cam/annotation",
                ],
                "/",
            );
            check_paths_in_space(
                &topo,
                &["robo/eyes/left/cam", "robo/eyes/left/cam/annotation"],
                "robo/eyes/left/cam",
            );
            let root_space = topo.subspace_for_entity(&"robo".into());
            let left_camera_space = topo.subspace_for_entity(&"robo/eyes/left/cam".into());
            assert_eq!(left_camera_space.origin, "robo/eyes/left/cam".into());
            assert_eq!(
                left_camera_space.parent_space,
                Some((root_space.origin.hash(), SubSpaceConnectionFlags::Pinhole))
            );
            assert_eq!(
                left_camera_space.dimensionality,
                SubSpaceDimensionality::TwoD
            );
            assert_eq!(root_space.dimensionality, SubSpaceDimensionality::ThreeD);
            assert!(root_space.parent_space.is_none());
        }

        // Introduce a third space at the right camera.
        add_diff(
            &mut topo,
            "robo/eyes/right/cam",
            &[PinholeProjection::name()],
        );
        {
            check_paths_in_space(&topo, &["robo", "robo/arm"], "/");
            check_paths_in_space(
                &topo,
                &["robo/eyes/right/cam", "robo/eyes/right/cam/annotation"],
                "robo/eyes/right/cam",
            );
            let root_space = topo.subspace_for_entity(&"robo".into());
            let right_camera_space = topo.subspace_for_entity(&"robo/eyes/right/cam".into());
            assert_eq!(right_camera_space.origin, "robo/eyes/right/cam".into());
            assert_eq!(
                right_camera_space.parent_space,
                Some((root_space.origin.hash(), SubSpaceConnectionFlags::Pinhole))
            );
            assert_eq!(
                right_camera_space.dimensionality,
                SubSpaceDimensionality::TwoD
            );

            // If we make up entities that weren't logged we get the closest space
            assert_eq!(
                topo.subspace_for_entity(&"robo/eyes/right/cam/unheard".into())
                    .origin,
                "robo/eyes/right/cam".into()
            );
            assert_eq!(
                topo.subspace_for_entity(&"bonkers".into()).origin,
                EntityPath::root()
            );
        }

        // Disconnect the left camera.
        add_diff(
            &mut topo,
            "robo/eyes/left/cam",
            &[DisconnectedSpace::name()],
        );
        {
            let root_space = topo.subspace_for_entity(&"robo".into());
            let left_camera_space = topo.subspace_for_entity(&"robo/eyes/left/cam".into());
            assert_eq!(left_camera_space.origin, "robo/eyes/left/cam".into());
            assert_eq!(
                left_camera_space.parent_space,
                Some((
                    root_space.origin.hash(),
                    SubSpaceConnectionFlags::Disconnected | SubSpaceConnectionFlags::Pinhole
                ))
            );
            assert_eq!(
                left_camera_space.dimensionality,
                SubSpaceDimensionality::TwoD
            );
            assert_eq!(root_space.dimensionality, SubSpaceDimensionality::ThreeD);
            assert!(root_space.parent_space.is_none());
        }
    }

    #[test]
    fn handle_invalid_splits_gracefully() {
        let mut topo = SpatialTopology::default();

        // Two nested cameras.
        add_diff(&mut topo, "cam0", &[PinholeProjection::name()]);
        add_diff(&mut topo, "cam0/cam1", &[PinholeProjection::name()]);

        check_paths_in_space(&topo, &["cam0"], "cam0");
        check_paths_in_space(&topo, &["cam0/cam1"], "cam0/cam1");

        let cam0 = topo.subspace_for_entity(&"cam0".into());
        let cam1 = topo.subspace_for_entity(&"cam0/cam1".into());

        assert_eq!(cam0.dimensionality, SubSpaceDimensionality::TwoD);
        assert_eq!(cam1.dimensionality, SubSpaceDimensionality::TwoD);
        assert_eq!(
            cam0.parent_space,
            Some((EntityPath::root().hash(), SubSpaceConnectionFlags::Pinhole))
        );
        assert_eq!(
            cam1.parent_space,
            Some((cam0.origin.hash(), SubSpaceConnectionFlags::Pinhole))
        );
    }

    #[test]
    fn disconnected_pinhole() {
        let mut topo = SpatialTopology::default();

        add_diff(&mut topo, "stuff", &[]);
        add_diff(
            &mut topo,
            "camera",
            &[PinholeProjection::name(), DisconnectedSpace::name()],
        );
        add_diff(&mut topo, "camera/image", &[]);

        check_paths_in_space(&topo, &["stuff"], "/");
        check_paths_in_space(&topo, &["camera", "camera/image"], "camera");

        let camera = topo.subspace_for_entity(&"camera".into());
        assert_eq!(camera.dimensionality, SubSpaceDimensionality::TwoD);
        assert_eq!(
            camera.parent_space,
            Some((
                EntityPath::root().hash(),
                SubSpaceConnectionFlags::Disconnected | SubSpaceConnectionFlags::Pinhole
            ))
        );

        let root_space = topo.subspace_for_entity(&"stuff".into());
        assert_eq!(root_space.dimensionality, SubSpaceDimensionality::ThreeD);
    }

    fn add_diff(topo: &mut SpatialTopology, path: &str, components: &[ComponentName]) {
        topo.on_store_diff(&path.into(), components.iter());
    }

    fn check_paths_in_space(topo: &SpatialTopology, paths: &[&str], expected_origin: &str) {
        for path in paths {
            let path = *path;
            assert_eq!(
                topo.subspace_for_entity(&path.into()).origin,
                expected_origin.into()
            );
        }

        let space = topo.subspace_for_entity(&paths[0].into());
        for path in paths {
            let path = *path;
            assert!(space.entities.contains(&path.into()));
        }
        assert_eq!(space.entities.len(), paths.len());
    }
}
