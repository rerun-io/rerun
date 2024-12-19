use once_cell::sync::OnceCell;

use ahash::HashMap;
use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::{
    ChunkStore, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber,
    ChunkStoreSubscriberHandle,
};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{
    components::{PinholeProjection, ViewCoordinates},
    Component,
};

bitflags::bitflags! {
    #[derive(PartialEq, Eq, Debug, Copy, Clone)]
    pub struct SubSpaceConnectionFlags: u8 {
        const Pinhole = 0b0000001;
    }
}

bitflags::bitflags! {
    /// Marks entities that are of special interest for heuristics.
    #[derive(PartialEq, Eq, Debug, Copy, Clone)]
    pub struct HeuristicHints: u8 {
        const ViewCoordinates3d = 0b0000001;
    }
}

/// Spatial subspace within we typically expect a homogeneous dimensionality without any projections.
///
/// Subspaces are separated by projections.
///
/// A subspace may contain internal transforms, but any such transforms must be invertible such
/// that all data can be represented regardless of choice of origin.
///
/// Within the tree of all subspaces, every entity is contained in exactly one subspace.
/// The subtree at (and including) the `origin` minus the
/// subtrees of all child spaces are considered to be contained in a subspace.
#[derive(Debug)]
pub struct SubSpace {
    /// The transform root of this subspace.
    ///
    /// This is also used to uniquely identify the space.
    pub origin: EntityPath,

    /// All entities that were logged at any point in time and are part of this subspace.
    ///
    /// Contains the origin entity as well, unless the origin is the `EntityPath::root()` and nothing was logged to it directly.
    ///
    /// Note that we this is merely here to speed up queries.
    /// Instead, we could check if an entity is equal to or a descendent of the
    /// origin and not equal or descendent of any child space.
    /// The problem with that is that it's common for a 3D space to have many 2D spaces as children,
    /// which would make this an expensive query.
    pub entities: IntSet<EntityPath>,

    /// Origin paths of child spaces.
    ///
    /// This implies that there is a projection at the origin of the child space.
    /// How it is connected is implied by `connection_to_parent` in the child space.
    ///
    /// Any path in `child_spaces` is *not* contained in `entities`.
    /// This implies that the camera itself is not part of its 3D space even when it may still have a 3D transform.
    pub child_spaces: IntSet<EntityPath>,

    /// Origin of the parent space.
    ///
    /// The root space has `EntityPathHash::NONE` as parent.
    pub parent_space: EntityPathHash,

    /// The connection to the parent space.
    ///
    /// Note that since flags are derived from the presence of components at the origin,
    /// the root space still tracks this information.
    pub connection_to_parent: SubSpaceConnectionFlags,

    /// Entities in this space that qualify for one or more heuristic hints.
    pub heuristic_hints: IntMap<EntityPath, HeuristicHints>,
    //
    // TODO(andreas):
    // We could (and should) add here additional transform hierarchy information within this space.
    // This would be useful in order to speed up determining the transforms for a given frame.
}

impl SubSpace {
    /// Whether 3D content in this subspace can be displayed.
    #[inline]
    pub fn supports_3d_content(&self) -> bool {
        // Note that we currently do *not* walk up the tree of spaces to check for pinholes.
        // Pro:
        // * on a disconnect everything should be possible again, so why would that not be the case at every cut?
        // * being overly restrictive means we won't display 3D content when we could.
        //    * for the same reason we also don't want to preclude 3D content when encountering 2D view coordinates, albeit this may still inform heuristics
        // Con:
        // * if at any point (without a disconnect) we encountered a pinhole prior, everything below should be considered 2D
        !self
            .connection_to_parent
            .contains(SubSpaceConnectionFlags::Pinhole)
    }

    /// Whether 2D content in this subspace can be displayed.
    #[inline]
    #[allow(clippy::unused_self)]
    pub fn supports_2d_content(&self) -> bool {
        // There's currently no way to prevent a subspace from displaying 2D content.
        true
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
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| ChunkStore::register_subscriber(Box::<Self>::default()))
    }
}

