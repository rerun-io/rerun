use lazy_static::lazy_static;
use nohash_hasher::IntSet;
use re_arrow_store::LatestAtQuery;
use re_data_store::{FieldName, ObjPath, TimeQuery};
use re_log_types::{
    context::{AnnotationInfo, ClassDescription},
    field_types::{ClassId, KeypointId},
    msg_bundle::Component,
    AnnotationContext, DataPath, MsgId,
};
use re_query::query_entity_with_primary;
use std::{collections::BTreeMap, sync::Arc};

use crate::{misc::ViewerContext, ui::scene::SceneQuery};

#[derive(Clone, Debug)]
pub struct Annotations {
    pub msg_id: MsgId,
    pub context: AnnotationContext,
}

impl Annotations {
    pub fn class_description(&self, class_id: Option<ClassId>) -> ResolvedClassDescription<'_> {
        ResolvedClassDescription(
            class_id.and_then(|class_id| self.context.class_map.get(&class_id)),
        )
    }
}

pub struct ResolvedClassDescription<'a>(pub Option<&'a ClassDescription>);

impl<'a> ResolvedClassDescription<'a> {
    pub fn annotation_info(&self) -> ResolvedAnnotationInfo {
        ResolvedAnnotationInfo(self.0.map(|desc| desc.info.clone()))
    }

    /// Merges class annotation info with keypoint annotation info (if existing respectively).
    pub fn annotation_info_with_keypoint(&self, keypoint_id: KeypointId) -> ResolvedAnnotationInfo {
        if let Some(desc) = self.0 {
            // Assuming that keypoint annotation is the rarer case, merging the entire annotation ahead of time
            // is cheaper than doing it lazily (which would cause more branches down the line for callsites without keypoints)
            if let Some(keypoint_annotation_info) = desc.keypoint_map.get(&keypoint_id) {
                ResolvedAnnotationInfo(Some(AnnotationInfo {
                    id: keypoint_id.0,
                    label: keypoint_annotation_info
                        .label
                        .clone()
                        .or_else(|| desc.info.label.clone()),
                    color: keypoint_annotation_info.color.or(desc.info.color),
                }))
            } else {
                self.annotation_info()
            }
        } else {
            ResolvedAnnotationInfo(None)
        }
    }
}

#[derive(Clone, Copy)]
pub enum DefaultColor<'a> {
    OpaqueWhite,
    TransparentBlack,
    ObjPath(&'a ObjPath),
}

pub struct ResolvedAnnotationInfo(pub Option<AnnotationInfo>);

impl ResolvedAnnotationInfo {
    pub fn color(
        &self,
        color: Option<&[u8; 4]>,
        default_color: DefaultColor<'_>,
    ) -> re_renderer::Color32 {
        if let Some([r, g, b, a]) = color {
            re_renderer::Color32::from_rgba_premultiplied(*r, *g, *b, *a)
        } else if let Some(color) = self.0.as_ref().and_then(|info| {
            info.color
                .map(|c| c.into())
                .or_else(|| Some(auto_color(info.id)))
        }) {
            color
        } else {
            match default_color {
                DefaultColor::TransparentBlack => re_renderer::Color32::TRANSPARENT,
                DefaultColor::OpaqueWhite => re_renderer::Color32::WHITE,
                DefaultColor::ObjPath(obj_path) => {
                    auto_color((obj_path.hash64() % std::u16::MAX as u64) as u16)
                }
            }
        }
    }

