use std::sync::Arc;

use ahash::HashMap;
use egui::{pos2, NumExt, Pos2, Rect, Stroke};
use itertools::Itertools as _;
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

use re_renderer::{renderer::PointCloudPoint, Color32, LineStripSeriesBuilder, Size};

use smallvec::smallvec;

pub struct HoverableImage {
    pub instance_hash: InstanceIdHash,

    pub tensor: Tensor,
    /// If this is a depth map, how long is a meter?
    ///
    /// For instance, with a `u16` dtype one might have
    /// `meter == 1000.0` for millimeter precision
    /// up to a ~65m range.
    pub meter: Option<f32>,

    /// A thing that provides additional semantic context for your dtype.
    pub annotations: Arc<Annotations>,
}

pub enum LabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(Rect),
    /// Labels a given point (in scene coordinates)
    Point(egui::Pos2),
}

pub struct Label2D {
    pub text: String,
    pub color: Color32,
    /// The shape being labled.
    pub target: LabelTarget,
    /// What is hovered if this label is hovered.
    pub labled_instance: InstanceIdHash,
}

/// A 2D scene, with everything needed to render it.
pub struct Scene2D {
    /// Estimated bounding box of all data. Accumulated.
    pub bbox: Rect,
    pub annotation_map: AnnotationMap,

    pub textured_rectangles: Vec<re_renderer::renderer::Rectangle>,
    pub points: Vec<PointCloudPoint>,
    pub lines: LineStripSeriesBuilder<()>,

    pub labels: Vec<Label2D>,

    /// Hoverable rects in scene units and their instance id hashes
    pub hoverable_rects: Vec<(Rect, InstanceIdHash)>,

    /// Images are a special case of rects where we're storing some extra information to allow minature previews etc.
    pub hoverable_images: Vec<HoverableImage>,

    /// Hoverable points in scene units and their instance id hashes
    pub hoverable_points: Vec<(egui::Pos2, InstanceIdHash)>,

    /// Hoverable line strips in scene units and their instance id hashes
    pub hoverable_line_strips: Vec<(smallvec::SmallVec<[egui::Pos2; 2]>, InstanceIdHash)>,
}

impl Default for Scene2D {
    fn default() -> Self {
        Self {
            bbox: Rect::NOTHING,
            annotation_map: Default::default(),
            hoverable_images: Default::default(),
            textured_rectangles: Default::default(),
            points: Default::default(),
            lines: Default::default(),
            labels: Default::default(),
            hoverable_rects: Default::default(),
            hoverable_points: Default::default(),
            hoverable_line_strips: Default::default(),
        }
    }
}

fn apply_hover_effect(paint_props: &mut ObjectPaintProperties) {
    paint_props.bg_stroke.width *= 2.0;
    paint_props.bg_stroke.color = Color32::BLACK;

    paint_props.fg_stroke.width *= 2.0;
    paint_props.fg_stroke.color = Color32::WHITE;
}

impl Scene2D {
    /// Loads all 2D objects into the scene according to the given query.
    ///
    /// In addition to the query, we also pass in the 2D state from last frame, so that we can
    /// compute custom paint properties for hovered items.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        self.load_images(ctx, query, hovered_instance);
        self.load_boxes(ctx, query, hovered_instance);
        self.load_line_segments(ctx, query, hovered_instance);
        self.load_points(ctx, query, hovered_instance);

