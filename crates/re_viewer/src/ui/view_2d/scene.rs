use std::sync::Arc;

use egui::{pos2, Color32, Pos2, Rect, Stroke};
use re_data_store::{
    query::{visit_type_data_2, visit_type_data_3, visit_type_data_4},
    FieldName, InstanceId, InstanceIdHash, ObjPath, ObjectTreeProperties,
};
use re_log_types::{DataVec, IndexHash, MsgId, ObjectType, Tensor};

use crate::{ui::SceneQuery, ViewerContext};

use super::{ClassDescription, ClassDescriptionMap, Legend, Legends, TwoDViewState};

// ---

// TODO(cmc): just turn all colors into Color32?

pub struct Image {
    pub msg_id: MsgId,
    pub instance_hash: InstanceIdHash,

    pub tensor: Tensor,
    /// If this is a depth map, how long is a meter?
    ///
    /// For instance, with a `u16` dtype one might have
    /// `meter == 1000.0` for millimeter precision
    /// up to a ~65m range.
    pub meter: Option<f32>,
    pub paint_props: ObjectPaintProperties,
    /// A thing that provides additional semantic context for your dtype.
    pub legend: Legend,
}

pub struct Box2D {
    pub instance_hash: InstanceIdHash,

    pub bbox: re_log_types::BBox2D,
    pub stroke_width: Option<f32>,
    pub label: Option<String>,
    pub paint_props: ObjectPaintProperties,
}

pub struct LineSegments2D {
    pub instance_hash: InstanceIdHash,

    /// Connected pair-wise even-odd.
    pub points: Vec<Pos2>,
    pub stroke_width: Option<f32>,
    pub paint_props: ObjectPaintProperties,
}

pub struct Point2D {
    pub instance_hash: InstanceIdHash,

    pub pos: Pos2,
    pub radius: Option<f32>,
    pub paint_props: ObjectPaintProperties,
}

/// A 2D scene, with everything needed to render it.
pub struct Scene2D {
    /// Estimated bounding box of all data. Accumulated.
    pub bbox: Rect,
    pub legends: Legends,

    pub images: Vec<Image>,
    pub boxes: Vec<Box2D>,
    pub line_segments: Vec<LineSegments2D>,
    pub points: Vec<Point2D>,
}

impl Default for Scene2D {
    fn default() -> Self {
        Self {
            bbox: Rect::NOTHING,
            legends: Default::default(),
            images: Default::default(),
            boxes: Default::default(),
            line_segments: Default::default(),
            points: Default::default(),
        }
    }
}

// TODO: what do we do with the whole IndexHash situation?

impl Scene2D {
    pub(crate) fn load(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        state: &TwoDViewState, // TODO: messy
        query: &SceneQuery<'_>,
    ) {
        puffin::profile_function!();

        // TODO: that is most definitely an issue.. no? maybe not
        // We introduce a 1-frame delay!
        let hovered_instance_id_hash = state
            .hovered_instance
            .as_ref()
            .map_or(InstanceIdHash::NONE, InstanceId::hash);

        let mut legends = Legends::default();
        {
            puffin::profile_scope!("Scene2D - load legends");
            for (_obj_type, obj_path, obj_store) in query.iter_object_stores(
                ctx.log_db,
                obj_tree_props,
                &[ObjectType::ClassDescription],
            ) {
                visit_type_data_2(
                    obj_store,
                    &FieldName::from("id"),
                    &query.time_query,
                    ("label", "color"),
                    |_instance_index: Option<&IndexHash>,
                     _time: i64,
                     msg_id: &MsgId,
                     id: &i32,
                     label: Option<&String>,
                     color: Option<&[u8; 4]>| {
                        let cdm = legends.0.entry(obj_path.clone()).or_insert_with(|| {
                            Arc::new(ClassDescriptionMap {
                                msg_id: *msg_id,
                                map: Default::default(),
                            })
                        });

                        Arc::get_mut(cdm).unwrap().map.insert(
                            *id,
                            ClassDescription {
                                label: label.map(|s| s.clone().into()),
                                color: color.cloned(),
                            },
                        );
                    },
                );
            }
        }

        {
            puffin::profile_scope!("Scene2D - load images");
            let images = query
                .iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::Image])
                .flat_map(|(_obj_type, obj_path, obj_store)| {
                    let mut batch = Vec::new();
                    visit_type_data_4(
                        obj_store,
                        &FieldName::from("tensor"),
                        &query.time_query,
                        ("_visible", "color", "meter", "legend"),
                        |instance_index: Option<&IndexHash>,
                         _time: i64,
                         msg_id: &MsgId,
                         tensor: &re_log_types::Tensor,
                         visible: Option<&bool>,
                         color: Option<&[u8; 4]>,
                         meter: Option<&f32>,
                         legend: Option<&ObjPath>| {
                            let visible = *visible.unwrap_or(&true);
                            let two_dimensions_min = tensor.shape.len() >= 2;
                            if !visible || !two_dimensions_min {
                                return;
                            }

                            let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                            let image = Image {
                                msg_id: *msg_id,
                                instance_hash: InstanceIdHash::from_path_and_index(
                                    obj_path,
                                    instance_index,
                                ),
                                tensor: tensor.clone(), // shallow
                                meter: meter.copied(),
                                legend: legends.find(legend),
                                paint_props: paint_properties(
                                    ctx,
                                    &hovered_instance_id_hash,
                                    obj_path,
                                    instance_index,
                                    color.copied(),
                                    DefaultColor::White,
                                    &None,
                                ),
                            };

                            batch.push(image);
                        },
                    );
                    batch
                });
            self.images.extend(images);

