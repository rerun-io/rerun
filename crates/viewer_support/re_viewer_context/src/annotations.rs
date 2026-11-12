use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use ahash::HashMap;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::{
    ChunkDirectLineageReport, ChunkStore, ChunkStoreDiff, ChunkStoreEvent,
    ChunkStoreSubscriberHandle, LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityPath;
use re_log_types::StoreId;
use re_sdk_types::archetypes;
use re_sdk_types::components::{AnnotationContext, Color, Text};
use re_sdk_types::datatypes::{AnnotationInfo, ClassDescription, ClassId, KeypointId};
use re_types_core::{Component as _, ComponentDescriptor, ComponentIdentifier};

use super::auto_color_egui;

const MISSING_ROW_ID: RowId = RowId::ZERO;

#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct Annotations {
    row_id: RowId,
    class_map: HashMap<ClassId, CachedClassDescription>,
}

/// The annotation-context value to materialize for a component slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationContextTargetKind {
    Color,
    Label,
}

/// A component slot whose values can be resolved from annotation context.
#[derive(Clone, Debug)]
pub struct AnnotationContextTarget {
    descriptor: ComponentDescriptor,
    kind: AnnotationContextTargetKind,
}

impl AnnotationContextTarget {
    pub fn color(descriptor: ComponentDescriptor) -> Self {
        re_log::debug_assert_eq!(descriptor.component_type, Some(Color::name()));
        Self {
            descriptor,
            kind: AnnotationContextTargetKind::Color,
        }
    }

    pub fn label(descriptor: ComponentDescriptor) -> Self {
        re_log::debug_assert_eq!(descriptor.component_type, Some(Text::name()));
        Self {
            descriptor,
            kind: AnnotationContextTargetKind::Label,
        }
    }

    pub fn descriptor(&self) -> &ComponentDescriptor {
        &self.descriptor
    }

    pub fn kind(&self) -> AnnotationContextTargetKind {
        self.kind
    }
}

/// Declares how a visualizer wants annotation context applied to resolved query results.
#[derive(Clone, Debug)]
pub struct AnnotationContextQuery {
    pub class_ids: ComponentIdentifier,
    pub keypoint_ids: Option<ComponentIdentifier>,
    pub targets: Vec<AnnotationContextTarget>,
}

impl AnnotationContextQuery {
    pub fn new(
        class_ids: ComponentIdentifier,
        targets: impl IntoIterator<Item = AnnotationContextTarget>,
    ) -> Self {
        Self {
            class_ids,
            keypoint_ids: None,
            targets: targets.into_iter().collect(),
        }
    }

    /// Uses keypoint-specific annotation values when keypoint IDs are active.
    #[must_use]
    pub fn with_keypoint_ids(mut self, keypoint_ids: ComponentIdentifier) -> Self {
        self.keypoint_ids = Some(keypoint_ids);
        self
    }
}

impl Annotations {
    #[inline]
    pub fn missing() -> Self {
        Self {
            row_id: MISSING_ROW_ID,
            class_map: Default::default(),
        }
    }

    /// Shared empty annotation context for callers that need default values.
    pub fn missing_ref() -> &'static Self {
        static CELL: std::sync::LazyLock<Annotations> =
            std::sync::LazyLock::new(Annotations::missing);
        &CELL
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

    /// The [`RowId`] of the annotation context that was used to create this [`Annotations`].
    ///
    /// This can be used as a cache key to determine if the annotation context has changed.
    #[inline]
    pub fn row_id(&self) -> RowId {
        self.row_id
    }
}

#[derive(Clone, Debug, re_byte_size::SizeBytes)]
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
        keypoint_id: re_sdk_types::encodings::KeypointId,
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

#[derive(Clone, Default, re_byte_size::SizeBytes)]
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
}

// ----------------------------------------------------------------------------

#[derive(Default, Clone, Debug, re_byte_size::SizeBytes)]
pub struct AnnotationMap(pub BTreeMap<EntityPath, Arc<Annotations>>);

