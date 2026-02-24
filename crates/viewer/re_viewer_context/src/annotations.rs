use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use ahash::HashMap;
use nohash_hasher::IntSet;
use re_chunk::RowId;
use re_chunk_store::{
    ChunkStore, ChunkStoreEvent, ChunkStoreSubscriberHandle, LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityPath;
use re_log_types::StoreId;
use re_sdk_types::archetypes;
use re_sdk_types::components::AnnotationContext;
use re_sdk_types::datatypes::{AnnotationInfo, ClassDescription, ClassId, KeypointId, Utf8};

use super::auto_color_egui;

const MISSING_ROW_ID: RowId = RowId::ZERO;

#[derive(Clone, Debug)]
pub struct Annotations {
    row_id: RowId,
    class_map: HashMap<ClassId, CachedClassDescription>,
}

impl Annotations {
    #[inline]
    pub fn missing() -> Self {
        Self {
            row_id: MISSING_ROW_ID,
            class_map: Default::default(),
        }
    }

    /// Fast access to an [`Arc`] sharing the same [`Annotations::missing`] instance.
    pub fn missing_arc() -> Arc<Self> {
        use std::sync::OnceLock;
        static CELL: OnceLock<Arc<Annotations>> = OnceLock::new();
        CELL.get_or_init(|| Arc::new(Self::missing())).clone()
    }

    #[inline]
    pub fn resolved_class_description(
        &self,
        class_id: Option<re_sdk_types::components::ClassId>,
    ) -> ResolvedClassDescription<'_> {
        let found = class_id.and_then(|class_id| self.class_map.get(&class_id.0));
        ResolvedClassDescription {
            class_id: class_id.map(|id| id.0),
            class_description: found.map(|f| &f.class_description),
            keypoint_map: found.map(|f| &f.keypoint_map),
        }
    }

    #[inline]
    pub fn row_id(&self) -> RowId {
        self.row_id
    }
}

#[derive(Clone, Debug)]
struct CachedClassDescription {
    class_description: ClassDescription,
    keypoint_map: HashMap<KeypointId, AnnotationInfo>,
}

