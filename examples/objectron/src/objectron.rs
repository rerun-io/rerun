/// Info about the camera characteristics used to capture images and depth data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AvCameraCalibrationData {
    /// 3x3 row-major matrix relating a camera's internal properties to an ideal
    /// pinhole-camera model.
    #[prost(float, repeated, tag = "1")]
    pub intrinsic_matrix: ::prost::alloc::vec::Vec<f32>,
    /// The image dimensions to which the intrinsic_matrix values are relative.
    #[prost(float, optional, tag = "2")]
    pub intrinsic_matrix_reference_dimension_width: ::core::option::Option<f32>,
    #[prost(float, optional, tag = "3")]
    pub intrinsic_matrix_reference_dimension_height: ::core::option::Option<f32>,
    /// 3x4 row-major matrix relating a camera's position and orientation to a
    /// world or scene coordinate system. Consists of a unitless 3x3 rotation
    /// matrix (R) on the left and a translation (t) 3x1 vector on the right. The
    /// translation vector's units are millimeters. For example:
    ///
    ///             |r1,1  r2,1  r3,1 | t1|
    ///   [R | t] = |r1,2  r2,2  r3,2 | t2|
    ///             |r1,3  r2,3  r3,3 | t3|
    ///
    ///   is stored as [r11, r21, r31, t1, r12, r22, r32, t2, ...]
    #[prost(float, repeated, tag = "4")]
    pub extrinsic_matrix: ::prost::alloc::vec::Vec<f32>,
    /// The size, in millimeters, of one image pixel.
    #[prost(float, optional, tag = "5")]
    pub pixel_size: ::core::option::Option<f32>,
    /// A list of floating-point values describing radial distortions imparted by
    /// the camera lens, for use in rectifying camera images.
    #[prost(float, repeated, tag = "6")]
    pub lens_distortion_lookup_values: ::prost::alloc::vec::Vec<f32>,
    /// A list of floating-point values describing radial distortions for use in
    /// reapplying camera geometry to a rectified image.
    #[prost(float, repeated, tag = "7")]
    pub inverse_lens_distortion_lookup_values: ::prost::alloc::vec::Vec<f32>,
    /// The offset of the distortion center of the camera lens from the top-left
    /// corner of the image.
    #[prost(float, optional, tag = "8")]
    pub lens_distortion_center_x: ::core::option::Option<f32>,
    #[prost(float, optional, tag = "9")]
    pub lens_distortion_center_y: ::core::option::Option<f32>,
}
/// Container for depth data information.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AvDepthData {
    /// PNG representation of the grayscale depth data map. See discussion about
    /// depth_data_map_original_minimum_value, below, for information about how
    /// to interpret the pixel values.
    #[prost(bytes = "vec", optional, tag = "1")]
    pub depth_data_map: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    /// Pixel format type of the original captured depth data.
    #[prost(string, optional, tag = "2")]
    pub depth_data_type: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(
        enumeration = "av_depth_data::Accuracy",
        optional,
        tag = "3",
        default = "Relative"
    )]
    pub depth_data_accuracy: ::core::option::Option<i32>,
    /// Indicates whether the depth_data_map contains temporally smoothed data.
    #[prost(bool, optional, tag = "4")]
    pub depth_data_filtered: ::core::option::Option<bool>,
    #[prost(enumeration = "av_depth_data::Quality", optional, tag = "5")]
    pub depth_data_quality: ::core::option::Option<i32>,
    /// Associated calibration data for the depth_data_map.
    #[prost(message, optional, tag = "6")]
    pub camera_calibration_data: ::core::option::Option<AvCameraCalibrationData>,
    /// The original range of values expressed by the depth_data_map, before
    /// grayscale normalization. For example, if the minimum and maximum values
    /// indicate a range of [0.5, 2.2], and the depth_data_type value indicates
    /// it was a depth map, then white pixels (255, 255, 255) will map to 0.5 and
    /// black pixels (0, 0, 0) will map to 2.2 with the grayscale range linearly
    /// interpolated inbetween. Conversely, if the depth_data_type value indicates
    /// it was a disparity map, then white pixels will map to 2.2 and black pixels
    /// will map to 0.5.
    #[prost(float, optional, tag = "7")]
    pub depth_data_map_original_minimum_value: ::core::option::Option<f32>,
    #[prost(float, optional, tag = "8")]
    pub depth_data_map_original_maximum_value: ::core::option::Option<f32>,
    /// The width of the depth buffer map.
    #[prost(int32, optional, tag = "9")]
    pub depth_data_map_width: ::core::option::Option<i32>,
    /// The height of the depth buffer map.
    #[prost(int32, optional, tag = "10")]
    pub depth_data_map_height: ::core::option::Option<i32>,
    /// The row-major flattened array of the depth buffer map pixels. This will be
    /// either a float32 or float16 byte array, depending on 'depth_data_type'.
    #[prost(bytes = "vec", optional, tag = "11")]
    pub depth_data_map_raw_values: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
