use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use nohash_hasher::IntSet;

use re_data_store2::LatestAtQuery;
use re_entity_db::EntityPath;
use re_log_types::RowId;
use re_query::LatestAtMonoResult;
use re_types::components::AnnotationContext;
use re_types::datatypes::{AnnotationInfo, ClassDescription, ClassId, KeypointId, Utf8};

use super::{auto_color, ViewerContext};
use crate::DefaultColor;

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
        class_id: Option<re_types::components::ClassId>,
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

impl<'a> ResolvedClassDescription<'a> {
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
        keypoint_id: re_types::datatypes::KeypointId,
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
    /// `rgba` should be unmultiplied
    #[inline]
    pub fn color(
        &self,
        rgba: Option<[u8; 4]>,
        default_color: DefaultColor<'_>,
    ) -> re_renderer::Color32 {
        if let Some([r, g, b, a]) = rgba {
            if a == 255 {
                // Common-case optimization
                re_renderer::Color32::from_rgb(r, g, b)
            } else {
                re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
            }
        } else if let Some(color) = self.annotation_info.as_ref().and_then(|info| {
            info.color
                .map(|c| c.into())
                .or_else(|| Some(auto_color(info.id)))
        }) {
            color
        } else {
            match (self.class_id, default_color) {
                (Some(class_id), _) if class_id.0 != 0 => auto_color(class_id.0),
                (_, DefaultColor::TransparentBlack) => re_renderer::Color32::TRANSPARENT,
                (_, DefaultColor::OpaqueWhite) => re_renderer::Color32::WHITE,
                (_, DefaultColor::EntityPath(entity_path)) => {
                    auto_color((entity_path.hash64() % std::u16::MAX as u64) as u16)
                }
            }
        }
    }

    #[inline]
    pub fn label(&self, label: Option<&str>) -> Option<String> {
        if let Some(label) = label {
            Some(label.to_owned())
        } else {
            self.annotation_info
                .as_ref()
                .and_then(|info| info.label.as_ref().map(|label| label.to_string()))
        }
    }

    #[inline]
    pub fn label_utf8(&self, label: Option<Utf8>) -> Option<Utf8> {
        if let Some(label) = label {
            Some(label)
        } else {
            self.annotation_info
                .as_ref()
                .and_then(|info| info.label.as_ref().cloned())
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
            Self::Same(n, info) => Either::Left(std::iter::repeat(info).take(*n)),
            Self::Many(infos) => Either::Right(infos.iter()),
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
    pub fn load<'a>(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_query: &LatestAtQuery,
        entities: impl Iterator<Item = &'a EntityPath>,
    ) {
        re_tracing::profile_function!();

        let mut visited = IntSet::<EntityPath>::default();

        // This logic is borrowed from `iter_ancestor_meta_field`, but using the arrow-store instead
        // not made generic as `AnnotationContext` was the only user of that function
        for ent_path in entities {
            let mut next_parent = Some(ent_path.clone());
            while let Some(parent) = next_parent {
                // If we've visited this parent before it's safe to break early.
                // All of it's parents have also been visited.
                if !visited.insert(parent.clone()) {
                    break;
                }

                match self.0.entry(parent.clone()) {
                    // If we've hit this path before and found a match, we can also break.
                    // This should not actually get hit due to the above early-exit.
                    std::collections::btree_map::Entry::Occupied(_) => break,
                    // Otherwise check the obj_store for the field.
                    // If we find one, insert it and then we can break.
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        if let Some(LatestAtMonoResult {
                            index: (_, row_id),
                            value: ann_ctx,
                        }) = ctx
                            .recording()
                            .latest_at_component::<AnnotationContext>(&parent, time_query)
                        {
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
                            entry.insert(Arc::new(annotations));
                        }
                    }
                }
                // Finally recurse to the next parent up the path
                // TODO(jleibs): this is somewhat expensive as it needs to re-hash the entity path.
                next_parent = parent.parent();
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