impl ChunkStoreSubscriber for SpatialTopologyStoreSubscriber {
    #[inline]
    fn name(&self) -> String {
        "SpatialTopologyStoreSubscriber".to_owned()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            if event.diff.kind != ChunkStoreDiffKind::Addition {
                // Topology is only additive, don't care about removals.
                continue;
            }

            // Possible optimization:
            // only update topologies if an entity is logged the first time or a new relevant component was added.
            self.topologies
                .entry(event.store_id.clone())
                .or_default()
                .on_store_diff(
                    event.diff.chunk.entity_path(),
                    event.diff.chunk.component_names(),
                );
        }
    }
}

/// Spatial topological information about a store.
///
/// Describes how 2D & 3D spaces are connected/disconnected.
///
/// Used to determine whether 2D/3D visualizers are applicable and to inform
/// view generation heuristics.
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
                    entities: IntSet::default(), // Note that this doesn't contain the root entity.
                    child_spaces: IntSet::default(),
                    parent_space: EntityPathHash::NONE,
                    connection_to_parent: SubSpaceConnectionFlags::empty(),
                    heuristic_hints: IntMap::default(),
                },
            ))
            .collect(),

            subspace_origin_per_logged_entity: Default::default(),
        }
    }
}

impl SpatialTopology {
    /// Accesses the spatial topology for a given store.
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&Self) -> T) -> Option<T> {
        ChunkStore::with_subscriber_once(
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

    /// Iterates over all subspaces.
    #[inline]
    pub fn iter_subspaces(&self) -> impl Iterator<Item = &SubSpace> {
        self.subspaces.values()
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

    fn on_store_diff(
        &mut self,
        entity_path: &EntityPath,
        added_components: impl Iterator<Item = re_types::ComponentName>,
    ) {
        re_tracing::profile_function!();

        let mut new_subspace_connections = SubSpaceConnectionFlags::empty();
        let mut new_heuristic_hints = HeuristicHints::empty();

        for added_component in added_components {
            if added_component == PinholeProjection::name() {
                new_subspace_connections.insert(SubSpaceConnectionFlags::Pinhole);
            } else if added_component == ViewCoordinates::name() {
                new_heuristic_hints.insert(HeuristicHints::ViewCoordinates3d);
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

        if !new_heuristic_hints.is_empty() {
            let subspace = self
                .subspaces
                .get_mut(&self.subspace_origin_hash_for_entity(entity_path))
                .expect("unknown subspace origin, `SpatialTopology` is in an invalid state");
            subspace
                .heuristic_hints
                .entry(entity_path.clone())
                .or_insert(HeuristicHints::empty())
                .insert(new_heuristic_hints);
        }
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
            subspace.connection_to_parent.insert(new_connections);
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

        let target_space_origin_hash =
            if subspace_connections.is_empty() || entity_path.hash() == subspace_origin_hash {
                // Add entity to the existing space.
                let subspace = self
                    .subspaces
                    .get_mut(&subspace_origin_hash)
                    .expect("Subspace origin not part of origin->subspace map.");
                subspace.entities.insert(entity_path.clone());
                subspace.connection_to_parent.insert(subspace_connections);
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
        connection_to_parent: SubSpaceConnectionFlags,
    ) {
        let split_subspace = self
            .subspaces
            .get_mut(&split_subspace_origin_hash)
            .expect("Subspace origin not part of origin->subspace map.");
        debug_assert!(new_space_origin.is_descendant_of(&split_subspace.origin));

        let mut new_space = SubSpace {
            origin: new_space_origin.clone(),
            entities: std::iter::once(new_space_origin.clone()).collect(),
            child_spaces: Default::default(),
            parent_space: split_subspace_origin_hash,
            connection_to_parent,
            heuristic_hints: Default::default(),
        };

        // Transfer entities from self to the new space if they're children of the new space.
        split_subspace.entities.retain(|e| {
            if e.starts_with(new_space_origin) {
                self.subspace_origin_per_logged_entity
                    .insert(e.hash(), new_space.origin.hash());
                new_space.entities.insert(e.clone());
                false
            } else {
                true
            }
        });

        // Transfer any child spaces from self to the new space if they're children of the new space.
        split_subspace.child_spaces.retain(|child_origin| {
            debug_assert!(child_origin != new_space_origin);

            if child_origin.is_descendant_of(new_space_origin) {
                new_space.child_spaces.insert(child_origin.clone());
                false
            } else {
                true
            }
        });

        split_subspace.child_spaces.insert(new_space_origin.clone());

        // Patch parents of the child spaces that were moved to the new space.
        for child_origin in &new_space.child_spaces {
            let child_space = self
                .subspaces
                .get_mut(&child_origin.hash())
                .expect("Child origin not part of origin->subspace map.");
            child_space.parent_space = new_space.origin.hash();
        }

        self.subspaces.insert(new_space.origin.hash(), new_space);
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::EntityPath;
    use re_types::{
        components::{PinholeProjection, ViewCoordinates},
        Component as _, ComponentName,
    };

    use crate::spatial_topology::{HeuristicHints, SubSpaceConnectionFlags};

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
        assert_eq!(
            topo.subspace_for_entity(&"robo/leg".into()).origin,
            EntityPath::root()
        );

        // Add splitting entities to the root space - this should not cause any splits.
        #[allow(clippy::single_element_loop)]
        for (name, flags) in [
            (PinholeProjection::name(), SubSpaceConnectionFlags::Pinhole),
            // Add future ways of splitting here (in the past `DisconnectedSpace` was used here).
        ] {
            add_diff(&mut topo, "", &[name]);
            let subspace = topo.subspace_for_entity(&"robo".into());
            assert_eq!(subspace.connection_to_parent, flags);
            assert!(subspace.child_spaces.is_empty());
            assert!(subspace.parent_space.is_none());
        }
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
            assert_eq!(left_camera.parent_space, root.origin.hash());
            assert_eq!(
                left_camera.connection_to_parent,
                SubSpaceConnectionFlags::Pinhole
            );

            assert_eq!(root.connection_to_parent, SubSpaceConnectionFlags::empty());
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
            assert_eq!(right_camera.parent_space, root.origin.hash());
            assert_eq!(
                right_camera.connection_to_parent,
                SubSpaceConnectionFlags::Pinhole
            );
            assert_eq!(left_camera.origin, "robo/eyes/left/cam".into());
            assert_eq!(left_camera.parent_space, root.origin.hash());
            assert_eq!(
                left_camera.connection_to_parent,
                SubSpaceConnectionFlags::Pinhole
            );

            assert_eq!(root.connection_to_parent, SubSpaceConnectionFlags::empty());
            assert!(root.parent_space.is_none());

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

        // Add view coordinates to robo.
        add_diff(&mut topo, "robo", &[ViewCoordinates::name()]);
        {
            let root = topo.subspace_for_entity(&EntityPath::root());

            assert!(root.parent_space.is_none());
            assert_eq!(root.connection_to_parent, SubSpaceConnectionFlags::empty());
            assert_eq!(
                root.heuristic_hints,
                std::iter::once((EntityPath::from("robo"), HeuristicHints::ViewCoordinates3d))
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

            assert_eq!(root.connection_to_parent, SubSpaceConnectionFlags::empty());
            assert_eq!(cam0.connection_to_parent, SubSpaceConnectionFlags::Pinhole);
            assert_eq!(cam1.connection_to_parent, SubSpaceConnectionFlags::Pinhole);
            assert_eq!(cam0.parent_space, EntityPath::root().hash());
            assert_eq!(cam1.parent_space, cam0.origin.hash());
            assert!(cam1.child_spaces.is_empty());
        }
    }

    fn add_diff(topo: &mut SpatialTopology, path: &str, components: &[ComponentName]) {
        topo.on_store_diff(&path.into(), components.iter().copied());
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
