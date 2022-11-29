use std::sync::Arc;

use ahash::HashMap;
use egui::{pos2, Color32, Pos2, Rect, Stroke};
use re_data_store::{
    query::{visit_type_data_2, visit_type_data_4, visit_type_data_5},
    FieldName, InstanceIdHash,
};
use re_log_types::{
    context::{ClassId, KeypointId},
    DataVec, IndexHash, MsgId, ObjectType, Tensor,
};

use crate::{
    ui::{
        annotations::{auto_color, AnnotationMap, DefaultColor},
        Annotations, SceneQuery,
    },
    ViewerContext,
};

// ---

// TODO(cmc): just turn all colors into Color32?

pub struct Image {
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
    pub annotations: Option<Arc<Annotations>>,

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

    pub label: Option<String>,
}

/// A 2D scene, with everything needed to render it.
pub struct Scene2D {
    /// Estimated bounding box of all data. Accumulated.
    pub bbox: Rect,
    pub annotation_map: AnnotationMap,

    pub images: Vec<Image>,
    pub boxes: Vec<Box2D>,
    pub line_segments: Vec<LineSegments2D>,
    pub points: Vec<Point2D>,
}

impl Default for Scene2D {
    fn default() -> Self {
        Self {
            bbox: Rect::NOTHING,
            annotation_map: Default::default(),
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

        self.annotation_map.load(ctx, query);

        self.load_images(ctx, query);
        self.load_boxes(ctx, query);
        self.load_points(ctx, query);
        self.load_line_segments(ctx, query);
    }

    fn load_images(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let images = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Image])
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_2(
                    obj_store,
                    &FieldName::from("tensor"),
                    &time_query,
                    ("color", "meter"),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     tensor: &re_log_types::Tensor,
                     color: Option<&[u8; 4]>,
                     meter: Option<&f32>| {
                        let two_or_three_dims = 2 <= tensor.shape.len() && tensor.shape.len() <= 3;
                        if !two_or_three_dims {
                            return;
                        }

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                        let annotations = self.annotation_map.find(obj_path);
                        let color = annotations
                            .class_description(None)
                            .annotation_info()
                            .color(color, DefaultColor::OpaqueWhite);

                        let paint_props = paint_properties(color, &None);

                        let image = Image {
                            instance_hash: InstanceIdHash::from_path_and_index(
                                obj_path,
                                instance_index,
                            ),
                            tensor: tensor.clone(), // shallow
                            meter: meter.copied(),
                            annotations: Some(annotations),
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
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_4(
                    obj_store,
                    &FieldName::from("bbox"),
                    &time_query,
                    ("color", "stroke_width", "label", "class_id"),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     bbox: &re_log_types::BBox2D,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>,
                     label: Option<&String>,
                     class_id: Option<&i32>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let stroke_width = stroke_width.copied();

                        let annotations = self.annotation_map.find(obj_path);
                        let annotation_info = annotations
                            .class_description(class_id.map(|i| ClassId(*i as _)))
                            .annotation_info();
                        let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));
                        let label = annotation_info.label(label);

                        let paint_props = paint_properties(color, &stroke_width);

                        batch.push(Box2D {
                            instance_hash: InstanceIdHash::from_path_and_index(
                                obj_path,
                                instance_index,
                            ),
                            bbox: bbox.clone(),
                            stroke_width,
                            label,
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
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                let annotations = self.annotation_map.find(obj_path);
                let default_color = DefaultColor::ObjPath(obj_path);

                // If keypoints ids show up we may need to connect them later!
                // We include time in the key, so that the "Visible history" (time range queries) feature works.
                let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, Pos2>> =
                Default::default();

                visit_type_data_5(
                    obj_store,
                    &FieldName::from("pos"),
                    &time_query,
                    ("color", "radius", "label", "class_id", "keypoint_id"),
                    |instance_index: Option<&IndexHash>,
                     time: i64,
                     _msg_id: &MsgId,
                     pos: &[f32; 2],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     label: Option<&String>,
                     class_id: Option<&i32>,
                     keypoint_id: Option<&i32>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let pos = Pos2::new(pos[0], pos[1]);

                        let class_id = class_id.map(|i| ClassId(*i as _));
                        let class_description =
                            annotations.class_description(class_id);

                            let annotation_info = if let Some(keypoint_id) = keypoint_id {
                                let keypoint_id = KeypointId(*keypoint_id as _);
                                if let Some(class_id) = class_id {
                                    keypoints
                                        .entry((class_id, time))
                                        .or_insert_with(Default::default)
                                        .insert(keypoint_id, pos);
                                }

                                class_description.annotation_info_with_keypoint(keypoint_id)
                            } else {
                                class_description.annotation_info()
                            };
                        let color = annotation_info.color(color, default_color);
                        let label = annotation_info.label(label);
                        let paint_props = paint_properties(color, &None);

                        batch.push(Point2D {
                            instance_hash: InstanceIdHash::from_path_and_index(
                                obj_path,
                                instance_index,
                            ),
                            pos,
                            radius: radius.copied(),
                            paint_props,
                            label,
                        });
                    },
                );

                // TODO(andreas): Make user configurable with this as the default.
                let show_labels = batch.len() < 10;
                if !show_labels {
                    for point in &mut batch {
                        point.label = None;
                    }
                }

                // Generate keypoint connections if any.
                let instance_hash = InstanceIdHash::from_path_and_index(obj_path, IndexHash::NONE);
                for ((class_id, _time), keypoints_in_class) in &keypoints {
                    let Some(class_description) = annotations.context.class_map.get(class_id) else {
                        continue;
                    };

                    let color = class_description
                        .info
                        .color
                        .unwrap_or_else(|| auto_color(class_description.info.id));

                    for (a, b) in &class_description.keypoint_connections {
                        let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b)) else {
                            re_log::warn_once!(
                                "Keypoint connection from index {:?} to {:?} could not be resolved in object {:?}",
                                a, b, obj_path
                            );
                            continue;
                        };
                        self.line_segments.push(LineSegments2D {
                            instance_hash,
                            points: vec![*a, *b],
                            stroke_width: None,
                            paint_props: paint_properties(color, &None),
                        });
                    }
                }

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
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                let annotations = self.annotation_map.find(obj_path);

                visit_type_data_2(
                    obj_store,
                    &FieldName::from("points"),
                    &time_query,
                    ("color", "stroke_width"),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     points: &DataVec,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        let Some(points) = points.as_vec_of_vec2("LineSegments2D::points")
                                else { return };

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let stroke_width = stroke_width.copied();

                        // TODO(andreas): support class ids for line segments
                        let annotation_info = annotations.class_description(None).annotation_info();
                        let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));

                        let paint_props = paint_properties(color, &None);

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
            annotation_map: _,
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

fn paint_properties(color: [u8; 4], stroke_width: &Option<f32>) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let fg_color = to_egui_color(color);
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
