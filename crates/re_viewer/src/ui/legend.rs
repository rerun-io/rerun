use egui::Color32;
use nohash_hasher::IntMap;

pub(crate) enum Legend<'s> {
    None,
    SegmentationMap(SegmentationMapS<'s>),
}

// TODO: is there a more idiomatic way of doing this?
pub(crate) fn find_legend<'s>(
    obj_path: Option<&re_data_store::ObjPath>,
    objects: &'s re_data_store::Objects<'s>,
) -> Legend<'s> {
    if let Some(obj_path) = obj_path {
        if let Some(seg_map) = objects.segmentation_maps.get(obj_path) {
            Legend::SegmentationMap(SegmentationMapS::<'s> { map: seg_map })
        } else {
            Legend::None
        }
    } else {
        Legend::None
    }
}

pub(crate) trait ColorMapping {
    fn map_func(&self) -> Box<dyn Fn(u8) -> Color32 + '_>;
}

pub(crate) struct SegmentationMapS<'s> {
    map: &'s IntMap<i32, re_data_store::SegmentationLabel<'s>>,
}

impl<'s> SegmentationMapS<'s> {
    fn apply(&self, val: u8) -> Color32 {
        let color = if let Some(seg_label) = self.map.get(&(val as i32)) {
            if let Some(color) = seg_label.color {
                color
            } else {
                // TODO: Better color for set label with unset color
                [0, 0, 0, 0]
            }
        } else {
            // TODO: Better color for non-defined label
            [0, 0, 0, 0]
        };
        Color32::from_rgb(color[0], color[1], color[2])
    }
}

impl<'s> ColorMapping for SegmentationMapS<'s> {
    fn map_func(&self) -> Box<dyn Fn(u8) -> Color32 + '_> {
        Box::new(|t| self.apply(t))
    }
}
