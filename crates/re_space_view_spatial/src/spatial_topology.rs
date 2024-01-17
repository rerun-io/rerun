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
    /// Note that this can both mean "there are both 2D and 3D relationships within the space"
    /// as well as "there are not spatial relationships at all within the space".
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

impl SubSpaceConnectionFlags {
    /// Pinhole flag but not disconnected
    #[inline]
    pub fn is_connected_pinhole(&self) -> bool {
        self.contains(SubSpaceConnectionFlags::Pinhole)
            && !self.contains(SubSpaceConnectionFlags::Disconnected)
    }
}

/// Spatial subspace within we typically expect a homogenous dimensionality without any projections & disconnects.
///
/// Subspaces are separated by projections or explicit disconnects.
///
/// A subspace may contain internal transforms, but any such transforms must be invertible such
/// that all data can be represented regardless of choice of origin.
///
/// Within the tree of all subspaces, every entity is contained in exactly one subspace.
/// The subtree at (and including) the `origin` minus the
/// subtrees of all child spaces are considered to be contained in a subspace.
pub struct SubSpace {
    /// The transform root of this subspace.
    ///
    /// This is also used to uniquely identify the space.
    pub origin: EntityPath,

    pub dimensionality: SubSpaceDimensionality,

    /// All entities that were logged at any point in time and are part of this subspace.
    ///
    /// Contains the origin entity as well, unless the origin is the `EntityPath::root()` and nothing was logged to it directly.
    ///
    /// Note that we this is merely here to speed up queries.
    /// Instead, we could check if an entity is equal to or a descendent of the
    /// origin and not equal or descendent of any child space.
    /// The problem with that is that it's common for a 3d space to have many 2d spaces as children,
    /// which would make this an expensive query.
    pub entities: IntSet<EntityPath>,

    /// Origin paths of child spaces and how they're connected.
    ///
    /// This implies that there is a either an explicit disconnect or
    /// a projection at the origin of the child space.
    /// How it is connected is implied by `parent_space` in the child space.
    ///
    /// Any path in `child_spaces` is *not* contained in `entities`.
    /// This implies that the camera itself is not part of its 3D space even when it may still have a 3D transform.
    pub child_spaces: IntMap<EntityPath, SubSpaceConnectionFlags>,

    /// Origin of the parent space if any.
    pub parent_space: Option<EntityPathHash>,
    //
    // TODO(andreas):
    // We could (and should) add here additional transform hierarchy information within this space.
    // This would be useful in order to speed up determining the transforms for a given frame.
}

impl SubSpace {
    /// Updates dimensionality based on a new connection to a child space.
    fn add_or_update_child_connection(
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
                    // TODO(#3849, #4301): We should be able to tag the source entity as having an invalid transform so we can display a permanent warning in the ui.
                    re_log::warn_once!("There was already a pinhole logged at {:?}.
The new pinhole at {:?} is nested under it, implying an invalid projection from a 2D space to a 2D space.", self.origin, child_path);
                    // We keep 2D.
                }
                SubSpaceDimensionality::ThreeD => {
                    // Already 3D.
                }
            }
        }

        self.child_spaces
            .entry(child_path.clone())
            .or_insert(new_connections_to_child) // insert into child spaces in the first place
            .insert(new_connections_to_child); // insert into connection flags
    }
}

#[derive(Default)]
pub struct SpatialTopologyStoreSubscriber {
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

/// Spatial toopological information about a store.
///
/// Describes how 2D & 3D spaces are connected/disconnected.
///
/// Used to determine whether 2D/3D visualizers are applicable and to inform
/// space view generation heuristics.
///
/// Spatial topology is time independent but may change as new data comes in.
/// Generally, the assumption is that topological cuts stay constant over time.
pub struct SpatialTopology {
    /// All subspaces, identified by their origin-hash.
    subspaces: IntMap<EntityPathHash, SubSpace>,

