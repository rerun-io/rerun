/// Info about the camera characteristics used to capture images and depth data.
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
    /// ```text
    ///            |r1,1  r2,1  r3,1 | t1|
    ///  [R | t] = |r1,2  r2,2  r3,2 | t2|
    ///            |r1,3  r2,3  r3,3 | t3|
    /// ```
    ///
    ///  is stored as [r11, r21, r31, t1, r12, r22, r32, t2, ...]
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Accuracy {
        UndefinedAccuracy = 0,
        /// Values in the depth map are usable for foreground/background separation
        /// but are not absolutely accurate in the physical world.
        Relative = 1,
        /// Values in the depth map are absolutely accurate in the physical world.
        Absolute = 2,
    }
    /// Quality of the depth_data_map.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Quality {
        UndefinedQuality = 0,
        High = 1,
        Low = 2,
    }
}
/// Estimated scene lighting information associated with a captured video frame.
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
    pub primary_light_direction: ::core::option::Option<ar_light_estimate::DirectionVector>,
    /// The estimated intensity, in lumens, of the strongest directional light
    /// source in the scene.
    #[prost(float, optional, tag = "5")]
    pub primary_light_intensity: ::core::option::Option<f32>,
}
/// Nested message and enum types in `ARLightEstimate`.
pub mod ar_light_estimate {
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
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
    /// A possible diagnosis for limited position tracking quality as of when the
    /// frame was captured.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
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
}
/// Container for a 3D mesh describing face topology.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArFaceGeometry {
    #[prost(message, repeated, tag = "1")]
    pub vertices: ::prost::alloc::vec::Vec<ar_face_geometry::Vertex>,
    /// The number of elements in the vertices list.
    #[prost(int32, optional, tag = "2")]
    pub vertex_count: ::core::option::Option<i32>,
    #[prost(message, repeated, tag = "3")]
    pub texture_coordinates: ::prost::alloc::vec::Vec<ar_face_geometry::TextureCoordinate>,
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
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArBlendShapeMap {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<ar_blend_shape_map::MapEntry>,
}
/// Nested message and enum types in `ARBlendShapeMap`.
pub mod ar_blend_shape_map {
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
    pub texture_coordinates: ::prost::alloc::vec::Vec<ar_plane_geometry::TextureCoordinate>,
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
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TextureCoordinate {
        #[prost(float, optional, tag = "1")]
        pub u: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub v: ::core::option::Option<f32>,
    }
}
/// Information about the position and orientation of a real-world flat surface.
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
    #[prost(
        enumeration = "ar_plane_anchor::PlaneClassification",
        optional,
        tag = "8"
    )]
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
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct PlaneVector {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Alignment {
        Undefined = 0,
        /// The plane is perpendicular to gravity.
        Horizontal = 1,
        /// The plane is parallel to gravity.
        Vertical = 2,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum PlaneClassification {
        None = 0,
        Wall = 1,
        Floor = 2,
        Ceiling = 3,
        Table = 4,
        Seat = 5,
    }
    /// The classification status for the plane.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
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
}
/// A collection of points in the world coordinate space.
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum CalibrationAccuracy {
        Uncalibrated = 0,
        Low = 1,
        Medium = 2,
        High = 3,
    }
}
/// A sample of device motion data.
/// Encapsulates measurements of the attitude, rotation rate, magnetic field, and
/// acceleration of the device. Core Media applies different algorithms of
/// bias-reduction and stabilization to rotation rate, magnetic field and
/// acceleration values. For raw values check correspondent fields in
/// CMMotionManagerSnapshot object.
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
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Vertex {
        #[prost(float, optional, tag = "1")]
        pub x: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "2")]
        pub y: ::core::option::Option<f32>,
        #[prost(float, optional, tag = "3")]
        pub z: ::core::option::Option<f32>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Face {
        //// Indices of vertices defining the face from correspondent array of parent
        //// message. A typical face is triangular.
        #[prost(int32, repeated, tag = "1")]
        pub vertex_indices: ::prost::alloc::vec::Vec<i32>,
    }
    /// Type of objects
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
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
}
/// A subdividision of the reconstructed, real-world scene surrounding the user.
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
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArMeshData {
    /// The timestamp for the data.
    #[prost(double, optional, tag = "1")]
    pub timestamp: ::core::option::Option<f64>,
    /// Set of mesh anchors containing the mesh data.
    #[prost(message, repeated, tag = "2")]
    pub mesh_anchor: ::prost::alloc::vec::Vec<ArMeshAnchor>,
}
