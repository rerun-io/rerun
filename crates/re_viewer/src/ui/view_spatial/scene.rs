use std::sync::Arc;

use ahash::HashMap;
use egui::NumExt as _;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_data_store::query::{
    visit_type_data_1, visit_type_data_2, visit_type_data_3, visit_type_data_4, visit_type_data_5,
};
use re_data_store::{FieldName, InstanceIdHash};
use re_log_types::{
    context::{ClassId, KeypointId},
    DataVec, IndexHash, MeshId, MsgId, ObjectType, Tensor,
};
use re_renderer::{
    renderer::{LineStripFlags, MeshInstance, PointCloudPoint},
    Color32, LineStripSeriesBuilder, Size,
};

use crate::{
    math::line_segment_distance_sq_to_point_2d,
    misc::{mesh_loader::CpuMesh, ViewerContext},
    ui::{
        annotations::{auto_color, AnnotationMap, DefaultColor},
        view_spatial::axis_color,
        Annotations, SceneQuery,
    },
};

use super::{eye::Eye, SpaceCamera3D};

// ----------------------------------------------------------------------------

pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),

    /// e.g. the camera mesh
    StaticGlb(MeshId, &'static [u8]),
}

impl MeshSourceData {
    pub fn mesh_id(&self) -> MeshId {
        match self {
            MeshSourceData::Mesh3D(mesh) => mesh.mesh_id(),
            MeshSourceData::StaticGlb(id, _) => *id,
        }
    }
}

/// TODO(andreas): Scene should only care about converted rendering primitive.
pub struct MeshSource {
    pub instance_id_hash: InstanceIdHash,
    // TODO(andreas): Make this Conformal3 once glow is gone?
    pub world_from_mesh: macaw::Affine3A,
    pub mesh: Arc<CpuMesh>,
    pub additive_tint: Option<Color32>,
}

pub struct Image {
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

pub enum Label2DTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),
    /// Labels a given point (in scene coordinates)
    Point(egui::Pos2),
}

// TODO(andreas): Merge Label2D and Label3D
pub struct Label2D {
    pub text: String,
    pub color: Color32,
    /// The shape being labled.
    pub target: Label2DTarget,
    /// What is hovered if this label is hovered.
    pub labled_instance: InstanceIdHash,
}

pub struct Label3D {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

fn to_ecolor([r, g, b, a]: [u8; 4]) -> Color32 {
    // TODO(andreas): ecolor should have a utility to get an array
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Data necessary to setup the ui [`SceneSpatial`] but of no interest to `re_renderer`.
#[derive(Default)]
pub struct SceneSpatialUiData {
    pub labels_3d: Vec<Label3D>,
    pub labels_2d: Vec<Label2D>,

    /// Cursor within any of these rects cause the referred instance to be hovered.
    pub rects: Vec<(egui::Rect, InstanceIdHash)>,

    /// Images are a special case of rects where we're storing some extra information to allow miniature previews etc.
    pub images: Vec<Image>,
}

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
#[derive(Default)]
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    bounding_box: macaw::BoundingBox,

    /// TODO(andreas): Need to decide of this should be used for hovering as well. If so add another builder with meta-data?
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,
    pub line_strips: LineStripSeriesBuilder<InstanceIdHash>,
    /// TODO(andreas): re_renderer should have a point builder <https://github.com/rerun-io/rerun/issues/509>
    pub points: Vec<PointCloudPoint>,

    /// Assigns an instance id to every point. Needs to have as many elements as points
    pub point_ids: Vec<InstanceIdHash>,

    pub meshes: Vec<MeshSource>,
}

impl SceneSpatialPrimitives {
    /// bounding box covering the rendered scene
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
    }

    pub fn recalculate_bounding_box(&mut self) {
        crate::profile_function!();

        self.bounding_box = macaw::BoundingBox::nothing();

        for rect in &self.textured_rectangles {
            self.bounding_box.extend(rect.top_left_corner_position);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_u);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        }

        for point in &self.points {
            self.bounding_box.extend(point.position);
        }

        for vertex in &self.line_strips.vertices {
            self.bounding_box.extend(vertex.pos);
        }