    /// Maps each logged entity to the origin of a subspace.
    ///
    /// This is purely an optimization to speed up searching for `subspaces`.
    subspace_origin_per_logged_entity: IntMap<EntityPathHash, EntityPathHash>,
}

impl Default for SpatialTopology {
    fn default() -> Self {
        Self {
            subspaces: std::iter::once((
                EntityPath::root().hash(),
                SubSpace {
                    origin: EntityPath::root(),
                    dimensionality: SubSpaceDimensionality::Unknown,
                    entities: IntSet::default(), // Note that this doesn't contain the root entity.
                    child_spaces: IntMap::default(),
                    parent_space: None,
                },
            ))
            .collect(),

            subspace_origin_per_logged_entity: Default::default(),
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
    #[inline]
    pub fn subspace_for_entity(&self, entity: &EntityPath) -> &SubSpace {
        self.subspaces
            .get(&self.subspace_origin_hash_for_entity(entity))
            .expect("unknown subspace origin, `SpatialTopology` is in an invalid state")
    }

    fn subspace_origin_hash_for_entity(&self, entity: &EntityPath) -> EntityPathHash {
        let mut entity_reference = entity;
        let mut entity_storage: EntityPath; // Only needed if we actually have to walk up the tree. Unused on the happy path.

        loop {
            // It's enough to check in`self.subspace_origin_per_logged_entity`, we don't have to check `self.subspaces`
            // since every origin of a subspace is also a logged entity (except the root which we checked initially),
            // making the keys of `self.subspace_origin_per_logged_entity` a superset of the keys of `self.subspaces`.
            if let Some(origin_hash) = self
                .subspace_origin_per_logged_entity
                .get(&entity_reference.hash())
            {
                return *origin_hash;
            }

            if let Some(parent) = entity_reference.parent() {
                entity_storage = parent;
                entity_reference = &entity_storage;
            } else {
                return EntityPath::root().hash();
            };
        }
    }

    /// Returns the subspace for a given origin.
    ///
    /// None if the origin doesn't identify its own subspace.
    #[inline]
    pub fn subspace_for_subspace_origin(&self, origin: EntityPathHash) -> Option<&SubSpace> {
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

        // Do we already know about this entity in general?
        if let Some(subspace_origin_hash) = self
            .subspace_origin_per_logged_entity
            .get(&entity_path.hash())
        {
            // In that case, this causes only changes if there's a change in connection.
            if !new_subspace_connections.is_empty() {
                self.update_space_with_new_connections(
                    entity_path,
                    *subspace_origin_hash,
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
        subspace_origin_hash: EntityPathHash,
        new_connections: SubSpaceConnectionFlags,
    ) {
        if subspace_origin_hash == entity_path.hash() {
            // If this is the origin of a space we can't split it.
            // Instead we have to update connectivity & dimensionality.
            let subspace = self
                .subspaces
                .get_mut(&subspace_origin_hash)
                .expect("Subspace origin not part of origin->subspace map.");

            // (see also `split_subspace`)`
            if new_connections.contains(SubSpaceConnectionFlags::Pinhole) {
                subspace.dimensionality = SubSpaceDimensionality::TwoD;
            }

            if let Some(parent_origin_hash) = subspace.parent_space {
                self.subspaces
                    .get_mut(&parent_origin_hash)
                    .expect("Parent origin not part of origin->subspace map.")
                    .add_or_update_child_connection(entity_path, new_connections);
            }
        } else {
            // Split the existing subspace.
            self.split_subspace(subspace_origin_hash, entity_path, new_connections);
        }
    }

    /// Adds a new entity to the spatial topology that wasn't known before.
    fn add_new_entity(
        &mut self,
        entity_path: &EntityPath,
        subspace_connections: SubSpaceConnectionFlags,
    ) {
        let subspace_origin_hash = self.subspace_origin_hash_for_entity(entity_path);

        let target_space_origin_hash = if subspace_connections.is_empty() {
            // Add entity to the existing space.
            let subspace = self
                .subspaces
                .get_mut(&subspace_origin_hash)
                .expect("Subspace origin not part of origin->subspace map.");
            subspace.entities.insert(entity_path.clone());
            subspace.origin.hash()
        } else {
            // Create a new subspace with this entity as its origin & containing this entity.
            self.split_subspace(subspace_origin_hash, entity_path, subspace_connections);
            entity_path.hash()
        };

        self.subspace_origin_per_logged_entity
            .insert(entity_path.hash(), target_space_origin_hash);
    }

    fn split_subspace(
        &mut self,
        split_subspace_origin_hash: EntityPathHash,
        new_space_origin: &EntityPath,
        connections: SubSpaceConnectionFlags,
    ) {
        let split_subspace = self
            .subspaces
            .get_mut(&split_subspace_origin_hash)
            .expect("Subspace origin not part of origin->subspace map.");
        debug_assert!(new_space_origin.is_descendant_of(&split_subspace.origin));

        // Determine the space dimensionality of the new space and update the current space's space type if necessary.
        // (see also `update_space_with_new_connections`)
        let space_dimensionality = if connections.contains(SubSpaceConnectionFlags::Pinhole) {
            SubSpaceDimensionality::TwoD
        } else {
            SubSpaceDimensionality::Unknown
        };

        let mut new_space = SubSpace {
            origin: new_space_origin.clone(),
            dimensionality: space_dimensionality,
            entities: std::iter::once(new_space_origin.clone()).collect(),
            child_spaces: Default::default(),
            parent_space: Some(split_subspace_origin_hash),
        };

        // Transfer entities from self to the new space if they're children of the new space.
        split_subspace.entities.retain(|e| {
            if e.is_descendant_of(new_space_origin) || e == new_space_origin {
                self.subspace_origin_per_logged_entity
                    .insert(e.hash(), new_space.origin.hash());
                new_space.entities.insert(e.clone());
                false
            } else {
                true
            }
        });

        // Transfer any child spaces from self to the new space if they're children of the new space.
        split_subspace
            .child_spaces
            .retain(|child_origin, connections| {
                debug_assert!(child_origin != new_space_origin);

                if child_origin.is_descendant_of(new_space_origin) {
                    new_space
                        .child_spaces
                        .insert(child_origin.clone(), *connections);
                    false
                } else {
                    true
                }
            });

        // Note that the new connection information may change the known dimensionality of the space that we're splitting.
        split_subspace.add_or_update_child_connection(new_space_origin, connections);

        // Patch parents of the child spaces that were moved to the new space.
        for child_origin in new_space.child_spaces.keys() {
            let child_space = self
                .subspaces
                .get_mut(&child_origin.hash())
                .expect("Child origin not part of origin->subspace map.");
            child_space.parent_space = Some(new_space.origin.hash());
        }

        self.subspaces.insert(new_space.origin.hash(), new_space);
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
        assert_eq!(topo.subspace_origin_per_logged_entity.len(), 0);

        // Add a simple tree without any splits for now.
        add_diff(&mut topo, "robo", &[]);
        add_diff(&mut topo, "robo/arm", &[]);
        add_diff(&mut topo, "robo/eyes/cam", &[]);

        // Check that all entities are in the same space.
        check_paths_in_space(&topo, &["robo", "robo/arm", "robo/eyes/cam"], "/");

        // .. and that space has no children and no parent.
        let subspace = topo.subspace_for_entity(&"robo".into());
        assert!(subspace.child_spaces.is_empty());
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
        add_diff(&mut topo, "robo/eyes/left/cam/annotation", &[]);
        add_diff(&mut topo, "robo/arm", &[]);
        add_diff(
            &mut topo,
            "robo/eyes/left/cam",
            &[PinholeProjection::name()],
        );
        add_diff(&mut topo, "robo/eyes/right/cam/annotation", &[]);
        add_diff(&mut topo, "robo/eyes/right/cam", &[]);
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

            let root = topo.subspace_for_entity(&"robo".into());
            let left_camera = topo.subspace_for_entity(&"robo/eyes/left/cam".into());

            assert_eq!(left_camera.origin, "robo/eyes/left/cam".into());
            assert_eq!(left_camera.parent_space, Some(root.origin.hash()));
            assert_eq!(left_camera.dimensionality, SubSpaceDimensionality::TwoD);

            assert_eq!(root.dimensionality, SubSpaceDimensionality::ThreeD);
            assert_eq!(
                root.child_spaces,
                std::iter::once((left_camera.origin.clone(), SubSpaceConnectionFlags::Pinhole))
                    .collect()
            );
            assert!(root.parent_space.is_none());
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

            let root = topo.subspace_for_entity(&"robo".into());
            let left_camera = topo.subspace_for_entity(&"robo/eyes/left/cam".into());
            let right_camera = topo.subspace_for_entity(&"robo/eyes/right/cam".into());

            assert_eq!(right_camera.origin, "robo/eyes/right/cam".into());
            assert_eq!(right_camera.parent_space, Some(root.origin.hash()));
            assert_eq!(right_camera.dimensionality, SubSpaceDimensionality::TwoD);
            assert_eq!(
                root.child_spaces,
                [
                    (left_camera.origin.clone(), SubSpaceConnectionFlags::Pinhole),
                    (
                        right_camera.origin.clone(),
                        SubSpaceConnectionFlags::Pinhole
                    )
                ]
                .into_iter()
                .collect()
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
            let root = topo.subspace_for_entity(&"robo".into());
            let left_camera = topo.subspace_for_entity(&"robo/eyes/left/cam".into());
            let right_camera = topo.subspace_for_entity(&"robo/eyes/right/cam".into());

            assert_eq!(left_camera.origin, "robo/eyes/left/cam".into());
            assert_eq!(left_camera.parent_space, Some(root.origin.hash()));
            assert_eq!(left_camera.dimensionality, SubSpaceDimensionality::TwoD);
            assert_eq!(root.dimensionality, SubSpaceDimensionality::ThreeD);
            assert!(root.parent_space.is_none());
            assert_eq!(
                root.child_spaces,
                [
                    (
                        left_camera.origin.clone(),
                        SubSpaceConnectionFlags::Disconnected | SubSpaceConnectionFlags::Pinhole
                    ),
                    (
                        right_camera.origin.clone(),
                        SubSpaceConnectionFlags::Pinhole
                    )
                ]
                .into_iter()
                .collect()
            );
        }
    }

    #[test]
    fn handle_invalid_splits_gracefully() {
        for nested_first in [false, true] {
            let mut topo = SpatialTopology::default();

            // Two nested cameras. Try both orderings
            if nested_first {
                add_diff(&mut topo, "cam0/cam1", &[PinholeProjection::name()]);
                add_diff(&mut topo, "cam0", &[PinholeProjection::name()]);
            } else {
                add_diff(&mut topo, "cam0", &[PinholeProjection::name()]);
                add_diff(&mut topo, "cam0/cam1", &[PinholeProjection::name()]);
            }

            check_paths_in_space(&topo, &["cam0"], "cam0");
            check_paths_in_space(&topo, &["cam0/cam1"], "cam0/cam1");

            let root = topo.subspace_for_entity(&EntityPath::root());
            let cam0 = topo.subspace_for_entity(&"cam0".into());
            let cam1 = topo.subspace_for_entity(&"cam0/cam1".into());

            assert_eq!(cam0.dimensionality, SubSpaceDimensionality::TwoD);
            assert_eq!(cam1.dimensionality, SubSpaceDimensionality::TwoD);
            assert_eq!(cam0.parent_space, Some(EntityPath::root().hash()));
            assert_eq!(cam1.parent_space, Some(cam0.origin.hash()));

            assert_eq!(
                root.child_spaces,
                std::iter::once((cam0.origin.clone(), SubSpaceConnectionFlags::Pinhole)).collect()
            );

            assert_eq!(
                cam0.child_spaces,
                std::iter::once((cam1.origin.clone(), SubSpaceConnectionFlags::Pinhole)).collect()
            );
            assert!(cam1.child_spaces.is_empty());
        }
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

        let cam = topo.subspace_for_entity(&"camera".into());
        assert_eq!(cam.dimensionality, SubSpaceDimensionality::TwoD);
        assert_eq!(cam.parent_space, Some(EntityPath::root().hash()));

        let root = topo.subspace_for_entity(&"stuff".into());
        assert_eq!(root.dimensionality, SubSpaceDimensionality::ThreeD);
        assert_eq!(
            root.child_spaces,
            std::iter::once((
                cam.origin.clone(),
                SubSpaceConnectionFlags::Disconnected | SubSpaceConnectionFlags::Pinhole
            ))
            .collect()
        );
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
