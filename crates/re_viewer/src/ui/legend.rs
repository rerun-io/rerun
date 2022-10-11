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
    ///
    fn map_func<T>(&self) -> Box<dyn Fn(T) -> Color32 + '_>;
}

pub(crate) struct SegmentationMapS<'s> {
    map: &'s IntMap<i32, re_data_store::SegmentationLabel<'s>>,
}

impl<'s> SegmentationMapS<'s> {
    fn apply<T>(&self, _val: T) -> Color32 {
        Color32::from_rgb(0, 255, 0)
    }
}

impl<'s> ColorMapping for SegmentationMapS<'s> {
    fn map_func<T>(&self) -> Box<dyn Fn(T) -> Color32 + '_> {
        Box::new(|t: T| self.apply(t))
    }
}
