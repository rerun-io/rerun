use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use lazy_static::lazy_static;
use re_data_store::ObjPath;
use re_log_types::MsgId;

// ---

#[derive(Clone, Debug)]
pub struct ClassDescription {
    pub label: Option<Cow<'static, str>>,
    pub color: Option<[u8; 4]>,
}

#[derive(Clone, Debug)]
pub struct ClassDescriptionMap {
    pub msg_id: MsgId,
    pub map: HashMap<i32, ClassDescription>,
}

#[derive(Default, Clone, Debug)]
pub struct Legends(pub BTreeMap<ObjPath, Arc<ClassDescriptionMap>>);

impl Legends {
    // If the object_path is set on the image, but it doesn't point to a valid legend
    // we return the default MissingLegend which gives us "reasonable" behavior.
    // TODO(jleibs): We should still surface a user-visible error in this case
    pub fn find<'a>(&self, obj_path: impl Into<Option<&'a ObjPath>>) -> Legend {
        let obj_path = obj_path.into();
        self.0
            .get(obj_path?)
            .cloned() // Arc
            .or_else(|| Some(Arc::clone(&MISSING_LEGEND)))
    }
}

pub type Legend = Option<Arc<ClassDescriptionMap>>;

// ---

lazy_static! {
    static ref MISSING_MSGID: MsgId = MsgId::random();
    static ref MISSING_LEGEND: Arc<ClassDescriptionMap> = {
        Arc::new(ClassDescriptionMap {
            msg_id: *MISSING_MSGID,
            map: Default::default(),
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

impl ColorMapping for ClassDescriptionMap {
    fn map_color(&self, val: u16) -> [u8; 4] {
        if let Some(seg_label) = self.map.get(&(val as i32)) {
            if let Some(color) = seg_label.color {
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

impl LabelMapping for ClassDescriptionMap {
    fn map_label(&self, val: u16) -> String {
        if let Some(seg_label) = self.map.get(&(val as i32)) {
            if let Some(label) = seg_label.label.as_ref() {
                label.to_string()
            } else {
                (val as i32).to_string()
            }
        } else {
            "unknown".to_owned()
        }
    }
}
