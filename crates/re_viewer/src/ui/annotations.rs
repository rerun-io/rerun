use std::{collections::BTreeMap, sync::Arc};

use lazy_static::lazy_static;
use re_data_store::{FieldName, ObjPath};
use re_log_types::{context::ClassId, AnnotationContext, Data, DataPath, MsgId};

use crate::{misc::ViewerContext, ui::scene::SceneQuery};

#[derive(Clone, Debug)]
pub struct Annotations {
    pub msg_id: MsgId,
    pub context: AnnotationContext,
}

#[derive(Clone, Copy)]
pub enum DefaultColor<'a> {
    OpaqueWhite,
    TransparentBlack,
    ObjPath(&'a ObjPath),
}

impl Annotations {
    pub fn color(
        &self,
        color: Option<&[u8; 4]>,
        class_id: Option<ClassId>,
        default_color: DefaultColor<'_>,
    ) -> [u8; 4] {
        if let Some(color) = color {
            *color
        } else if let Some(color) = class_id.and_then(|id| {
            self.context
                .class_map
                .get(&id)
                // If have a valid id, use it for color even if the context doesn't have one.
                .map(|desc| desc.info.color.unwrap_or_else(|| auto_color(id.0)))
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

    pub fn label(&self, label: Option<&String>, class_id: Option<ClassId>) -> Option<String> {
        if let Some(label) = label {
            Some(label.clone())
        } else {
            class_id.and_then(|id| {
                self.context
                    .class_map
                    .get(&id)
                    .and_then(|desc| desc.info.label.as_ref().map(ToString::to_string))
            })
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

    pub(crate) fn find_associated(
        ctx: &mut ViewerContext<'_>,
        obj_path: &ObjPath,
    ) -> Option<(DataPath, Annotations)> {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let store = ctx.log_db.obj_db.store.get(timeline)?;
        let time_query = ctx.rec_cfg.time_ctrl.time_query()?;

        let mut path = obj_path.clone();
        loop {
            let Some(parent) = path.parent() else {
                    break None;
                };
            path = parent;
            let Some(store) = store.get(&path) else {
                    continue;
                };
            let annotation_context = store.iter().find_map(|(field_name, field_store)| {
                match field_store.query_field_to_datavec(&time_query, None) {
                    Ok((meta, data_vec)) => {
                        if let Some(data) = data_vec.last() {
                            if field_name.as_str() == "_annotation_context" {
                                if let Data::AnnotationContext(context) = data {
                                    return Some((
                                        DataPath::new(path.clone(), *field_name),
                                        Annotations {
                                            msg_id: meta.last().unwrap().1,
                                            context,
                                        },
                                    ));
                                }
                            }
                        }
                        None
                    }
                    Err(_) => None,
                }
            });
            if annotation_context.is_some() {
                break annotation_context;
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
