use std::sync::Arc;

use egui::{pos2, Color32, Pos2, Rect, Stroke};
use re_data_store::{
    query::{visit_type_data_2, visit_type_data_3, visit_type_data_4},
    FieldName, InstanceIdHash, ObjPath,
};
use re_log_types::{DataVec, IndexHash, MsgId, ObjectType, Tensor};

use crate::{ui::SceneQuery, ViewerContext};

use super::{ClassDescription, ClassDescriptionMap, Legend, Legends};

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

    /// If true, draw a frame around it
    pub is_hovered: bool,
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

impl Scene2D {
    /// Loads all 2D objects into the scene according to the given query.
    ///
    /// In addition to the query, we also pass in the 2D state from last frame, so that we can
    /// compute custom paint properties for hovered items.
    pub(crate) fn load_objects(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_legends(ctx, query); // before images!
        self.load_images(ctx, query);
        self.load_boxes(ctx, query);
        self.load_points(ctx, query);
        self.load_line_segments(ctx, query);
    }

    fn load_legends(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::ClassDescription])
        {
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
                    let cdm = self.legends.0.entry(obj_path.clone()).or_insert_with(|| {
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

    fn load_images(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let images = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Image])
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
                        let two_or_three_dims = 2 <= tensor.shape.len() && tensor.shape.len() <= 3;
                        if !visible || !two_or_three_dims {
                            return;
                        }

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                        let paint_props = paint_properties(
                            ctx,
                            obj_path,
                            color.copied(),
                            DefaultColor::White,
                            &None,
                        );

                        let image = Image {
                            msg_id: *msg_id,
                            instance_hash: InstanceIdHash::from_path_and_index(
                                obj_path,
                                instance_index,
                            ),
                            tensor: tensor.clone(), // shallow
                            meter: meter.copied(),
                            legend: self.legends.find(legend),
                            paint_props,
                            is_hovered: false, // Will be filled in later
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

    fn load_boxes(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let boxes = query
            .iter_object_stores(ctx.log_db, &[ObjectType::BBox2D])
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

                        let paint_props = paint_properties(
                            ctx,
                            obj_path,
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

    fn load_points(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let points = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Point2D])
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

                        let paint_props = paint_properties(
                            ctx,
                            obj_path,
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

    fn load_line_segments(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let segments = query
            .iter_object_stores(ctx.log_db, &[ObjectType::LineSegments2D])
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

                        let paint_props = paint_properties(
                            ctx,
                            obj_path,
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

impl Scene2D {
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

pub struct ObjectPaintProperties {
    pub bg_stroke: Stroke,
    pub fg_stroke: Stroke,
}

#[derive(Clone, Copy)]
enum DefaultColor {
    White,
    Random,
}

fn paint_properties(
    ctx: &mut ViewerContext<'_>,
    obj_path: &ObjPath,
    color: Option<[u8; 4]>,
    default_color: DefaultColor,
    stroke_width: &Option<f32>,
) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let fg_color = color.map_or_else(
        || match default_color {
            DefaultColor::White => Color32::WHITE,
            DefaultColor::Random => {
                let [r, g, b] = ctx.random_color(obj_path);
                Color32::from_rgb(r, g, b)
            }
        },
        to_egui_color,
    );
    let stroke_width = stroke_width.unwrap_or(1.5);
    let bg_stroke = Stroke::new(stroke_width + 2.0, bg_color);
    let fg_stroke = Stroke::new(stroke_width, fg_color);

    ObjectPaintProperties {
        bg_stroke,
        fg_stroke,
    }
}

fn to_egui_color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