/// Nested message and enum types in `AVDepthData`.
pub mod av_depth_data {
    /// Indicates the general accuracy of the depth_data_map.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Accuracy {
        UndefinedAccuracy = 0,
        /// Values in the depth map are usable for foreground/background separation
        /// but are not absolutely accurate in the physical world.
        Relative = 1,
        /// Values in the depth map are absolutely accurate in the physical world.
        Absolute = 2,
    }
    impl Accuracy {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Accuracy::UndefinedAccuracy => "UNDEFINED_ACCURACY",
                Accuracy::Relative => "RELATIVE",
                Accuracy::Absolute => "ABSOLUTE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED_ACCURACY" => Some(Self::UndefinedAccuracy),
                "RELATIVE" => Some(Self::Relative),
                "ABSOLUTE" => Some(Self::Absolute),
                _ => None,
            }
        }
    }
    /// Quality of the depth_data_map.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Quality {
        UndefinedQuality = 0,
        High = 1,
        Low = 2,
    }
    impl Quality {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Quality::UndefinedQuality => "UNDEFINED_QUALITY",
                Quality::High => "HIGH",
                Quality::Low => "LOW",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED_QUALITY" => Some(Self::UndefinedQuality),
                "HIGH" => Some(Self::High),
                "LOW" => Some(Self::Low),
                _ => None,
            }
        }
    }
}
/// Estimated scene lighting information associated with a captured video frame.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArLightEstimate {
    /// The estimated intensity, in lumens, of ambient light throughout the scene.
    #[prost(double, optional, tag = "1")]
    pub ambient_intensity: ::core::option::Option<f64>,
    /// The estimated color temperature, in degrees Kelvin, of ambient light
    /// throughout the scene.
    #[prost(double, optional, tag = "2")]
    pub ambient_color_temperature: ::core::option::Option<f64>,
    /// Data describing the estimated lighting environment in all directions.
    /// Second-level spherical harmonics in separate red, green, and blue data
    /// planes. Thus, this buffer contains 3 sets of 9 coefficients, or a total of
    /// 27 values.
    #[prost(float, repeated, tag = "3")]
    pub spherical_harmonics_coefficients: ::prost::alloc::vec::Vec<f32>,
    /// A vector indicating the orientation of the strongest directional light
    /// source, normalized in the world-coordinate space.
    #[prost(message, optional, tag = "4")]
    pub primary_light_direction: ::core::option::Option<
        ar_light_estimate::DirectionVector,
    >,
    /// The estimated intensity, in lumens, of the strongest directional light
    /// source in the scene.
    #[prost(float, optional, tag = "5")]
    pub primary_light_intensity: ::core::option::Option<f32>,
}
/// Nested message and enum types in `ARLightEstimate`.
pub mod ar_light_estimate {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct DirectionVector {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
}
/// Information about the camera position and imaging characteristics for a
/// captured video frame.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArCamera {
    #[prost(
        enumeration = "ar_camera::TrackingState",
        optional,
        tag = "1",
        default = "Unavailable"
    )]
    pub tracking_state: ::core::option::Option<i32>,
    #[prost(
        enumeration = "ar_camera::TrackingStateReason",
        optional,
        tag = "2",
        default = "None"
    )]
    pub tracking_state_reason: ::core::option::Option<i32>,
    /// 4x4 row-major matrix expressing position and orientation of the camera in
    /// world coordinate space.
    #[prost(float, repeated, tag = "3")]
    pub transform: ::prost::alloc::vec::Vec<f32>,
    #[prost(message, optional, tag = "4")]
    pub euler_angles: ::core::option::Option<ar_camera::EulerAngles>,
    /// The width and height, in pixels, of the captured camera image.
    #[prost(int32, optional, tag = "5")]
    pub image_resolution_width: ::core::option::Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub image_resolution_height: ::core::option::Option<i32>,
    /// 3x3 row-major matrix that converts between the 2D camera plane and 3D world
    /// coordinate space.
    #[prost(float, repeated, tag = "7")]
    pub intrinsics: ::prost::alloc::vec::Vec<f32>,
    /// 4x4 row-major transform matrix appropriate for rendering 3D content to
    /// match the image captured by the camera.
    #[prost(float, repeated, tag = "8")]
    pub projection_matrix: ::prost::alloc::vec::Vec<f32>,
    /// 4x4 row-major transform matrix appropriate for converting from world-space
    /// to camera space. Relativized for the captured_image orientation (i.e.
    /// UILandscapeOrientationRight).
    #[prost(float, repeated, tag = "9")]
    pub view_matrix: ::prost::alloc::vec::Vec<f32>,
}
/// Nested message and enum types in `ARCamera`.
pub mod ar_camera {
    /// The orientation of the camera, expressed as roll, pitch, and yaw values.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct EulerAngles {
        #[prost(float, optional, tag = "1")]
        pub roll: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub pitch: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub yaw: ::core::option::Option<f32>,
    }
    /// The general quality of position tracking available when the camera captured
    /// a frame.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum TrackingState {
        UndefinedTrackingState = 0,
        /// Camera position tracking is not available.
        Unavailable = 1,
        /// Tracking is available, but the quality of results is questionable.
        Limited = 2,
        /// Camera position tracking is providing optimal results.
        Normal = 3,
    }
    impl TrackingState {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                TrackingState::UndefinedTrackingState => "UNDEFINED_TRACKING_STATE",
                TrackingState::Unavailable => "UNAVAILABLE",
                TrackingState::Limited => "LIMITED",
                TrackingState::Normal => "NORMAL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED_TRACKING_STATE" => Some(Self::UndefinedTrackingState),
                "UNAVAILABLE" => Some(Self::Unavailable),
                "LIMITED" => Some(Self::Limited),
                "NORMAL" => Some(Self::Normal),
                _ => None,
            }
        }
    }
    /// A possible diagnosis for limited position tracking quality as of when the
    /// frame was captured.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum TrackingStateReason {
        UndefinedTrackingStateReason = 0,
        /// The current tracking state is not limited.
        None = 1,
        /// Not yet enough camera or motion data to provide tracking information.
        Initializing = 2,
        /// The device is moving too fast for accurate image-based position tracking.
        ExcessiveMotion = 3,
        /// Not enough distinguishable features for image-based position tracking.
        InsufficientFeatures = 4,
        /// Tracking is limited due to a relocalization in progress.
        Relocalizing = 5,
    }
    impl TrackingStateReason {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                TrackingStateReason::UndefinedTrackingStateReason => {
                    "UNDEFINED_TRACKING_STATE_REASON"
                }
                TrackingStateReason::None => "NONE",
                TrackingStateReason::Initializing => "INITIALIZING",
                TrackingStateReason::ExcessiveMotion => "EXCESSIVE_MOTION",
                TrackingStateReason::InsufficientFeatures => "INSUFFICIENT_FEATURES",
                TrackingStateReason::Relocalizing => "RELOCALIZING",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED_TRACKING_STATE_REASON" => {
                    Some(Self::UndefinedTrackingStateReason)
                }
                "NONE" => Some(Self::None),
                "INITIALIZING" => Some(Self::Initializing),
                "EXCESSIVE_MOTION" => Some(Self::ExcessiveMotion),
                "INSUFFICIENT_FEATURES" => Some(Self::InsufficientFeatures),
                "RELOCALIZING" => Some(Self::Relocalizing),
                _ => None,
            }
        }
    }
}
/// Container for a 3D mesh describing face topology.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArFaceGeometry {
    #[prost(message, repeated, tag = "1")]
    pub vertices: ::prost::alloc::vec::Vec<ar_face_geometry::Vertex>,
    /// The number of elements in the vertices list.
    #[prost(int32, optional, tag = "2")]
    pub vertex_count: ::core::option::Option<i32>,
    #[prost(message, repeated, tag = "3")]
    pub texture_coordinates: ::prost::alloc::vec::Vec<
        ar_face_geometry::TextureCoordinate,
    >,
    /// The number of elements in the texture_coordinates list.
    #[prost(int32, optional, tag = "4")]
    pub texture_coordinate_count: ::core::option::Option<i32>,
    /// Each integer value in this ordered list represents an index into the
    /// vertices and texture_coordinates lists. Each set of three indices
    /// identifies the vertices comprising a single triangle in the mesh. Each set
    /// of three indices forms a triangle, so the number of indices in the
    /// triangle_indices buffer is three times the triangle_count value.
    #[prost(int32, repeated, tag = "5")]
    pub triangle_indices: ::prost::alloc::vec::Vec<i32>,
    /// The number of triangles described by the triangle_indices buffer.
    #[prost(int32, optional, tag = "6")]
    pub triangle_count: ::core::option::Option<i32>,
}
/// Nested message and enum types in `ARFaceGeometry`.
pub mod ar_face_geometry {
    /// Each vertex represents a 3D point in the face mesh, in the face coordinate
    /// space.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Vertex {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    /// Each texture coordinate represents UV texture coordinates for the vertex at
    /// the corresponding index in the vertices buffer.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TextureCoordinate {
        #[prost(float, optional, tag = "1")]
        pub u: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub v: ::core::option::Option<f32>,
    }
}
/// Contains a list of blend shape entries wherein each item maps a specific
/// blend shape location to its associated coefficient.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArBlendShapeMap {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<ar_blend_shape_map::MapEntry>,
}
/// Nested message and enum types in `ARBlendShapeMap`.
pub mod ar_blend_shape_map {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct MapEntry {
        /// Identifier for the specific facial feature.
        #[prost(string, optional, tag = "1")]
        pub blend_shape_location: ::core::option::Option<::prost::alloc::string::String>,
        /// Indicates the current position of the feature relative to its neutral
        /// configuration, ranging from 0.0 (neutral) to 1.0 (maximum movement).
        #[prost(float, optional, tag = "2")]
        pub blend_shape_coefficient: ::core::option::Option<f32>,
    }
}
/// Information about the pose, topology, and expression of a detected face.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArFaceAnchor {
    /// A coarse triangle mesh representing the topology of the detected face.
    #[prost(message, optional, tag = "1")]
    pub geometry: ::core::option::Option<ArFaceGeometry>,
    /// A map of named coefficients representing the detected facial expression in
    /// terms of the movement of specific facial features.
    #[prost(message, optional, tag = "2")]
    pub blend_shapes: ::core::option::Option<ArBlendShapeMap>,
    /// 4x4 row-major matrix encoding the position, orientation, and scale of the
    /// anchor relative to the world coordinate space.
    #[prost(float, repeated, packed = "false", tag = "3")]
    pub transform: ::prost::alloc::vec::Vec<f32>,
    /// Indicates whether the anchor's transform is valid. Frames that have a face
    /// anchor with this value set to NO should probably be ignored.
    #[prost(bool, optional, tag = "4")]
    pub is_tracked: ::core::option::Option<bool>,
}
/// Container for a 3D mesh.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArPlaneGeometry {
    /// A buffer of vertex positions for each point in the plane mesh.
    #[prost(message, repeated, tag = "1")]
    pub vertices: ::prost::alloc::vec::Vec<ar_plane_geometry::Vertex>,
    /// The number of elements in the vertices buffer.
    #[prost(int32, optional, tag = "2")]
    pub vertex_count: ::core::option::Option<i32>,
    /// A buffer of texture coordinate values for each point in the plane mesh.
    #[prost(message, repeated, tag = "3")]
    pub texture_coordinates: ::prost::alloc::vec::Vec<
        ar_plane_geometry::TextureCoordinate,
    >,
    /// The number of elements in the texture_coordinates buffer.
    #[prost(int32, optional, tag = "4")]
    pub texture_coordinate_count: ::core::option::Option<i32>,
    /// Each integer value in this ordered list represents an index into the
    /// vertices and texture_coordinates lists. Each set of three indices
    /// identifies the vertices comprising a single triangle in the mesh. Each set
    /// of three indices forms a triangle, so the number of indices in the
    /// triangle_indices buffer is three times the triangle_count value.
    #[prost(int32, repeated, tag = "5")]
    pub triangle_indices: ::prost::alloc::vec::Vec<i32>,
    /// Each set of three indices forms a triangle, so the number of indices in the
    /// triangle_indices buffer is three times the triangle_count value.
    #[prost(int32, optional, tag = "6")]
    pub triangle_count: ::core::option::Option<i32>,
    /// Each value in this buffer represents the position of a vertex along the
    /// boundary polygon of the estimated plane. The owning plane anchor's
    /// transform matrix defines the coordinate system for these points.
    #[prost(message, repeated, tag = "7")]
    pub boundary_vertices: ::prost::alloc::vec::Vec<ar_plane_geometry::Vertex>,
    /// The number of elements in the boundary_vertices buffer.
    #[prost(int32, optional, tag = "8")]
    pub boundary_vertex_count: ::core::option::Option<i32>,
}
/// Nested message and enum types in `ARPlaneGeometry`.
pub mod ar_plane_geometry {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Vertex {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    /// Each texture coordinate represents UV texture coordinates for the vertex at
    /// the corresponding index in the vertices buffer.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TextureCoordinate {
        #[prost(float, optional, tag = "1")]
        pub u: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub v: ::core::option::Option<f32>,
    }
}
/// Information about the position and orientation of a real-world flat surface.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArPlaneAnchor {
    /// The ID of the plane.
    #[prost(string, optional, tag = "1")]
    pub identifier: ::core::option::Option<::prost::alloc::string::String>,
    /// 4x4 row-major matrix encoding the position, orientation, and scale of the
    /// anchor relative to the world coordinate space.
    #[prost(float, repeated, packed = "false", tag = "2")]
    pub transform: ::prost::alloc::vec::Vec<f32>,
    /// The general orientation of the detected plane with respect to gravity.
    #[prost(enumeration = "ar_plane_anchor::Alignment", optional, tag = "3")]
    pub alignment: ::core::option::Option<i32>,
    /// A coarse triangle mesh representing the general shape of the detected
    /// plane.
    #[prost(message, optional, tag = "4")]
    pub geometry: ::core::option::Option<ArPlaneGeometry>,
    /// The center point of the plane relative to its anchor position.
    /// Although the type of this property is a 3D vector, a plane anchor is always
    /// two-dimensional, and is always positioned in only the x and z directions
    /// relative to its transform position. (That is, the y-component of this
    /// vector is always zero.)
    #[prost(message, optional, tag = "5")]
    pub center: ::core::option::Option<ar_plane_anchor::PlaneVector>,
    /// The estimated width and length of the detected plane.
    #[prost(message, optional, tag = "6")]
    pub extent: ::core::option::Option<ar_plane_anchor::PlaneVector>,
    /// A Boolean value that indicates whether plane classification is available on
    /// the current device. On devices without plane classification support, all
    /// plane anchors report a classification value of NONE
    /// and a classification_status value of UNAVAILABLE.
    #[prost(bool, optional, tag = "7")]
    pub classification_supported: ::core::option::Option<bool>,
    /// A general characterization of what kind of real-world surface the plane
    /// anchor represents.
    #[prost(enumeration = "ar_plane_anchor::PlaneClassification", optional, tag = "8")]
    pub classification: ::core::option::Option<i32>,
    /// The current state of process for classifying the plane anchor.
    /// When this property's value is KNOWN, the classification property represents
    /// characterization of the real-world surface corresponding to the
    /// plane anchor.
    #[prost(
        enumeration = "ar_plane_anchor::PlaneClassificationStatus",
        optional,
        tag = "9"
    )]
    pub classification_status: ::core::option::Option<i32>,
}
/// Nested message and enum types in `ARPlaneAnchor`.
pub mod ar_plane_anchor {
    /// Wrapper for a 3D point / vector within the plane. See extent and center
    /// values for more information.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct PlaneVector {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Alignment {
        Undefined = 0,
        /// The plane is perpendicular to gravity.
        Horizontal = 1,
        /// The plane is parallel to gravity.
        Vertical = 2,
    }
    impl Alignment {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Alignment::Undefined => "UNDEFINED",
                Alignment::Horizontal => "HORIZONTAL",
                Alignment::Vertical => "VERTICAL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED" => Some(Self::Undefined),
                "HORIZONTAL" => Some(Self::Horizontal),
                "VERTICAL" => Some(Self::Vertical),
                _ => None,
            }
        }
    }
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum PlaneClassification {
        None = 0,
        Wall = 1,
        Floor = 2,
        Ceiling = 3,
        Table = 4,
        Seat = 5,
    }
    impl PlaneClassification {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                PlaneClassification::None => "NONE",
                PlaneClassification::Wall => "WALL",
                PlaneClassification::Floor => "FLOOR",
                PlaneClassification::Ceiling => "CEILING",
                PlaneClassification::Table => "TABLE",
                PlaneClassification::Seat => "SEAT",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "NONE" => Some(Self::None),
                "WALL" => Some(Self::Wall),
                "FLOOR" => Some(Self::Floor),
                "CEILING" => Some(Self::Ceiling),
                "TABLE" => Some(Self::Table),
                "SEAT" => Some(Self::Seat),
                _ => None,
            }
        }
    }
    /// The classification status for the plane.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum PlaneClassificationStatus {
        /// The classfication process for the plane anchor has completed but the
        /// result is inconclusive.
        Unknown = 0,
        /// No classication information can be provided (set on error or if the
        /// device does not support plane classification).
        Unavailable = 1,
        /// The classification process has not completed.
        Undetermined = 2,
        /// The classfication process for the plane anchor has completed.
        Known = 3,
    }
    impl PlaneClassificationStatus {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                PlaneClassificationStatus::Unknown => "UNKNOWN",
                PlaneClassificationStatus::Unavailable => "UNAVAILABLE",
                PlaneClassificationStatus::Undetermined => "UNDETERMINED",
                PlaneClassificationStatus::Known => "KNOWN",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "UNAVAILABLE" => Some(Self::Unavailable),
                "UNDETERMINED" => Some(Self::Undetermined),
                "KNOWN" => Some(Self::Known),
                _ => None,
            }
        }
    }
}
/// A collection of points in the world coordinate space.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArPointCloud {
    /// The number of points in the cloud.
    #[prost(int32, optional, tag = "1")]
    pub count: ::core::option::Option<i32>,
    /// The list of detected points.
    #[prost(message, repeated, tag = "2")]
    pub point: ::prost::alloc::vec::Vec<ar_point_cloud::Point>,
    /// A list of unique identifiers corresponding to detected feature points.
    /// Each identifier in this list corresponds to the point at the same index
    /// in the points array.
    #[prost(int64, repeated, tag = "3")]
    pub identifier: ::prost::alloc::vec::Vec<i64>,
}
/// Nested message and enum types in `ARPointCloud`.
pub mod ar_point_cloud {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Point {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
}
/// A 3D vector
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmVector {
    #[prost(double, optional, tag = "1")]
    pub x: ::core::option::Option<f64>,
    #[prost(double, optional, tag = "2")]
    pub y: ::core::option::Option<f64>,
    #[prost(double, optional, tag = "3")]
    pub z: ::core::option::Option<f64>,
}
/// Represents calibrated magnetic field data and accuracy estimate of it.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmCalibratedMagneticField {
    /// Vector of magnetic field estimate.
    #[prost(message, optional, tag = "1")]
    pub field: ::core::option::Option<CmVector>,
    /// Calibration accuracy of a magnetic field estimate.
    #[prost(
        enumeration = "cm_calibrated_magnetic_field::CalibrationAccuracy",
        optional,
        tag = "2"
    )]
    pub calibration_accuracy: ::core::option::Option<i32>,
}
/// Nested message and enum types in `CMCalibratedMagneticField`.
pub mod cm_calibrated_magnetic_field {
    /// Indicates the calibration accuracy of a magnetic field estimate.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum CalibrationAccuracy {
        Uncalibrated = 0,
        Low = 1,
        Medium = 2,
        High = 3,
    }
    impl CalibrationAccuracy {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                CalibrationAccuracy::Uncalibrated => "UNCALIBRATED",
                CalibrationAccuracy::Low => "LOW",
                CalibrationAccuracy::Medium => "MEDIUM",
                CalibrationAccuracy::High => "HIGH",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNCALIBRATED" => Some(Self::Uncalibrated),
                "LOW" => Some(Self::Low),
                "MEDIUM" => Some(Self::Medium),
                "HIGH" => Some(Self::High),
                _ => None,
            }
        }
    }
}
/// A sample of device motion data.
/// Encapsulates measurements of the attitude, rotation rate, magnetic field, and
/// acceleration of the device. Core Media applies different algorithms of
/// bias-reduction and stabilization to rotation rate, magnetic field and
/// acceleration values. For raw values check correspondent fields in
/// CMMotionManagerSnapshot object.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmDeviceMotion {
    /// The device motion data object timestamp. May differ from the frame
    /// timestamp value since the data may be collected at higher rate.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// The quaternion representing the device’s orientation relative to a known
    /// frame of reference at a point in time.
    #[prost(message, optional, tag = "2")]
    pub attitude_quaternion: ::core::option::Option<cm_device_motion::Quaternion>,
    /// The gravity acceleration vector expressed in the device's reference frame.
    #[prost(message, optional, tag = "3")]
    pub gravity: ::core::option::Option<CmVector>,
    /// The acceleration that the user is giving to the device.
    #[prost(message, optional, tag = "4")]
    pub user_acceleration: ::core::option::Option<CmVector>,
    /// Returns the magnetic field vector filtered with respect to the device bias.
    #[prost(message, optional, tag = "5")]
    pub magnetic_field: ::core::option::Option<CmCalibratedMagneticField>,
    /// The rotation rate of the device adjusted by bias-removing Core Motion
    /// algoriths.
    #[prost(message, optional, tag = "6")]
    pub rotation_rate: ::core::option::Option<CmVector>,
}
/// Nested message and enum types in `CMDeviceMotion`.
pub mod cm_device_motion {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Quaternion {
        #[prost(double, optional, tag = "1")]
        pub x: ::core::option::Option<f64>,
        #[prost(double, optional, tag = "2")]
        pub y: ::core::option::Option<f64>,
        #[prost(double, optional, tag = "3")]
        pub z: ::core::option::Option<f64>,
        #[prost(double, optional, tag = "4")]
        pub w: ::core::option::Option<f64>,
    }
}
/// A sample of raw accelerometer data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmAccelerometerData {
    /// The accelerometer data object timestamp. May differ from the frame
    /// timestamp value since the data may be collected at higher rate.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// Raw acceleration measured by the accelerometer which effectively is a sum
    /// of gravity and user_acceleration of CMDeviceMotion object.
    #[prost(message, optional, tag = "2")]
    pub acceleration: ::core::option::Option<CmVector>,
}
/// A sample of raw gyroscope data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmGyroData {
    /// The gyroscope data object timestamp. May differ from the frame
    /// timestamp value since the data may be collected at higher rate.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// Raw rotation rate as measured by the gyroscope.
    #[prost(message, optional, tag = "2")]
    pub rotation_rate: ::core::option::Option<CmVector>,
}
/// A sample of raw magnetometer data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmMagnetometerData {
    /// The magnetometer data object timestamp. May differ from the frame
    /// timestamp value since the data may be collected at higher rate.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// Raw magnetic field measured by the magnetometer.
    #[prost(message, optional, tag = "2")]
    pub magnetic_field: ::core::option::Option<CmVector>,
}
/// Contains most recent snapshots of device motion data
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CmMotionManagerSnapshot {
    /// Most recent samples of device motion data.
    #[prost(message, repeated, tag = "1")]
    pub device_motion: ::prost::alloc::vec::Vec<CmDeviceMotion>,
    /// Most recent samples of raw accelerometer data.
    #[prost(message, repeated, tag = "2")]
    pub accelerometer_data: ::prost::alloc::vec::Vec<CmAccelerometerData>,
    /// Most recent samples of raw gyroscope data.
    #[prost(message, repeated, tag = "3")]
    pub gyro_data: ::prost::alloc::vec::Vec<CmGyroData>,
    /// Most recent samples of raw magnetometer data.
    #[prost(message, repeated, tag = "4")]
    pub magnetometer_data: ::prost::alloc::vec::Vec<CmMagnetometerData>,
}
/// Video image and face position tracking information.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArFrame {
    /// The timestamp for the frame.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// The depth data associated with the frame. Not all frames have depth data.
    #[prost(message, optional, tag = "2")]
    pub depth_data: ::core::option::Option<AvDepthData>,
    /// The depth data object timestamp associated with the frame. May differ from
    /// the frame timestamp value. Is only set when the frame has depth_data.
    #[prost(double, optional, tag = "3")]
    pub depth_data_timestamp: ::core::option::Option<f64>,
    /// Camera information associated with the frame.
    #[prost(message, optional, tag = "4")]
    pub camera: ::core::option::Option<ArCamera>,
    /// Light information associated with the frame.
    #[prost(message, optional, tag = "5")]
    pub light_estimate: ::core::option::Option<ArLightEstimate>,
    /// Face anchor information associated with the frame. Not all frames have an
    /// active face anchor.
    #[prost(message, optional, tag = "6")]
    pub face_anchor: ::core::option::Option<ArFaceAnchor>,
    /// Plane anchors associated with the frame. Not all frames have a plane
    /// anchor. Plane anchors and face anchors are mutually exclusive.
    #[prost(message, repeated, tag = "7")]
    pub plane_anchor: ::prost::alloc::vec::Vec<ArPlaneAnchor>,
    /// The current intermediate results of the scene analysis used to perform
    /// world tracking.
    #[prost(message, optional, tag = "8")]
    pub raw_feature_points: ::core::option::Option<ArPointCloud>,
    /// Snapshot of Core Motion CMMotionManager object containing most recent
    /// motion data associated with the frame. Since motion data capture rates can
    /// be higher than rates of AR capture, the entities of this object reflect all
    /// of the aggregated events which have occurred since the last ARFrame was
    /// recorded.
    #[prost(message, optional, tag = "9")]
    pub motion_manager_snapshot: ::core::option::Option<CmMotionManagerSnapshot>,
}
/// Mesh geometry data stored in an array-based format.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArMeshGeometry {
    /// The vertices of the mesh.
    #[prost(message, repeated, tag = "1")]
    pub vertices: ::prost::alloc::vec::Vec<ar_mesh_geometry::Vertex>,
    /// The faces of the mesh.
    #[prost(message, repeated, tag = "2")]
    pub faces: ::prost::alloc::vec::Vec<ar_mesh_geometry::Face>,
    /// Rays that define which direction is outside for each face.
    /// Normals contain 'rays that define which direction is outside for each
    /// face', in practice the normals count is always identical to vertices count
    /// which looks like vertices normals and not faces normals.
    #[prost(message, repeated, tag = "3")]
    pub normals: ::prost::alloc::vec::Vec<ar_mesh_geometry::Vertex>,
    /// Classification for each face in the mesh.
    #[prost(
        enumeration = "ar_mesh_geometry::MeshClassification",
        repeated,
        packed = "false",
        tag = "4"
    )]
    pub classification: ::prost::alloc::vec::Vec<i32>,
}
/// Nested message and enum types in `ARMeshGeometry`.
pub mod ar_mesh_geometry {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Vertex {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Face {
        /// / Indices of vertices defining the face from correspondent array of parent
        /// / message. A typical face is triangular.
        #[prost(int32, repeated, tag = "1")]
        pub vertex_indices: ::prost::alloc::vec::Vec<i32>,
    }
    /// Type of objects
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum MeshClassification {
        None = 0,
        Wall = 1,
        Floor = 2,
        Ceiling = 3,
        Table = 4,
        Seat = 5,
        Window = 6,
        Door = 7,
    }
    impl MeshClassification {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                MeshClassification::None => "NONE",
                MeshClassification::Wall => "WALL",
                MeshClassification::Floor => "FLOOR",
                MeshClassification::Ceiling => "CEILING",
                MeshClassification::Table => "TABLE",
                MeshClassification::Seat => "SEAT",
                MeshClassification::Window => "WINDOW",
                MeshClassification::Door => "DOOR",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "NONE" => Some(Self::None),
                "WALL" => Some(Self::Wall),
                "FLOOR" => Some(Self::Floor),
                "CEILING" => Some(Self::Ceiling),
                "TABLE" => Some(Self::Table),
                "SEAT" => Some(Self::Seat),
                "WINDOW" => Some(Self::Window),
                "DOOR" => Some(Self::Door),
                _ => None,
            }
        }
    }
}
/// A subdividision of the reconstructed, real-world scene surrounding the user.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArMeshAnchor {
    /// The ID of the mesh.
    #[prost(string, optional, tag = "1")]
    pub identifier: ::core::option::Option<::prost::alloc::string::String>,
    /// 4x4 row-major matrix encoding the position, orientation, and scale of the
    /// anchor relative to the world coordinate space.
    #[prost(float, repeated, tag = "2")]
    pub transform: ::prost::alloc::vec::Vec<f32>,
    /// 3D information about the mesh such as its shape and classifications.
    #[prost(message, optional, tag = "3")]
    pub geometry: ::core::option::Option<ArMeshGeometry>,
}
/// Container object for mesh data of real-world scene surrounding the user.
/// Even though each ARFrame may have a set of ARMeshAnchors associated with it,
/// only a single frame's worth of mesh data is written separately at the end of
/// each recording due to concerns regarding latency and memory bloat.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArMeshData {
    /// The timestamp for the data.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// Set of mesh anchors containing the mesh data.
    #[prost(message, repeated, tag = "2")]
    pub mesh_anchor: ::prost::alloc::vec::Vec<ArMeshAnchor>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
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
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Type {
        UndefinedType = 0,
        BoundingBox = 1,
        Skeleton = 2,
        Mesh = 3,
    }
    impl Type {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Type::UndefinedType => "UNDEFINED_TYPE",
                Type::BoundingBox => "BOUNDING_BOX",
                Type::Skeleton => "SKELETON",
                Type::Mesh => "MESH",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNDEFINED_TYPE" => Some(Self::UndefinedType),
                "BOUNDING_BOX" => Some(Self::BoundingBox),
                "SKELETON" => Some(Self::Skeleton),
                "MESH" => Some(Self::Mesh),
                _ => None,
            }
        }
    }
    /// Enum to reflect how this object is created.
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Method {
        UnknownMethod = 0,
        /// Created by data annotation.
        Annotation = 1,
        /// Created by data augmentation.
        Augmentation = 2,
    }
    impl Method {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Method::UnknownMethod => "UNKNOWN_METHOD",
                Method::Annotation => "ANNOTATION",
                Method::Augmentation => "AUGMENTATION",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN_METHOD" => Some(Self::UnknownMethod),
                "ANNOTATION" => Some(Self::Annotation),
                "AUGMENTATION" => Some(Self::Augmentation),
                _ => None,
            }
        }
    }
}
/// The edge connecting two keypoints together
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Skeletons {
    #[prost(message, repeated, tag = "1")]
    pub object: ::prost::alloc::vec::Vec<Skeleton>,
}
/// Projection of a 3D point on an image, and its metric depth.
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Point3D {
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    #[prost(float, tag = "3")]
    pub z: f32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnnotatedKeyPoint {
    #[prost(int32, tag = "1")]
    pub id: i32,
    #[prost(message, optional, tag = "2")]
    pub point_3d: ::core::option::Option<Point3D>,
    #[prost(message, optional, tag = "3")]
    pub point_2d: ::core::option::Option<NormalizedPoint2D>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
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
#[allow(clippy::derive_partial_eq_without_eq)]
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
