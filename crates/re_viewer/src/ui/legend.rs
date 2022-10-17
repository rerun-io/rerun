pub enum Legend<'s> {
    None,
    SegmentationMap(&'s re_data_store::SegmentationMap<'s>),
}

// TODO(jleibs): is there a more idiomatic way of doing this?
pub(crate) fn find_legend<'s>(
    obj_path: Option<&re_data_store::ObjPath>,
    objects: &'s re_data_store::Objects<'s>,
) -> Legend<'s> {
    if let Some(obj_path) = obj_path {
        if let Some(seg_map) = objects.segmentation_map.get(obj_path) {
            Legend::SegmentationMap(seg_map)
        } else {
            Legend::None
        }
    } else {
        Legend::None
    }
}

impl<'s> Legend<'s> {
    pub fn get_msgid(&self) -> Option<re_log_types::MsgId> {
        match &self {
            Legend::None => None,
            Legend::SegmentationMap(seg_map) => Some(*seg_map.msg_id),
        }
    }
}

// default colors
// Borrowed from `egui::PlotUi`
pub fn auto_color(val: u8) -> [u8; 4] {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    let color: egui::Color32 = egui::color::Hsva::new(h, 0.85, 0.5, 1.0).into();
    color.to_array()
}

// TODO(jleibs) should this use egui::Color type
// Currently using a pair [u8;4] since it converts more easily
// to DynamicImage
pub(crate) trait ColorMapping {
    fn map_color(&self, val: u8) -> [u8; 4];
}

impl<'s> ColorMapping for Legend<'s> {
    fn map_color(&self, val: u8) -> [u8; 4] {
        match &self {
            Legend::None => [val, val, val, 255],
            Legend::SegmentationMap(map) => {
                if let Some(seg_label) = map.map.get(&(val as i32)) {
                    if let Some(color) = seg_label.color {
                        color
                    } else {
                        auto_color(val)
                    }
                } else {
                    // Should we use a special color for unset labels?
                    [0, 0, 0, 0]
                }
            }
        }
    }
}

// TODO(jleibs): sort out lifetime of label
pub(crate) trait LabelMapping {
    fn map_label(&self, val: u8) -> String;
}

impl<'s> LabelMapping for Legend<'s> {
    fn map_label(&self, val: u8) -> String {
        match &self {
            Legend::None => "".to_owned(),
            Legend::SegmentationMap(map) => {
                if let Some(seg_label) = map.map.get(&(val as i32)) {
                    if let Some(label) = seg_label.label {
                        label.to_owned()
                    } else {
                        seg_label.id.to_string()
                    }
                } else {
                    "unknown".to_owned()
                }
            }
        }
    }
}