        for mesh in &self.meshes {
            self.bounding_box = self.bounding_box.union(*mesh.mesh.bbox());
        }
    }

    pub fn mesh_instances(&self) -> Vec<MeshInstance> {
        crate::profile_function!();
        self.meshes
            .iter()
            .flat_map(|mesh| {
                let (scale, rotation, translation) =
                    mesh.world_from_mesh.to_scale_rotation_translation();
                // TODO(andreas): The renderer should make it easy to apply a transform to a bunch of meshes
                let base_transform = macaw::Conformal3::from_scale_rotation_translation(
                    re_renderer::importer::to_uniform_scale(scale),
                    rotation,
                    translation,
                );
                mesh.mesh
                    .mesh_instances
                    .iter()
                    .map(move |instance| MeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        mesh: None, // Don't care.
                        world_from_mesh: base_transform * instance.world_from_mesh,
                        additive_tint: mesh.additive_tint.unwrap_or(Color32::TRANSPARENT),
                    })
            })
            .collect()
    }
}

#[derive(Default)]
pub struct SceneSpatial {
    pub annotation_map: AnnotationMap,
    pub primitives: SceneSpatialPrimitives,
    pub ui: SceneSpatialUiData,
}

impl SceneSpatial {
    /// Loads all 3D objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        self.load_points_3d(ctx, query, hovered_instance);
        self.load_boxes_3d(ctx, query, hovered_instance);
        self.load_lines_3d(ctx, query, hovered_instance);
        self.load_arrows_3d(ctx, query, hovered_instance);
        self.load_meshes(ctx, query, hovered_instance);

        self.load_images(ctx, query, hovered_instance);
        self.load_boxes_2d(ctx, query, hovered_instance);
        self.load_line_segments_2d(ctx, query, hovered_instance);
        self.load_points_2d(ctx, query, hovered_instance);

