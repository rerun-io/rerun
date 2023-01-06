use lazy_static::lazy_static;
use re_data_store::{FieldName, ObjPath, TimeQuery};
use re_log_types::{
    context::{AnnotationInfo, ClassDescription},
    field_types::{ClassId, KeypointId},
    AnnotationContext, DataPath, MsgId,
};
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
    pub fn color(&self, color: Option<&[u8; 4]>, default_color: DefaultColor<'_>) -> [u8; 4] {
        if let Some(color) = color {
            *color
        } else if let Some(color) = self.0.as_ref().and_then(|info| {
            info.color
                .map(|c| c.to_array())
                .or_else(|| Some(auto_color(info.id)))
        }) {
            color
        } else {
            match default_color {
                DefaultColor::TransparentBlack => [0, 0, 0, 0],
                DefaultColor::OpaqueWhite => [255, 255, 255, 255],
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
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
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
    static ref MISSING_ANNOTATIONS: Arc<Annotations> = {
        Arc::new(Annotations {
            msg_id: MISSING_MSG_ID,
            context: Default::default(),
        })
    };
}

// default colors
// Borrowed from `egui::PlotUi`
pub fn auto_color_egui(val: u16) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.85, 0.5, 1.0))
}

pub fn auto_color(val: u16) -> [u8; 4] {
    let color = auto_color_egui(val);
    color.to_array()
}
