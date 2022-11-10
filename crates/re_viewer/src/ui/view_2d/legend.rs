use std::{collections::BTreeMap, sync::Arc};

use lazy_static::lazy_static;
use re_data_store::ObjPath;
use re_log_types::{AnnotationContext, MsgId};

#[derive(Clone, Debug)]
pub struct Annotations {
    pub msg_id: MsgId,
    pub context: AnnotationContext,
}

// TODO: rename Legend to something more annotation-specific?

#[derive(Default, Clone, Debug)]
pub struct Legends(pub BTreeMap<ObjPath, Arc<Annotations>>);

impl Legends {
    // If the object_path is set on the image, but it doesn't point to a valid legend
    // we return the default MissingLegend which gives us "reasonable" behavior.
    // TODO(jleibs): We should still surface a user-visible error in this case
    pub fn find<'a>(&self, obj_path: impl Into<Option<&'a ObjPath>>) -> Arc<Annotations> {
        let mut next_parent = obj_path.into().cloned();
        while let Some(parent) = next_parent {
            if let Some(legend) = self.0.get(&parent) {
                return legend.clone();
            }

            next_parent = parent.parent();
        }

        // Otherwise return the missing legend
        Arc::clone(&MISSING_LEGEND)
    }
}

pub type Legend = Option<Arc<Annotations>>;

// ---

lazy_static! {
    static ref MISSING_MSGID: MsgId = MsgId::random();
    static ref MISSING_LEGEND: Arc<Annotations> = {
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

// Currently using a pair [u8;4] since it converts more easily
// to DynamicImage
pub trait ColorMapping {
    fn map_color(&self, val: u16) -> [u8; 4];
}

impl ColorMapping for Annotations {
    fn map_color(&self, val: u16) -> [u8; 4] {
        if let Some(class_desc) = self.context.class_map.get(&val) {
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
}

// TODO(jleibs): sort out lifetime of label
pub trait LabelMapping {
    fn map_label(&self, val: u16) -> String;
}

impl LabelMapping for Annotations {
    fn map_label(&self, val: u16) -> String {
        if let Some(class_desc) = self.context.class_map.get(&val) {
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