        self.recalculate_bounding_box();
    }

    fn recalculate_bounding_box(&mut self) {
        for image in &self.hoverable_images {
            let [h, w] = [image.tensor.shape[0].size, image.tensor.shape[1].size];
            self.bbox.extend_with(Pos2::ZERO);
            self.bbox.extend_with(pos2(w as _, h as _));
        }

        for (point, _) in &self.hoverable_points {
            self.bbox.extend_with(*point);
        }

        for (points, _) in &self.hoverable_line_strips {
            for p in points {
                self.bbox.extend_with(*p);
            }
        }
    }

    fn load_images(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Image])
        {
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
                    let (w, h) = (tensor.shape[1].size as f32, tensor.shape[0].size as f32);

                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let instance_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    let annotations = self.annotation_map.find(obj_path);
                    let color = annotations
                        .class_description(None)
                        .annotation_info()
                        .color(color, DefaultColor::OpaqueWhite);

                    let paint_props = paint_properties(color, None);

                    if hovered_instance == instance_hash {
                        self.lines
                            .add_axis_aligned_rectangle_outline_2d(
                                glam::Vec2::ZERO,
                                glam::vec2(w, h),
                            )
                            .color(paint_props.fg_stroke.color)
                            .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
                    }

                    let legend = Some(annotations.clone());
                    let tensor_view =
                        ctx.cache
                            .image
                            .get_view_with_annotations(tensor, &legend, ctx.render_ctx);

                    self.textured_rectangles
                        .push(re_renderer::renderer::Rectangle {
                            top_left_corner_position: glam::Vec3::ZERO,
                            extent_u: glam::Vec3::X * w,
                            extent_v: glam::Vec3::Y * h,
                            texture: tensor_view.texture_handle,
                            texture_filter_magnification:
                                re_renderer::renderer::TextureFilterMag::Nearest,
                            texture_filter_minification:
                                re_renderer::renderer::TextureFilterMin::Linear,
                            multiplicative_tint: paint_props.fg_stroke.color.into(),
                        });

                    self.hoverable_images.push(HoverableImage {
                        instance_hash,
                        tensor: tensor.clone(),
                        meter: meter.copied(),
                        annotations,
                    });
                },
            );
        }

        let total_num_images = self.textured_rectangles.len();
        for (image_idx, img) in self.textured_rectangles.iter_mut().enumerate() {
            img.top_left_corner_position = glam::vec3(
                0.0,
                0.0,
                // We use RDF (X=Right, Y=Down, Z=Forward) for 2D spaces, so we want lower Z in order to put images on top
                (total_num_images - image_idx - 1) as f32 * 0.1,
            );

            let opacity = if image_idx == 0 {
                1.0 // bottom image
            } else {
                // make top images transparent
                1.0 / total_num_images.at_most(20) as f32 // avoid precision problems in framebuffer
            };
            img.multiplicative_tint = img.multiplicative_tint.multiply(opacity);
        }
    }

    fn load_boxes(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::BBox2D])
        {
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
                    let instance_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    let annotations = self.annotation_map.find(obj_path);
                    let annotation_info = annotations
                        .class_description(class_id.map(|i| ClassId(*i as _)))
                        .annotation_info();
                    let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));
                    let label = annotation_info.label(label);

                    let rect = Rect::from_min_max(bbox.min.into(), bbox.max.into());
                    self.hoverable_rects.push((rect, instance_hash));

                    let mut paint_props = paint_properties(color, stroke_width);
                    if hovered_instance == instance_hash {
                        apply_hover_effect(&mut paint_props);
                    }

                    self.lines
                        .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                        .color(paint_props.bg_stroke.color)
                        .radius(Size::new_points(paint_props.bg_stroke.width * 0.5));
                    self.lines
                        .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));

                    if let Some(label) = label {
                        self.labels.push(Label2D {
                            text: label,
                            color: paint_props.fg_stroke.color,
                            target: LabelTarget::Rect(rect),
                            labled_instance: instance_hash,
                        });
                    }
                },
            );
        }
    }

    fn load_points(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        // Ensure keypoint connection lines are behind points.
        let connection_depth = self.lines.next_2d_z;
        let point_depth = self.lines.next_2d_z - 0.1;

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point2D])
        {
            let mut label_batch = Vec::new();
            let max_num_labels = 10;

            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec2>> =
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
                    let instance_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);
                    let pos = glam::vec2(pos[0], pos[1]);

                    let class_id = class_id.map(|i| ClassId(*i as _));
                    let class_description = annotations.class_description(class_id);

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

                    let mut paint_props = paint_properties(color, radius);
                    if hovered_instance == instance_hash {
                        apply_hover_effect(&mut paint_props);
                    }

                    self.points.push(PointCloudPoint {
                        position: pos.extend(point_depth),
                        radius: Size::new_points(paint_props.bg_stroke.width * 0.5),
                        color: paint_props.bg_stroke.color,
                    });
                    self.points.push(PointCloudPoint {
                        position: pos.extend(point_depth - 0.1),
                        radius: Size::new_points(paint_props.fg_stroke.width * 0.5),
                        color: paint_props.fg_stroke.color,
                    });

                    let pos = egui::pos2(pos.x, pos.y);
                    self.hoverable_points.push((pos, instance_hash));

                    if let Some(label) = label {
                        if label_batch.len() < max_num_labels {
                            label_batch.push(Label2D {
                                text: label,
                                color: paint_props.fg_stroke.color,
                                target: LabelTarget::Point(pos),
                                labled_instance: instance_hash,
                            });
                        }
                    }
                },
            );

            // TODO(andreas): Make user configurable with this as the default.
            if label_batch.len() < max_num_labels {
                self.labels.extend(label_batch.into_iter());
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
                    // TODO(andreas): No outlines for these?
                    // Specify depth explicitly so we're behind the points.
                    self.lines
                        .add_segment(a.extend(connection_depth), b.extend(connection_depth))
                        .color(to_egui_color(color));

                    self.hoverable_line_strips.push((
                        smallvec![egui::pos2(a.x, a.y), egui::pos2(b.x, b.y)],
                        instance_hash,
                    ));
                }
            }
        }
    }

    fn load_line_segments(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::LineSegments2D])
        {
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
                    let instance_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    // TODO(andreas): support class ids for line segments
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));

                    let mut paint_props = paint_properties(color, stroke_width);
                    if hovered_instance == instance_hash {
                        apply_hover_effect(&mut paint_props);
                    }

                    // TODO(andreas): support outlines directly by re_renderer (need only 1 and 2 *point* black outlines)
                    self.lines
                        .add_segments_2d(
                            points
                                .iter()
                                .tuple_windows()
                                .map(|(a, b)| (glam::vec2(a[0], a[1]), glam::vec2(b[0], b[1]))),
                        )
                        .color(paint_props.bg_stroke.color)
                        .radius(Size::new_points(paint_props.bg_stroke.width * 0.5));
                    self.lines
                        .add_segments_2d(
                            points
                                .iter()
                                .tuple_windows()
                                .map(|(a, b)| (glam::vec2(a[0], a[1]), glam::vec2(b[0], b[1]))),
                        )
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));

                    self.hoverable_line_strips.push((
                        points.iter().map(|p| egui::pos2(p[0], p[1])).collect(),
                        instance_hash,
                    ));

                    for p in points {
                        self.bbox.extend_with(egui::pos2(p[0], p[1]));
                    }
                },
            );
        }
    }
}

impl Scene2D {
    pub fn is_empty(&self) -> bool {
        let Self {
            bbox: _,
            annotation_map: _,
            hoverable_images: images,
            points,
            lines,
            textured_rectangles,
            labels,
            hoverable_rects: _,
            hoverable_points: _,
            hoverable_line_strips: _,
        } = self;

        images.is_empty()
            && lines.is_empty()
            && points.is_empty()
            && labels.is_empty()
            && textured_rectangles.is_empty()
    }
}

// ---

pub struct ObjectPaintProperties {
    pub bg_stroke: Stroke,
    pub fg_stroke: Stroke,
}

// TODO(andreas): we're no longer using egui strokes. Replace this.
fn paint_properties(color: [u8; 4], stroke_width: Option<&f32>) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let fg_color = to_egui_color(color);
    let stroke_width = stroke_width.map_or(1.5, |w| *w);
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
