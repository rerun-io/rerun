use ahash::HashMap;
use lazy_static::lazy_static;
use re_log_types::MsgId;

pub type Legend<'s> = Option<&'s re_data_store::ClassDescriptionMap<'s>>;

lazy_static! {
    static ref MISSING_MSGID: MsgId = MsgId::random();
    static ref MISSING_LEGEND: re_data_store::ClassDescriptionMap<'static> = {
        re_data_store::ClassDescriptionMap {
            msg_id: &MISSING_MSGID,
            map: HashMap::<i32, re_data_store::ClassDescription<'static>>::default(),
        }
    };
}

// If the object_path is set on the image, but it doesn't point to a valid legend
// we return the default MissingLegend which gives us "reasonable" behavior.
// TODO(jleibs): We should still surface a user-visible error in this case
pub(crate) fn find_legend<'s>(
    obj_path: Option<&re_data_store::ObjPath>,
    objects: &'s re_data_store::Objects<'s>,
) -> Legend<'s> {
    objects
        .class_description_map
        .get(obj_path?)
        .or(Some(&MISSING_LEGEND))
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
pub(crate) trait ColorMapping {
    fn map_color(&self, val: u16) -> [u8; 4];
}

impl<'s> ColorMapping for re_data_store::ClassDescriptionMap<'s> {
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
pub(crate) trait LabelMapping {
    fn map_label(&self, val: u16) -> String;
}

impl<'s> LabelMapping for re_data_store::ClassDescriptionMap<'s> {
    fn map_label(&self, val: u16) -> String {
        if let Some(seg_label) = self.map.get(&(val as i32)) {
            if let Some(label) = seg_label.label {
                label.to_owned()
            } else {
                (val as i32).to_string()
            }
        } else {
            "unknown".to_owned()
        }
    }
}
