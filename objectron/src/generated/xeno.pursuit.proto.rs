// option cc_api_version = 2;
// option java_api_version = 2;

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyPoint {
    /// The position of the keypoint in the local coordinate system of the rigid
    /// object.
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    #[prost(float, tag = "3")]
    pub z: f32,
    /// Sphere around the keypoint, indiciating annotator's confidence of the
    /// position in meters.
    #[prost(float, tag = "4")]
    pub confidence_radius: f32,
    /// The name of the keypoint (e.g. legs, head, etc.).
    /// Does not have to be unique.
    #[prost(string, tag = "5")]
    pub name: ::prost::alloc::string::String,
    /// Indicates whether the keypoint is hidden or not.
    #[prost(bool, tag = "6")]
    pub hidden: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Object {
    /// Unique object id through a sequence. There might be multiple objects of
    /// the same label in this sequence.
    #[prost(int32, tag = "1")]
    pub id: i32,
    /// Describes what category an object is. E.g. object class, attribute,
    /// instance or person identity. This provides additional context for the
    /// object type.
    #[prost(string, tag = "2")]
    pub category: ::prost::alloc::string::String,
    #[prost(enumeration = "object::Type", tag = "3")]
    pub r#type: i32,
    /// 3x3 row-major rotation matrix describing the orientation of the rigid
    /// object's frame of reference in the world-coordinate system.
    #[prost(float, repeated, tag = "4")]
    pub rotation: ::prost::alloc::vec::Vec<f32>,
    /// 3x1 vector describing the translation of the rigid object's frame of
    /// reference in the world-coordinate system in meters.
    #[prost(float, repeated, tag = "5")]
    pub translation: ::prost::alloc::vec::Vec<f32>,
    /// 3x1 vector describing the scale of the rigid object's frame of reference in
    /// the world-coordinate system in meters.
    #[prost(float, repeated, tag = "6")]
    pub scale: ::prost::alloc::vec::Vec<f32>,
    /// List of all the key points associated with this object in the object
    /// coordinate system.
    /// The first keypoint is always the object's frame of reference,
    /// e.g. the centroid of the box.
    /// E.g. bounding box with its center as frame of reference, the 9 keypoints :
    /// {0., 0., 0.},
    /// {-.5, -.5, -.5}, {-.5, -.5, +.5}, {-.5, +.5, -.5}, {-.5, +.5, +.5},
    /// {+.5, -.5, -.5}, {+.5, -.5, +.5}, {+.5, +.5, -.5}, {+.5, +.5, +.5}
    /// To get the bounding box in the world-coordinate system, we first scale the
    /// box then transform the scaled box.
    /// For example, bounding box in the world coordinate system is
    /// rotation * scale * keypoints + translation
    #[prost(message, repeated, tag = "7")]
    pub keypoints: ::prost::alloc::vec::Vec<KeyPoint>,
    #[prost(enumeration = "object::Method", tag = "8")]
    pub method: i32,
}
/// Nested message and enum types in `Object`.
pub mod object {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Type {
        UndefinedType = 0,
        BoundingBox = 1,
        Skeleton = 2,
        Mesh = 3,
    }
    /// Enum to reflect how this object is created.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Method {
        UnknownMethod = 0,
        /// Created by data annotation.
        Annotation = 1,
        /// Created by data augmentation.
        Augmentation = 2,
    }
}
/// The edge connecting two keypoints together
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Edge {
    /// keypoint id of the edge's source
    #[prost(int32, tag = "1")]
    pub source: i32,
    /// keypoint id of the edge's sink
    #[prost(int32, tag = "2")]
    pub sink: i32,
}
/// The skeleton template for different objects (e.g. humans, chairs, hands, etc)
/// The annotation tool reads the skeleton template dictionary.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Skeleton {
    /// The origin keypoint in the object coordinate system. (i.e. Point 0, 0, 0)
    #[prost(int32, tag = "1")]
    pub reference_keypoint: i32,
    /// The skeleton's category (e.g. human, chair, hand.). Should be unique in the
    /// dictionary.
    #[prost(string, tag = "2")]
    pub category: ::prost::alloc::string::String,
    /// Initialization value for all the keypoints in the skeleton in the object's
    /// local coordinate system. Pursuit will transform these points using object's
    /// transformation to get the keypoint in the world-cooridnate.
    #[prost(message, repeated, tag = "3")]
    pub keypoints: ::prost::alloc::vec::Vec<KeyPoint>,
    /// List of edges connecting keypoints
    #[prost(message, repeated, tag = "4")]
    pub edges: ::prost::alloc::vec::Vec<Edge>,
}
/// The list of all the modeled skeletons in our library. These models can be
/// objects (chairs, desks, etc), humans (full pose, hands, faces, etc), or box.
/// We can have multiple skeletons in the same file.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Skeletons {
    #[prost(message, repeated, tag = "1")]
    pub object: ::prost::alloc::vec::Vec<Skeleton>,
}
// option cc_api_version = 2;
// option java_api_version = 2;