impl AnnotationMap {
    /// For each passed [`EntityPath`], walk up the tree and find the nearest ancestor
    ///
    /// An entity is considered its own (nearest) ancestor.
    pub fn load(&mut self, db: &re_entity_db::EntityDb, time_query: &LatestAtQuery) {
        re_tracing::profile_function!();

        // Load current annotations.
        for entity in AnnotationContextStoreSubscriber::entities(db.store_id()) {
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

    /// Finds the nearest annotation context for an entity path.
    pub fn find(&self, entity_path: &EntityPath) -> Option<&Annotations> {
        let mut next_parent = Some(entity_path.clone());
        while let Some(parent) = next_parent {
            if let Some(legend) = self.0.get(&parent) {
                return Some(legend);
            }

            next_parent = parent.parent();
        }

        None
    }
}

/// Keeps track of all entities that have an annotation context.
#[derive(Default)]
pub struct AnnotationContextStoreSubscriber {
    /// An entity can have several chunks holding an annotation context at once, e.g. temporal ones
    /// logged at different times, or a static one alongside the newer one overwriting it.
    /// It keeps its annotation context until the last of them is removed.
    chunks_per_entity: IntMap<EntityPath, BTreeSet<ChunkId>>,
}

impl AnnotationContextStoreSubscriber {
    /// The entities that have an annotation context at some point in time.
    pub fn entities(store_id: &StoreId) -> Vec<EntityPath> {
        ChunkStore::with_per_store_subscriber_once(
            Self::subscription_handle(),
            store_id,
            |subscriber: &Self| subscriber.chunks_per_entity.keys().cloned().collect(),
        )
        .unwrap_or_default()
    }

    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    fn add_chunk(&mut self, chunk: &Chunk) {
        if chunk
            .components()
            .contains_key(&archetypes::AnnotationContext::descriptor_context().component)
        {
            self.chunks_per_entity
                .entry(chunk.entity_path().clone())
                .or_default()
                .insert(chunk.id());
        }
    }

    fn remove_chunk(&mut self, chunk: &Chunk) {
        if let Some(chunk_ids) = self.chunks_per_entity.get_mut(chunk.entity_path()) {
            chunk_ids.remove(&chunk.id());
            if chunk_ids.is_empty() {
                self.chunks_per_entity.remove(chunk.entity_path());
            }
        }
    }
}

impl re_byte_size::MemUsageTreeCapture for AnnotationContextStoreSubscriber {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        use re_byte_size::SizeBytes as _;
        re_byte_size::MemUsageTree::Bytes(self.chunks_per_entity.total_size_bytes())
    }
}

impl PerStoreChunkSubscriber for AnnotationContextStoreSubscriber {
    #[inline]
    fn name() -> String {
        "AnnotationContextStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        for event in events {
            match &event.diff {
                ChunkStoreDiff::Addition(addition) => {
                    // Compaction removes the chunks it merges without firing deletions for them.
                    if let ChunkDirectLineageReport::CompactedFrom(sources) =
                        &addition.direct_lineage
                    {
                        for source in sources.values() {
                            self.remove_chunk(source);
                        }
                    }
                    self.add_chunk(&addition.chunk_after_processing);
                }
                ChunkStoreDiff::Deletion(deletion) => self.remove_chunk(&deletion.chunk),
                ChunkStoreDiff::VirtualAddition(_) | ChunkStoreDiff::SchemaAddition(_) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use re_entity_db::EntityDb;
    use re_log_types::{StoreKind, TimePoint, TimelineName};
    use re_sdk_types::encodings::Rgba32;

    use super::*;

    #[test]
    fn overwritten_static_annotation_context_resolves_to_the_newest() {
        // Per-store subscribers only see the chunks added after their registration.
        AnnotationContextStoreSubscriber::subscription_handle();

        let mut db = EntityDb::new(StoreId::random(StoreKind::Recording, "test_app"));
        let entity_path = EntityPath::from("segmentation");
        let newest_color = Rgba32::from_rgb(0, 0, 255);
        for color in [Rgba32::from_rgb(255, 0, 0), newest_color] {
            let chunk = Chunk::builder(entity_path.clone())
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &archetypes::AnnotationContext::new([(1, "class", color)]),
                )
                .build()
                .unwrap();
            db.add_chunk(&Arc::new(chunk)).unwrap();
        }

        let mut annotation_map = AnnotationMap::default();
        annotation_map.load(&db, &LatestAtQuery::latest(TimelineName::log_tick()));

        let color = annotation_map.find(&entity_path).map(|annotations| {
            annotations
                .resolved_class_description(Some(1.into()))
                .annotation_info()
                .color()
        });
        assert_eq!(color, Some(Some(newest_color.into())));
    }
}