            for image in &self.images {
                let [h, w] = [image.tensor.shape[0].size, image.tensor.shape[1].size];
                self.bbox.extend_with(Pos2::ZERO);
                self.bbox.extend_with(pos2(w as _, h as _));
            }
        }

        {
            puffin::profile_scope!("Scene2D - load boxes");
            let boxes = query
                .iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::BBox2D])
                .flat_map(|(_obj_type, obj_path, obj_store)| {
                    let mut batch = Vec::new();
                    visit_type_data_4(
                        obj_store,
                        &FieldName::from("bbox"),
                        &query.time_query,
                        ("_visible", "color", "stroke_width", "label"),
                        |instance_index: Option<&IndexHash>,
                         _time: i64,
                         _msg_id: &MsgId,
                         bbox: &re_log_types::BBox2D,
                         visible: Option<&bool>,
                         color: Option<&[u8; 4]>,
                         stroke_width: Option<&f32>,
                         label: Option<&String>| {
                            let visible = *visible.unwrap_or(&true);
                            if !visible {
                                return;
                            }

                            let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                            let stroke_width = stroke_width.copied();

                            // TODO: we gotta have something to replace the ol' InstanceProps
                            let paint_props = paint_properties(
                                ctx,
                                &hovered_instance_id_hash,
                                obj_path,
                                instance_index,
                                color.copied(),
                                DefaultColor::Random,
                                &stroke_width,
                            );

                            batch.push(Box2D {
                                instance_hash: InstanceIdHash::from_path_and_index(
                                    obj_path,
                                    instance_index,
                                ),
                                bbox: bbox.clone(),
                                stroke_width,
                                label: label.map(ToOwned::to_owned),
                                paint_props,
                            });
                        },
                    );
                    batch
                });
            self.boxes.extend(boxes);

            for bbox in &self.boxes {
                self.bbox.extend_with(bbox.bbox.min.into());
                self.bbox.extend_with(bbox.bbox.max.into());
            }
        }

        {
            puffin::profile_scope!("Scene2D - load points");
            let points = query
                .iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::Point2D])
                .flat_map(|(_obj_type, obj_path, obj_store)| {
                    let mut batch = Vec::new();
                    visit_type_data_3(
                        obj_store,
                        &FieldName::from("pos"),
                        &query.time_query,
                        ("_visible", "color", "radius"),
                        |instance_index: Option<&IndexHash>,
                         _time: i64,
                         _msg_id: &MsgId,
                         pos: &[f32; 2],
                         visible: Option<&bool>,
                         color: Option<&[u8; 4]>,
                         radius: Option<&f32>| {
                            let visible = *visible.unwrap_or(&true);
                            if !visible {
                                return;
                            }

                            let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                            // TODO: we gotta have something to replace the ol' InstanceProps
                            let paint_props = paint_properties(
                                ctx,
                                &hovered_instance_id_hash,
                                obj_path,
                                instance_index,
                                color.copied(),
                                DefaultColor::Random,
                                &None,
                            );

                            batch.push(Point2D {
                                instance_hash: InstanceIdHash::from_path_and_index(
                                    obj_path,
                                    instance_index,
                                ),
                                pos: Pos2::new(pos[0], pos[1]),
                                radius: radius.copied(),
                                paint_props,
                            });
                        },
                    );
                    batch
                });
            self.points.extend(points);

            for point in &self.points {
                self.bbox.extend_with(point.pos);
            }
        }

        {
            puffin::profile_scope!("Scene2D - load line segments");
            let segments = query
                .iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::LineSegments2D])
                .flat_map(|(_obj_type, obj_path, obj_store)| {
                    let mut batch = Vec::new();
                    visit_type_data_3(
                        obj_store,
                        &FieldName::from("points"),
                        &query.time_query,
                        ("_visible", "color", "stroke_width"),
                        |instance_index: Option<&IndexHash>,
                         _time: i64,
                         _msg_id: &MsgId,
                         points: &DataVec,
                         visible: Option<&bool>,
                         color: Option<&[u8; 4]>,
                         stroke_width: Option<&f32>| {
                            let visible = *visible.unwrap_or(&true);
                            if !visible {
                                return;
                            }

                            let Some(points) = points.as_vec_of_vec2("LineSegments2D::points")
                                else { return };

                            let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                            let stroke_width = stroke_width.copied();

                            // TODO: we gotta have something to replace the ol' InstanceProps
                            let paint_props = paint_properties(
                                ctx,
                                &hovered_instance_id_hash,
                                obj_path,
                                instance_index,
                                color.copied(),
                                DefaultColor::Random,
                                &None,
                            );

                            batch.push(LineSegments2D {
                                instance_hash: InstanceIdHash::from_path_and_index(
                                    obj_path,
                                    instance_index,
                                ),
                                points: points.iter().map(|p| Pos2::new(p[0], p[1])).collect(),
                                stroke_width,
                                paint_props,
                            });
                        },
                    );
                    batch
                });
            self.line_segments.extend(segments);

            for segment in &self.line_segments {
                for &point in &segment.points {
                    self.bbox.extend_with(point);
                }
            }
        }
    }
}