/// Projection of a 3D point on an image, and its metric depth.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NormalizedPoint2D {
    /// x-y position of the 2d keypoint in the image coordinate system.
    /// u,v \in [0, 1], where top left corner is (0, 0) and the bottom-right corner
    /// is (1, 1).
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    /// The depth of the point in the camera coordinate system (in meters).
    #[prost(float, tag = "3")]
    pub depth: f32,
}
/// The 3D point in the camera coordinate system, the scales are in meters.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Point3D {
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    #[prost(float, tag = "3")]
    pub z: f32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnnotatedKeyPoint {
    #[prost(int32, tag = "1")]
    pub id: i32,
    #[prost(message, optional, tag = "2")]
    pub point_3d: ::core::option::Option<Point3D>,
    #[prost(message, optional, tag = "3")]
    pub point_2d: ::core::option::Option<NormalizedPoint2D>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ObjectAnnotation {
    /// Reference to the object identifier in ObjectInstance.
    #[prost(int32, tag = "1")]
    pub object_id: i32,
    /// For each objects, list all the annotated keypoints here.
    /// E.g. for bounding-boxes, we have 8 keypoints, hands = 21 keypoints, etc.
    /// These normalized points are the projection of the Object's 3D keypoint
    /// on the current frame's camera poses.
    #[prost(message, repeated, tag = "2")]
    pub keypoints: ::prost::alloc::vec::Vec<AnnotatedKeyPoint>,
    /// Visibiity of this annotation in a frame.
    #[prost(float, tag = "3")]
    pub visibility: f32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameAnnotation {
    /// Unique frame id, corresponds to images.
    #[prost(int32, tag = "1")]
    pub frame_id: i32,
    /// List of the annotated objects in this frame. Depending on how many object
    /// are observable in this frame, we might have non or as much as
    /// sequence.objects_size() annotations.
    #[prost(message, repeated, tag = "2")]
    pub annotations: ::prost::alloc::vec::Vec<ObjectAnnotation>,
    /// Information about the camera transformation (in the world coordinate) and
    /// imaging characteristics for a captured video frame.
    #[prost(message, optional, tag = "3")]
    pub camera: ::core::option::Option<ArCamera>,
    /// The timestamp for the frame.
    #[prost(double, tag = "4")]
    pub timestamp: f64,
    /// Plane center and normal in camera frame.
    #[prost(float, repeated, tag = "5")]
    pub plane_center: ::prost::alloc::vec::Vec<f32>,
    #[prost(float, repeated, tag = "6")]
    pub plane_normal: ::prost::alloc::vec::Vec<f32>,
}
/// The sequence protocol contains the annotation data for the entire video clip.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Sequence {
    /// List of all the annotated 3D objects in this sequence in the world
    /// Coordinate system. Given the camera poses of each frame (also in the
    /// world-coordinate) these objects bounding boxes can be projected to each
    /// frame to get the per-frame annotation (i.e. image_annotation below).
    #[prost(message, repeated, tag = "1")]
    pub objects: ::prost::alloc::vec::Vec<Object>,
    /// List of annotated data per each frame in sequence + frame information.
    #[prost(message, repeated, tag = "2")]
    pub frame_annotations: ::prost::alloc::vec::Vec<FrameAnnotation>,
}