        self.primitives.recalculate_bounding_box();
    }

    const HOVER_COLOR: Color32 = Color32::from_rgb(255, 200, 200);

    fn hover_size_boost(size: Size) -> Size {
        if size.is_auto() {
            Size::AUTO_LARGE
        } else {
            size * 1.5
        }
    }

    fn load_points_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        query
            .iter_object_stores(ctx.log_db, &[ObjectType::Point3D])
            .for_each(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch_size = 0;
                let mut show_labels = true;
                let mut label_batch = Vec::new();

                // If keypoints ids show up we may need to connect them later!
                // We include time in the key, so that the "Visible history" (time range queries) feature works.
                let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>> =
                    Default::default();

                let annotations = self.annotation_map.find(obj_path);
                let default_color = DefaultColor::ObjPath(obj_path);

                visit_type_data_5(
                    obj_store,
                    &FieldName::from("pos"),
                    &time_query,
                    ("color", "radius", "label", "class_id", "keypoint_id"),
                    |instance_index: Option<&IndexHash>,
                     time: i64,
                     _msg_id: &MsgId,
                     pos: &[f32; 3],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     label: Option<&String>,
                     class_id: Option<&i32>,
                     keypoint_id: Option<&i32>| {
                        batch_size += 1;

                        let position = Vec3::from_slice(pos);

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let class_id = class_id.map(|i| ClassId(*i as _));
                        let class_description = annotations.class_description(class_id);

                        let annotation_info = if let Some(keypoint_id) = keypoint_id {
                            let keypoint_id = KeypointId(*keypoint_id as _);
                            if let Some(class_id) = class_id {
                                keypoints
                                    .entry((class_id, time))
                                    .or_insert_with(Default::default)
                                    .insert(keypoint_id, position);
                            }

                            class_description.annotation_info_with_keypoint(keypoint_id)
                        } else {
                            class_description.annotation_info()
                        };

                        let mut color = to_ecolor(annotation_info.color(color, default_color));
                        let mut radius = radius.copied().map_or(Size::AUTO, Size::new_scene);

                        if instance_id_hash == hovered_instance {
                            color = Self::HOVER_COLOR;
                            radius = Self::hover_size_boost(radius);
                        }

                        show_labels = batch_size < 10;
                        if show_labels {
                            if let Some(label) = annotation_info.label(label) {
                                label_batch.push(Label3D {
                                    text: label,
                                    origin: position,
                                });
                            }
                        }

                        self.primitives.points.push(PointCloudPoint {
                            position,
                            radius,
                            color,
                        });
                        self.primitives.point_ids.push(instance_id_hash);
                    },
                );

                if show_labels {
                    self.ui.labels_3d.extend(label_batch);
                }

                self.load_keypoint_connections(obj_path, keypoints, &annotations);
            });
    }

    fn load_keypoint_connections(
        &mut self,
        obj_path: &re_data_store::ObjPath,
        keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>,
        annotations: &Arc<Annotations>,
    ) {
        // Generate keypoint connections if any.
        let instance_id_hash = InstanceIdHash::from_path_and_index(obj_path, IndexHash::NONE);
        for ((class_id, _time), keypoints_in_class) in keypoints {
            let Some(class_description) = annotations.context.class_map.get(&class_id) else {
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
                self.primitives
                    .line_strips
                    .add_segment(*a, *b)
                    .radius(Size::AUTO)
                    .color(to_ecolor(color))
                    .user_data(instance_id_hash);
            }
        }
    }

    fn load_boxes_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Box3D])
        {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_4(
                obj_store,
                &FieldName::from("obb"),
                &time_query,
                ("color", "stroke_width", "label", "class_id"),
                |instance_index: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 obb: &re_log_types::Box3,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>,
                 label: Option<&String>,
                 class_id: Option<&i32>| {
                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let mut line_radius =
                        stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                    let annotation_info = annotations
                        .class_description(class_id.map(|i| ClassId(*i as _)))
                        .annotation_info();
                    let mut color = to_ecolor(annotation_info.color(color, default_color));
                    let label = annotation_info.label(label);

                    let instance_id_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);
                    if instance_id_hash == hovered_instance {
                        color = Self::HOVER_COLOR;
                        line_radius = Self::hover_size_boost(line_radius);
                    }

                    self.add_box_3d(instance_id_hash, color, line_radius, label, obb);
                },
            );
        }
    }

    /// Both `Path3D` and `LineSegments3D`.
    fn load_lines_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (obj_type, obj_path, time_query, obj_store) in query.iter_object_stores(
            ctx.log_db,
            &[ObjectType::Path3D, ObjectType::LineSegments3D],
        ) {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

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
                    let what = match obj_type {
                        ObjectType::Path3D => "Path3D::points",
                        ObjectType::LineSegments3D => "LineSegments3D::points",
                        _ => return,
                    };
                    let Some(points) = points.as_vec_of_vec3(what) else { return };

                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let instance_id_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    let mut radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                    // TODO(andreas): support class ids for lines
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let mut color = to_ecolor(annotation_info.color(color, default_color));

                    if instance_id_hash == hovered_instance {
                        color = Self::HOVER_COLOR;
                        radius = Self::hover_size_boost(radius);
                    }

                    // Add renderer primitive
                    match obj_type {
                        ObjectType::Path3D => self
                            .primitives
                            .line_strips
                            .add_strip(points.iter().map(|v| Vec3::from_slice(v))),
                        ObjectType::LineSegments3D => self.primitives.line_strips.add_segments(
                            points
                                .chunks_exact(2)
                                .map(|points| (points[0].into(), points[1].into())),
                        ),
                        _ => unreachable!("already early outed earlier"),
                    }
                    .radius(radius)
                    .color(color)
                    .user_data(instance_id_hash);
                },
            );
        }
    }

    fn load_arrows_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Arrow3D])
        {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_3(
                obj_store,
                &FieldName::from("arrow3d"),
                &time_query,
                ("color", "width_scale", "label"),
                |instance_index: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 arrow: &re_log_types::Arrow3D,
                 color: Option<&[u8; 4]>,
                 width_scale: Option<&f32>,
                 label: Option<&String>| {
                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let instance_id_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    let width = width_scale.copied().unwrap_or(1.0);

                    // TODO(andreas): support class ids for arrows
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let color = annotation_info.color(color, default_color);
                    let label = annotation_info.label(label);

                    self.add_arrow(
                        instance_id_hash,
                        hovered_instance,
                        color,
                        Some(width),
                        label,
                        arrow,
                    );
                },
            );
        }
    }

    fn load_meshes(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        let meshes = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Mesh3D])
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_1(
                    obj_store,
                    &FieldName::from("mesh"),
                    &time_query,
                    ("color",),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     mesh: &re_log_types::Mesh3D,
                     _color: Option<&[u8; 4]>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let additive_tint = if hovered_instance == instance_id_hash {
                            Some(Self::HOVER_COLOR)
                        } else {
                            None
                        };

                        let Some(mesh) = ctx.cache.cpu_mesh.load(
                                &obj_path.to_string(),
                                &MeshSourceData::Mesh3D(mesh.clone()),
                                ctx.render_ctx
                            )
                            .map(|cpu_mesh| MeshSource {
                                instance_id_hash,
                                world_from_mesh: Default::default(),
                                mesh: cpu_mesh,
                                additive_tint,
                            }) else { return };

                        batch.push(mesh);
                    },
                );
                batch
            });

        self.primitives.meshes.extend(meshes);
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
                        self.primitives
                            .line_strips
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

                    self.primitives
                        .textured_rectangles
                        .push(re_renderer::renderer::TexturedRect {
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

                    self.ui.images.push(Image {
                        instance_hash,
                        tensor: tensor.clone(),
                        meter: meter.copied(),
                        annotations,
                    });
                },
            );
        }

        let total_num_images = self.primitives.textured_rectangles.len();
        for (image_idx, img) in self.primitives.textured_rectangles.iter_mut().enumerate() {
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

    fn load_boxes_2d(
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

                    // Hovering with a rect.
                    let rect = egui::Rect::from_min_max(bbox.min.into(), bbox.max.into());
                    self.ui.rects.push((rect, instance_hash));

                    let mut paint_props = paint_properties(color, stroke_width);
                    if hovered_instance == instance_hash {
                        apply_hover_effect(&mut paint_props);
                    }

                    // Lines don't associated with instance (i.e. won't participate in hovering)
                    self.primitives
                        .line_strips
                        .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                        .color(paint_props.bg_stroke.color)
                        .radius(Size::new_points(paint_props.bg_stroke.width * 0.5));
                    self.primitives
                        .line_strips
                        .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));

                    if let Some(label) = label {
                        self.ui.labels_2d.push(Label2D {
                            text: label,
                            color: paint_props.fg_stroke.color,
                            target: Label2DTarget::Rect(rect),
                            labled_instance: instance_hash,
                        });
                    }
                },
            );
        }
    }

    fn load_points_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        // Ensure keypoint connection lines are behind points.
        let connection_depth = self.primitives.line_strips.next_2d_z;
        let point_depth = self.primitives.line_strips.next_2d_z - 0.1;

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point2D])
        {
            let mut label_batch = Vec::new();
            let max_num_labels = 10;

            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>> =
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
                                .insert(keypoint_id, pos.extend(connection_depth));
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

                    self.primitives.points.push(PointCloudPoint {
                        position: pos.extend(point_depth),
                        radius: Size::new_points(paint_props.bg_stroke.width * 0.5),
                        color: paint_props.bg_stroke.color,
                    });
                    self.primitives.points.push(PointCloudPoint {
                        position: pos.extend(point_depth - 0.1),
                        radius: Size::new_points(paint_props.fg_stroke.width * 0.5),
                        color: paint_props.fg_stroke.color,
                    });
                    self.primitives.point_ids.push(instance_hash);
                    self.primitives.point_ids.push(InstanceIdHash::NONE);

                    if let Some(label) = label {
                        if label_batch.len() < max_num_labels {
                            label_batch.push(Label2D {
                                text: label,
                                color: paint_props.fg_stroke.color,
                                target: Label2DTarget::Point(egui::pos2(pos.x, pos.y)),
                                labled_instance: instance_hash,
                            });
                        }
                    }
                },
            );

            // TODO(andreas): Make user configurable with this as the default.
            if label_batch.len() < max_num_labels {
                self.ui.labels_2d.extend(label_batch.into_iter());
            }

            // Generate keypoint connections if any.
            self.load_keypoint_connections(obj_path, keypoints, &annotations);
        }
    }

    fn load_line_segments_2d(
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
                    self.primitives
                        .line_strips
                        .add_segments_2d(points.chunks_exact(2).map(|chunk| {
                            (
                                glam::vec2(chunk[0][0], chunk[0][1]),
                                glam::vec2(chunk[1][0], chunk[1][1]),
                            )
                        }))
                        .color(paint_props.bg_stroke.color)
                        .radius(Size::new_points(paint_props.bg_stroke.width * 0.5))
                        .user_data(instance_hash);
                    self.primitives
                        .line_strips
                        .add_segments_2d(points.chunks_exact(2).map(|chunk| {
                            (
                                glam::vec2(chunk[0][0], chunk[0][1]),
                                glam::vec2(chunk[1][0], chunk[1][1]),
                            )
                        }))
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
                },
            );
        }
    }

    // ---

    pub(crate) fn add_cameras(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        scene_bbox: &macaw::BoundingBox,
        viewport_size: egui::Vec2,
        eye: &Eye,
        cameras: &[SpaceCamera3D],
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

        for camera in cameras {
            let instance_id = InstanceIdHash {
                obj_path_hash: *camera.camera_obj_path.hash(),
                instance_index_hash: camera.instance_index_hash,
            };
            let is_hovered = instance_id == hovered_instance;

            let (line_radius, line_color) = if is_hovered {
                (Size::AUTO_LARGE, Self::HOVER_COLOR)
            } else {
                (Size::AUTO, Color32::from_rgb(255, 128, 128))
            }; // TODO(emilk): camera color

            let scale_based_on_scene_size = 0.05 * scene_bbox.size().length();
            let dist_to_eye = eye_camera_plane.distance(camera.position()).at_least(0.0);
            let scale_based_on_distance = dist_to_eye * point_size_at_one_meter * 50.0; // shrink as we get very close. TODO(emilk): fade instead!
            let scale = scale_based_on_scene_size.min(scale_based_on_distance);

            if ctx.options.show_camera_mesh_in_3d {
                if let Some(world_from_rub_view) = camera.world_from_rub_view() {
                    // The camera mesh file is 1m long in RUB (X=Right, Y=Up, Z=Back).
                    // The lens is at the origin.

                    let scale = Vec3::splat(scale);

                    let mesh_id = MeshId(uuid::uuid!("0de12a29-64ea-40b9-898b-63686b5436af"));
                    let world_from_mesh = world_from_rub_view * glam::Affine3A::from_scale(scale);

                    if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                        "camera_mesh",
                        &MeshSourceData::StaticGlb(
                            mesh_id,
                            include_bytes!("../../../data/camera.glb"),
                        ),
                        ctx.render_ctx,
                    ) {
                        let additive_tint = is_hovered.then_some(Self::HOVER_COLOR);

                        self.primitives.meshes.push(MeshSource {
                            instance_id_hash: instance_id,
                            world_from_mesh,
                            mesh: cpu_mesh,
                            additive_tint,
                        });
                    }
                }
            }

            if ctx.options.show_camera_axes_in_3d {
                let world_from_cam = camera.world_from_cam();

                // TODO(emilk): include the names of the axes ("Right", "Down", "Forward", etc)
                let cam_origin = camera.position();

                for (axis_index, dir) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
                    let axis_end = world_from_cam.transform_point3(scale * *dir);
                    let color = axis_color(axis_index);

                    self.primitives
                        .line_strips
                        .add_segment(cam_origin, axis_end)
                        .radius(Size::new_points(2.0))
                        .color(color)
                        .user_data(instance_id);
                }
            }

            self.add_camera_frustum(camera, scene_bbox, instance_id, line_radius, line_color);
        }
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        camera: &SpaceCamera3D,
        scene_bbox: &macaw::BoundingBox,
        instance_id: InstanceIdHash,
        line_radius: Size,
        color: Color32,
    ) -> Option<()> {
        let world_from_image = camera.world_from_image()?;
        let [w, h] = camera.pinhole?.resolution?;

        // At what distance do we end the frustum?
        let d = scene_bbox.size().length() * 0.3;

        // TODO(emilk): there is probably a off-by-one or off-by-half error here.
        // The image coordinates are in [0, w-1] range, so either we should use those limits
        // or [-0.5, w-0.5] for the "pixels are tiny squares" interpretation of the frustum.

        let corners = [
            world_from_image.transform_point3(d * vec3(0.0, 0.0, 1.0)),
            world_from_image.transform_point3(d * vec3(0.0, h, 1.0)),
            world_from_image.transform_point3(d * vec3(w, h, 1.0)),
            world_from_image.transform_point3(d * vec3(w, 0.0, 1.0)),
        ];

        let center = camera.position();

        let segments = [
            (center, corners[0]),     // frustum corners
            (center, corners[1]),     // frustum corners
            (center, corners[2]),     // frustum corners
            (center, corners[3]),     // frustum corners
            (corners[0], corners[1]), // `d` distance plane sides
            (corners[1], corners[2]), // `d` distance plane sides
            (corners[2], corners[3]), // `d` distance plane sides
            (corners[3], corners[0]), // `d` distance plane sides
        ];

        self.primitives
            .line_strips
            .add_segments(segments.into_iter())
            .radius(line_radius)
            .color(color)
            .user_data(instance_id);

        Some(())
    }

    fn add_arrow(
        &mut self,
        instance_id_hash: InstanceIdHash,
        hovered_instance: InstanceIdHash,
        color: [u8; 4],
        width_scale: Option<f32>,
        label: Option<String>,
        arrow: &re_log_types::Arrow3D,
    ) {
        drop(label); // TODO(andreas): support labels

        let re_log_types::Arrow3D { origin, vector } = arrow;

        let width_scale = width_scale.unwrap_or(1.0);
        let vector = Vec3::from_slice(vector);
        let origin = Vec3::from_slice(origin);

        let mut radius = Size::new_scene(width_scale * 0.5);
        let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
        let vector_len = vector.length();
        let end = origin + vector * ((vector_len - tip_length) / vector_len);

        let mut color = to_ecolor(color);
        if instance_id_hash == hovered_instance {
            color = Self::HOVER_COLOR;
            radius = Self::hover_size_boost(radius);
        }

        self.primitives
            .line_strips
            .add_segment(origin, end)
            .radius(radius)
            .color(color)
            .flags(re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE)
            .user_data(instance_id_hash);
    }

    fn add_box_3d(
        &mut self,
        instance_id: InstanceIdHash,
        color: Color32,
        line_radius: Size,
        label: Option<String>,
        box3: &re_log_types::Box3,
    ) {
        let re_log_types::Box3 {
            rotation,
            translation,
            half_size,
        } = box3;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = Vec3::from(*translation);
        let half_size = Vec3::from(*half_size);
        let transform =
            glam::Affine3A::from_scale_rotation_translation(half_size, rotation, translation);

        let corners = [
            transform.transform_point3(vec3(-0.5, -0.5, -0.5)),
            transform.transform_point3(vec3(-0.5, -0.5, 0.5)),
            transform.transform_point3(vec3(-0.5, 0.5, -0.5)),
            transform.transform_point3(vec3(-0.5, 0.5, 0.5)),
            transform.transform_point3(vec3(0.5, -0.5, -0.5)),
            transform.transform_point3(vec3(0.5, -0.5, 0.5)),
            transform.transform_point3(vec3(0.5, 0.5, -0.5)),
            transform.transform_point3(vec3(0.5, 0.5, 0.5)),
        ];

        if let Some(label) = label {
            self.ui.labels_3d.push(Label3D {
                text: label,
                origin: translation,
            });
        }

        let segments = [
            // bottom:
            (corners[0b000], corners[0b001]),
            (corners[0b000], corners[0b010]),
            (corners[0b011], corners[0b001]),
            (corners[0b011], corners[0b010]),
            // top:
            (corners[0b100], corners[0b101]),
            (corners[0b100], corners[0b110]),
            (corners[0b111], corners[0b101]),
            (corners[0b111], corners[0b110]),
            // sides:
            (corners[0b000], corners[0b100]),
            (corners[0b001], corners[0b101]),
            (corners[0b010], corners[0b110]),
            (corners[0b011], corners[0b111]),
        ];

        self.primitives
            .line_strips
            .add_segments(segments.into_iter())
            .radius(line_radius)
            .color(color)
            .user_data(instance_id);
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.bounding_box().is_nothing()
    }

    /// Heuristic whether the default way of looking at this scene should be 2d or 3d.
    pub fn prefer_2d_mode(&self) -> bool {
        // If any 2D interactable picture is there we regard it as 2d.
        if !self.ui.images.is_empty() {
            return true;
        }

        // Instead a mesh indicates 3d.
        if !self.primitives.meshes.is_empty() {
            return false;
        }

        // Otherwise do an heuristic based on the z extent of bounding box
        let bbox = self.primitives.bounding_box();
        bbox.min.z >= self.primitives.line_strips.next_2d_z * 2.0 && bbox.max.z < 1.0
    }

    pub fn picking(
        &self,
        pointer_in_ui: glam::Vec2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Option<(InstanceIdHash, Vec3)> {
        crate::profile_function!();

        let ui_from_world = eye.ui_from_world(rect);
        let world_from_ui = eye.world_from_ui(rect);

        let ray_in_world = {
            let ray_dir =
                world_from_ui.project_point3(Vec3::new(pointer_in_ui.x, pointer_in_ui.y, -1.0))
                    - eye.pos_in_world();
            macaw::Ray3::from_origin_dir(eye.pos_in_world(), ray_dir.normalize())
        };

        let SceneSpatialPrimitives {
            bounding_box: _,
            textured_rectangles: _, // TODO(andreas): Should be able to pick 2d rectangles!
            line_strips,
            points,
            point_ids,
            meshes,
        } = &self.primitives;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui

        let mut closest_z = f32::INFINITY;
        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        let mut closest_instance_id = None;

        {
            crate::profile_scope!("points_3d");
            debug_assert_eq!(point_ids.len(), points.len());
            for (point, instance_id_hash) in points.iter().zip(point_ids.iter()) {
                if instance_id_hash.is_none() {
                    continue;
                }

                // TODO(emilk): take point radius into account
                let pos_in_ui = ui_from_world.project_point3(point.position);
                if pos_in_ui.z < 0.0 {
                    continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                }
                let dist_sq = pos_in_ui.truncate().distance_squared(pointer_in_ui);
                if dist_sq < max_side_dist_sq {
                    let t = pos_in_ui.z.abs();
                    if t < closest_z || dist_sq < closest_side_dist_sq {
                        closest_z = t;
                        closest_side_dist_sq = dist_sq;
                        closest_instance_id = Some(*instance_id_hash);
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments_3d");
            for ((_line_strip, vertices), instance_id_hash) in line_strips
                .iter_strips_with_vertices()
                .zip(line_strips.strip_user_data.iter())
            {
                if instance_id_hash.is_none() {
                    continue;
                }
                // TODO(emilk): take line segment radius into account

                for (start, end) in vertices.tuple_windows() {
                    let a = ui_from_world.project_point3(start.pos);
                    let b = ui_from_world.project_point3(end.pos);
                    let dist_sq = line_segment_distance_sq_to_point_2d(
                        [a.truncate(), b.truncate()],
                        pointer_in_ui,
                    );

                    if dist_sq < max_side_dist_sq {
                        let t = a.z.abs(); // not very accurate
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(*instance_id_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if !mesh.instance_id_hash.is_some() {
                    continue;
                }
                let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.mesh.bbox());

                if t < f32::INFINITY {
                    let dist_sq = 0.0;
                    if t < closest_z || dist_sq < closest_side_dist_sq {
                        closest_z = t; // TODO(emilk): I think this is wrong
                        closest_side_dist_sq = dist_sq;
                        closest_instance_id = Some(mesh.instance_id_hash);
                    }
                }
            }
        }

        if let Some(closest_instance_id) = closest_instance_id {
            let closest_point = world_from_ui.project_point3(Vec3::new(
                pointer_in_ui.x,
                pointer_in_ui.y,
                closest_z,
            ));
            Some((closest_instance_id, closest_point))
        } else {
            None
        }
    }
}

pub struct ObjectPaintProperties {
    pub bg_stroke: egui::Stroke,
    pub fg_stroke: egui::Stroke,
}

// TODO(andreas): we're no longer using egui strokes. Replace this.
fn paint_properties(color: [u8; 4], stroke_width: Option<&f32>) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let fg_color = to_ecolor(color);
    let stroke_width = stroke_width.map_or(1.5, |w| *w);
    let bg_stroke = egui::Stroke::new(stroke_width + 2.0, bg_color);
    let fg_stroke = egui::Stroke::new(stroke_width, fg_color);

    ObjectPaintProperties {
        bg_stroke,
        fg_stroke,
    }
}

fn apply_hover_effect(paint_props: &mut ObjectPaintProperties) {
    paint_props.bg_stroke.width *= 2.0;
    paint_props.bg_stroke.color = Color32::BLACK;

    paint_props.fg_stroke.width *= 2.0;
    paint_props.fg_stroke.color = Color32::WHITE;
}
