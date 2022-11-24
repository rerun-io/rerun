use std::{collections::BTreeMap, sync::Arc};

use lazy_static::lazy_static;
use re_data_store::{FieldName, ObjPath};
use re_log_types::{
    context::{AnnotationInfo, ClassDescription, ClassId, KeypointId},
    AnnotationContext, MsgId,
};

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

pub struct ResolvedClassDescription<'a>(Option<&'a ClassDescription>);

impl<'a> ResolvedClassDescription<'a> {
    pub fn annotation_info(&self) -> ResolvedAnnotationInfo {
        ResolvedAnnotationInfo(self.0.map(|desc| (desc.info.clone(), desc.id.0)))
    }

    /// Merges class annotation info with keypoint annotation info (if existing respectively).
    pub fn annotation_info_with_keypoint(
        &self,
        keypoint_id: &KeypointId,
    ) -> ResolvedAnnotationInfo {
        if let Some(desc) = self.0 {
            // Assuming that keypoint annotation is the rarer case, merging the entire annotation ahead of time
            // is cheaper than doing it lazily (which would cause more branches down the line for everyone)
            if let Some(keypoint_annotation_info) = desc.keypoint_map.get(keypoint_id) {
                ResolvedAnnotationInfo(Some((
                    AnnotationInfo {
                        label: keypoint_annotation_info
                            .label
                            .clone()
                            .or_else(|| desc.info.label.clone()),
                        color: keypoint_annotation_info.color.or(desc.info.color),
                    },
                    keypoint_id.0,
                )))
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

pub struct ResolvedAnnotationInfo(Option<(AnnotationInfo, u16)>);

impl ResolvedAnnotationInfo {
    pub fn color(&self, color: Option<&[u8; 4]>, default_color: DefaultColor<'_>) -> [u8; 4] {
        if let Some(color) = color {
            *color
        } else if let Some(color) = self
            .0
            .as_ref()
            .and_then(|info| info.0.color.or_else(|| Some(auto_color(info.1))))
        {
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
                .and_then(|info| info.0.label.as_ref().map(ToString::to_string))
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
                mono_field_store.query(&query.time_query, |_time, msg_id, context| {
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
pub fn auto_color(val: u16) -> [u8; 4] {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    let color = egui::Color32::from(egui::color::Hsva::new(h, 0.85, 0.5, 1.0));
    color.to_array()
}
