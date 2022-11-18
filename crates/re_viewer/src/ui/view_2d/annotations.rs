use std::{collections::BTreeMap, sync::Arc};

use lazy_static::lazy_static;
use re_data_store::ObjPath;
use re_log_types::{context::ClassId, AnnotationContext, MsgId};

#[derive(Clone, Debug)]
pub struct Annotations {
    pub msg_id: MsgId,
    pub context: AnnotationContext,
}

#[derive(Clone, Copy)]
pub enum DefaultColor {
    White,
    Random,
}

impl Annotations {
    pub fn color(
        &self,
        color: Option<&[u8; 4]>,
        class_id: Option<ClassId>,
        obj_path: &ObjPath,
        default_color: DefaultColor,
    ) -> [u8; 4] {
        if let Some(color) = color {
            return *color;
        }
        if let Some(class_id) = class_id {
            return self.color_from_class_id(class_id.0);
        }

        match default_color {
            DefaultColor::White => [255, 255, 255, 255],
            DefaultColor::Random => auto_color((obj_path.hash64() % std::u16::MAX as u64) as u16),
        }
    }

    pub fn color_from_class_id(&self, val: u16) -> [u8; 4] {
        if let Some(class_desc) = self.context.class_map.get(&ClassId(val)) {
            if let Some(color) = class_desc.info.color {
                color
            } else {
                auto_color(val)
            }
        } else {
            // TODO(jleibs) Unset labels default to transparent black
            // This gives us better behavior for the "0" id, though we
            // should be more explicit about this in the future.
            if val == 0 {
                [0, 0, 0, 0]
            } else {
                auto_color(val)
            }
        }
    }

    // TODO: Same for labels
}

#[derive(Default, Clone, Debug)]
pub struct AnnotationMap(pub BTreeMap<ObjPath, Arc<Annotations>>);

impl AnnotationMap {
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

lazy_static! {
    static ref MISSING_MSGID: MsgId = MsgId::random();
    static ref MISSING_ANNOTATIONS: Arc<Annotations> = {
        Arc::new(Annotations {
            msg_id: *MISSING_MSGID,
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

// TODO(jleibs): sort out lifetime of label
pub trait LabelMapping {
    fn map_label(&self, val: u16) -> String;
}

impl LabelMapping for Annotations {
    fn map_label(&self, val: u16) -> String {
        if let Some(class_desc) = self.context.class_map.get(&ClassId(val)) {
            if let Some(label) = class_desc.info.label.as_ref() {
                label.to_string()
            } else {
                (val as i32).to_string()
            }
        } else {
            "unknown".to_owned()
        }
    }
}