impl Scene2D {
    pub fn clear(&mut self) {
        let Self {
            bbox,
            legends,
            images,
            boxes: bboxes,
            line_segments,
            points,
        } = self;

        *bbox = Rect::NOTHING;
        legends.0.clear();
        images.clear();
        bboxes.clear();
        line_segments.clear();
        points.clear();
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            bbox: _,
            legends: _,
            images,
            boxes: bboxes,
            line_segments,
            points,
        } = self;

        images.is_empty() && bboxes.is_empty() && line_segments.is_empty() && points.is_empty()
    }
}

// ---

// TODO: still that props mess

pub struct ObjectPaintProperties {
    pub is_hovered: bool,
    pub bg_stroke: Stroke,
    pub fg_stroke: Stroke,
}

impl ObjectPaintProperties {
    pub fn boost_radius_on_hover(&self, r: f32) -> f32 {
        if self.is_hovered {
            2.0 * r
        } else {
            r
        }
    }
}

#[derive(Clone, Copy)]
enum DefaultColor {
    White,
    Random,
}

fn paint_properties(
    ctx: &mut ViewerContext<'_>,
    hovered: &InstanceIdHash,
    obj_path: &ObjPath,
    instance_index: IndexHash,
    color: Option<[u8; 4]>,
    default_color: DefaultColor,
    stroke_width: &Option<f32>,
) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let color = color.map_or_else(
        || match default_color {
            DefaultColor::White => Color32::WHITE,
            DefaultColor::Random => {
                let [r, g, b] = ctx.random_color(obj_path);
                Color32::from_rgb(r, g, b)
            }
        },
        to_egui_color,
    );
    let is_hovered = &InstanceIdHash::from_path_and_index(obj_path, instance_index) == hovered;
    let fg_color = if is_hovered { Color32::WHITE } else { color };
    let stroke_width = stroke_width.unwrap_or(1.5);
    let stoke_width = if is_hovered {
        2.0 * stroke_width
    } else {
        stroke_width
    };
    let bg_stroke = Stroke::new(stoke_width + 2.0, bg_color);
    let fg_stroke = Stroke::new(stoke_width, fg_color);

    ObjectPaintProperties {
        is_hovered,
        bg_stroke,
        fg_stroke,
    }
}

fn to_egui_color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