impl From<ClassDescription> for CachedClassDescription {
    fn from(desc: ClassDescription) -> Self {
        let keypoint_map = desc
            .keypoint_annotations
            .iter()
            .map(|kp| (kp.id.into(), kp.clone()))
            .collect();
        Self {
            class_description: desc,
            keypoint_map,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedClassDescription<'a> {
    pub class_id: Option<ClassId>,
    pub class_description: Option<&'a ClassDescription>,
    pub keypoint_map: Option<&'a HashMap<KeypointId, AnnotationInfo>>,
}

impl ResolvedClassDescription<'_> {
    #[inline]
    pub fn annotation_info(&self) -> ResolvedAnnotationInfo {
        ResolvedAnnotationInfo {
            class_id: self.class_id,
            annotation_info: self.class_description.map(|desc| desc.info.clone()),
        }
    }

    /// Merges class annotation info with keypoint annotation info (if existing respectively).
    pub fn annotation_info_with_keypoint(
        &self,
        keypoint_id: re_sdk_types::datatypes::KeypointId,
    ) -> ResolvedAnnotationInfo {
        if let (Some(desc), Some(keypoint_map)) = (self.class_description, self.keypoint_map) {
            // Assuming that keypoint annotation is the rarer case, merging the entire annotation ahead of time
            // is cheaper than doing it lazily (which would cause more branches down the line for callsites without keypoints)
            if let Some(keypoint_annotation_info) = keypoint_map.get(&keypoint_id) {
                ResolvedAnnotationInfo {
                    class_id: self.class_id,
                    annotation_info: Some(AnnotationInfo {
                        id: keypoint_id.0,
                        label: keypoint_annotation_info
                            .label
                            .clone()
                            .or_else(|| desc.info.label.clone()),
                        color: keypoint_annotation_info.color.or(desc.info.color),
                    }),
                }
            } else {
                self.annotation_info()
            }
        } else {
            ResolvedAnnotationInfo {
                class_id: self.class_id,
                annotation_info: None,
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct ResolvedAnnotationInfo {
    pub class_id: Option<ClassId>,
    pub annotation_info: Option<AnnotationInfo>,
}

impl ResolvedAnnotationInfo {
    pub fn color(&self) -> Option<egui::Color32> {
        #![expect(clippy::manual_map)] // for readability

        if let Some(info) = &self.annotation_info {
            // Use annotation context based color.
            if let Some(color) = info.color {
                Some(color.into())
            } else {
                Some(auto_color_egui(info.id))
            }
        } else if let Some(class_id) = self.class_id {
            // Use class id based color (or give up).
            Some(auto_color_egui(class_id.0))
        } else {
            None
        }
    }

    #[inline]
    pub fn label(&self, label: Option<&str>) -> Option<String> {
        if let Some(label) = label {
            Some(label.to_owned())
        } else {
            self.annotation_info
                .as_ref()?
                .label
                .as_ref()
                .map(|label| label.to_string())
        }
    }

    #[inline]
    pub fn label_utf8(&self, label: Option<Utf8>) -> Option<Utf8> {
        if let Some(label) = label {
            Some(label)
        } else {
            self.annotation_info.as_ref()?.label.clone()
        }
    }
}

// ----------------------------------------------------------------------------

/// Many [`ResolvedAnnotationInfo`], with optimization
/// for a common case where they are all the same.
pub enum ResolvedAnnotationInfos {
    /// All the same
    Same(usize, ResolvedAnnotationInfo),

    /// All different
    Many(Vec<ResolvedAnnotationInfo>),
}

impl ResolvedAnnotationInfos {
    pub fn iter(&self) -> impl Iterator<Item = &ResolvedAnnotationInfo> {
        use itertools::Either;
        match self {
            Self::Same(n, info) => Either::Left(std::iter::repeat_n(info, *n)),
            Self::Many(infos) => Either::Right(infos.iter()),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Same(n, _) => *n,
            Self::Many(infos) => infos.len(),
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Same(n, _) => *n == 0,
            Self::Many(infos) => infos.is_empty(),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Default, Clone, Debug)]
pub struct AnnotationMap(pub BTreeMap<EntityPath, Arc<Annotations>>);

impl AnnotationMap {
    /// For each passed [`EntityPath`], walk up the tree and find the nearest ancestor
    ///
    /// An entity is considered its own (nearest) ancestor.
    pub fn load(&mut self, db: &re_entity_db::EntityDb, time_query: &LatestAtQuery) {
        re_tracing::profile_function!();

        let entities_with_annotation_context =
            AnnotationContextStoreSubscriber::access(db.store_id(), |entities| entities.clone())
                .unwrap_or_default();

        // Load current annotations.
        // (order doesn't matter, we're feeding into another hashmap)
        #[expect(clippy::iter_over_hash_type)]
        for entity in entities_with_annotation_context {
            if let Some(((_time, row_id), ann_ctx)) = db.latest_at_component::<AnnotationContext>(
                &entity,
                time_query,
                archetypes::AnnotationContext::descriptor_context().component,
            ) {
                let annotations = Annotations {
                    row_id,
                    class_map: ann_ctx
                        .0
                        .into_iter()
                        .map(|elem| {
                            (
                                elem.class_id,
                                CachedClassDescription::from(elem.class_description),
                            )
                        })
                        .collect(),
                };
                self.0.insert(entity, Arc::new(annotations));
            }
        }
    }

    // Search through the all prefixes of this entity path until we find a
    // matching annotation. If we find nothing return the default [`Annotations::missing_arc`].
    pub fn find(&self, entity_path: &EntityPath) -> Arc<Annotations> {
        let mut next_parent = Some(entity_path.clone());
        while let Some(parent) = next_parent {
            if let Some(legend) = self.0.get(&parent) {
                return legend.clone();
            }

            next_parent = parent.parent();
        }

        // Otherwise return the missing legend
        Annotations::missing_arc()
    }
}

/// Keeps track of all entities that have an annotation context.
#[derive(Default)]
pub struct AnnotationContextStoreSubscriber {
    pub entities_with_annotation_context: IntSet<EntityPath>,
}

impl AnnotationContextStoreSubscriber {
    /// Accesses the list of entities that have an annotation context at some point in time.
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&IntSet<EntityPath>) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber_once(
            Self::subscription_handle(),
            store_id,
            move |subscriber: &Self| f(&subscriber.entities_with_annotation_context),
        )
    }

    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }
}

impl PerStoreChunkSubscriber for AnnotationContextStoreSubscriber {
    #[inline]
    fn name() -> String {
        "AnnotationContextStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        for event in events {
            let Some(delta_chunk) = event.delta_chunk() else {
                continue;
            };

            if delta_chunk
                .components()
                .contains_key(&archetypes::AnnotationContext::descriptor_context().component)
            {
                let path = delta_chunk.entity_path();
                if event.is_addition() {
                    self.entities_with_annotation_context.insert(path.clone());
                } else if event.is_deletion() {
                    // Deletions do *not* account for chunks that were compacted away, and therefore this
                    // will correctly mirror the number of additions above (including splits).
                    self.entities_with_annotation_context.remove(path);
                }
            }
        }
    }
}
