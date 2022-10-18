pub type Legend<'s> = Option<&'s re_data_store::SegmentationMap<'s>>;

pub(crate) fn find_legend<'s>(
    obj_path: Option<&re_data_store::ObjPath>,
    objects: &'s re_data_store::Objects<'s>,
) -> Legend<'s> {
    objects.segmentation_map.get(obj_path?)
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

impl<'s> ColorMapping for re_data_store::SegmentationMap<'s> {
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
            [0, 0, 0, 0]
        }
    }
}

// TODO(jleibs): sort out lifetime of label
pub(crate) trait LabelMapping {
    fn map_label(&self, val: u16) -> String;
}

impl<'s> LabelMapping for re_data_store::SegmentationMap<'s> {
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