    pub fn label(&self, label: Option<&String>) -> Option<String> {
        if let Some(label) = label {
            Some(label.clone())
        } else {
            self.0
                .as_ref()
                .and_then(|info| info.label.as_ref().map(|label| label.0.clone()))
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct AnnotationMap(pub BTreeMap<ObjPath, Arc<Annotations>>);

impl AnnotationMap {
    fn load_classic(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (obj_path, field_store) in
            query.iter_ancestor_meta_field(ctx.log_db, &FieldName::from("_annotation_context"))
        {
            if let Ok(mono_field_store) = field_store.get_mono::<re_log_types::AnnotationContext>()
            {
                let time_query = TimeQuery::LatestAt(query.latest_at.as_i64());
                mono_field_store.query(&time_query, |_time, msg_id, context| {
                    self.0.entry(obj_path.clone()).or_insert_with(|| {
                        Arc::new(Annotations {
                            msg_id: *msg_id,
                            context: context.clone(),
                        })
                    });
                });
            }
        }
    }

    /// For each `ObjPath` in the `SceneQuery`, walk up the tree and find the nearest ancestor
    ///
    /// An object is considered its own (nearest) ancestor.
    fn load_arrow(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let mut visited = IntSet::<ObjPath>::default();

        let arrow_store = &ctx.log_db.obj_db.arrow_store;
        let arrow_query = LatestAtQuery::new(query.timeline, query.latest_at);

        // This logic is borrowed from `iter_ancestor_meta_field`, but using the arrow-store instead
        // not made generic as `AnnotationContext` was the only user of that function
        for obj_path in query
            .obj_paths
            .iter()
            .filter(|obj_path| query.obj_props.get(obj_path).visible)
        {
            let mut next_parent = Some(obj_path.clone());
            while let Some(parent) = next_parent {
                // If we've visited this parent before it's safe to break early.
                // All of it's parents have have also been visited.
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
                        if query_entity_with_primary::<AnnotationContext>(
                            arrow_store,
                            &arrow_query,
                            &parent,
                            &[MsgId::name()],
                        )
                        .ok()
                        .and_then(|entity| {
                            if let (Some(context), Some(msg_id)) = (
                                entity.iter_primary().ok()?.next()?,
                                entity.iter_component::<MsgId>().ok()?.next()?,
                            ) {
                                Some(entry.insert(Arc::new(Annotations { msg_id, context })))
                            } else {
                                None
                            }
                        })
                        .is_some()
                        {
                            break;
                        }
                    }
                }
                // Finally recurse to the next parent up the path
                // TODO(jleibs): this is somewhat expensive as it needs to re-hash the object path
                // given ObjPathImpl is already an Arc, consider pre-computing and storing parents
                // for faster iteration.
                next_parent = parent.parent();
            }
        }
    }

    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();
        self.load_classic(ctx, query);
        self.load_arrow(ctx, query);
    }

    pub(crate) fn find_associated(
        ctx: &mut ViewerContext<'_>,
        mut obj_path: ObjPath,
    ) -> Option<(DataPath, Annotations)> {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let timeline_store = ctx.log_db.obj_db.store.get(timeline)?;
        let query_time = ctx.rec_cfg.time_ctrl.time_i64()?;
        let field_name = FieldName::from("_annotation_context");

        let annotation_context_for_path = |obj_path: &ObjPath| {
            let field_store = timeline_store.get(obj_path)?.get(&field_name)?;
            // `_annotation_context` is only allowed to be stored in a mono-field.
            let mono_field_store = field_store
                .get_mono::<re_log_types::AnnotationContext>()
                .ok()?;
            let (_, msg_id, context) = mono_field_store.latest_at(&query_time)?;
            Some((
                DataPath::new(obj_path.clone(), field_name),
                Annotations {
                    msg_id: *msg_id,
                    context: context.clone(),
                },
            ))
        };

        loop {
            obj_path = obj_path.parent()?;
            let annotations = annotation_context_for_path(&obj_path);
            if annotations.is_some() {
                return annotations;
            }
        }
    }

    // Search through the all prefixes of this object path until we find a
    // matching annotation. If we find nothing return the default `MISSING_ANNOTATIONS`.
    pub fn find<'a>(&self, obj_path: impl Into<&'a ObjPath>) -> Arc<Annotations> {
        let mut next_parent = Some(obj_path.into().clone());
        while let Some(parent) = next_parent {
            if let Some(legend) = self.0.get(&parent) {
                return legend.clone();
            }

            next_parent = parent.parent().clone();
        }

        // Otherwise return the missing legend
        Arc::clone(&MISSING_ANNOTATIONS)
    }
}

// ---

const MISSING_MSG_ID: MsgId = MsgId::ZERO;

lazy_static! {
    pub static ref MISSING_ANNOTATIONS: Arc<Annotations> = {
        Arc::new(Annotations {
            msg_id: MISSING_MSG_ID,
            context: Default::default(),
        })
    };
}

// default colors
// Borrowed from `egui::PlotUi`
pub fn auto_color(val: u16) -> re_renderer::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.85, 0.5, 1.0))
}
