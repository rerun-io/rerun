// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/pinhole.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/image_plane_distance.hpp"
#include "../components/pinhole_projection.hpp"
#include "../components/resolution.hpp"
#include "../components/view_coordinates.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cmath>
#include <cstdint>
#include <limits>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Camera perspective projection (a.k.a. intrinsics).
    ///
    /// ## Examples
    ///
    /// ### Simple pinhole camera
    /// ![image](https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm> // std::generate
    /// #include <cstdlib>   // std::rand
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_pinhole");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log("world/image", rerun::Pinhole::from_focal_length_and_resolution(3.0f, {3.0f, 3.0f}));
    ///
    ///     std::vector<uint8_t> random_data(3 * 3 * 3);
    ///     std::generate(random_data.begin(), random_data.end(), [] {
    ///         return static_cast<uint8_t>(std::rand());
    ///     });
    ///
    ///     rec.log("world/image", rerun::Image::from_rgb24(random_data, {3, 3}));
    /// }
    /// ```
    ///
    /// ### Perspective pinhole camera
    /// ![image](https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_pinhole_perspective");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     const float fov_y = 0.7853982f;
    ///     const float aspect_ratio = 1.7777778f;
    ///     rec.log(
    ///         "world/cam",
    ///         rerun::Pinhole::from_fov_and_aspect_ratio(fov_y, aspect_ratio)
    ///             .with_camera_xyz(rerun::components::ViewCoordinates::RUB)
    ///             .with_image_plane_distance(0.1f)
    ///     );
    ///
    ///     rec.log(
    ///         "world/points",
    ///         rerun::Points3D({{0.0f, 0.0f, -0.5f}, {0.1f, 0.1f, -0.5f}, {-0.1f, -0.1f, -0.5f}}
    ///         ).with_radii({0.025f})
    ///     );
    /// }
    /// ```
    struct Pinhole {
        /// Camera projection, from image coordinates to view coordinates.
        std::optional<ComponentBatch> image_from_camera;

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// Example:
        /// ```text
        /// [1920.0, 1440.0]
        /// ```
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        std::optional<ComponentBatch> resolution;

        /// Sets the view coordinates for the camera.
        ///
        /// All common values are available as constants on the `components::ViewCoordinates` class.
        ///
        /// The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
        /// This means that the camera frustum will point along the positive Z axis of the parent space,
        /// and the cameras "up" direction will be along the negative Y axis of the parent space.
        ///
        /// The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
        /// When logging a depth image under this entity, this is the direction the point cloud will be projected.
        /// With `RDF`, the default forward is +Z.
        ///
        /// The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
        /// This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
        /// With `RDF`, the default is up is -Y.
        ///
        /// The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
        /// This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
        /// With `RDF`, the default right is +x.
        ///
        /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).
        ///
        /// NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
        /// and make the pinhole matrix not match up with the coordinate system of the pinhole entity.
        ///
        /// The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
        /// but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
        std::optional<ComponentBatch> camera_xyz;

        /// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
        ///
        /// This is only used for visualization purposes, and does not affect the projection itself.
        std::optional<ComponentBatch> image_plane_distance;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.PinholeIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Pinhole";

        /// `ComponentDescriptor` for the `image_from_camera` field.
        static constexpr auto Descriptor_image_from_camera = ComponentDescriptor(
            ArchetypeName, "image_from_camera",
            Loggable<rerun::components::PinholeProjection>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `resolution` field.
        static constexpr auto Descriptor_resolution = ComponentDescriptor(
            ArchetypeName, "resolution",
            Loggable<rerun::components::Resolution>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `camera_xyz` field.
        static constexpr auto Descriptor_camera_xyz = ComponentDescriptor(
            ArchetypeName, "camera_xyz",
            Loggable<rerun::components::ViewCoordinates>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `image_plane_distance` field.
        static constexpr auto Descriptor_image_plane_distance = ComponentDescriptor(
            ArchetypeName, "image_plane_distance",
            Loggable<rerun::components::ImagePlaneDistance>::Descriptor.component_name
        );

      public: // START of extensions from pinhole_ext.cpp:
        /// Creates a pinhole from the camera focal length and resolution, both specified in pixels.
        ///
        /// The focal length is the diagonal of the projection matrix.
        /// Set the same value for x & y value for symmetric cameras, or two values for anamorphic
        /// cameras.
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_focal_length_and_resolution(
            const datatypes::Vec2D& focal_length, const datatypes::Vec2D& resolution
        );

        /// Creates a symmetric pinhole from the camera focal length and resolution, both specified
        /// in pixels.
        ///
        /// The focal length is the diagonal of the projection matrix.
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_focal_length_and_resolution(
            float focal_length, const datatypes::Vec2D& resolution
        ) {
            return from_focal_length_and_resolution({focal_length, focal_length}, resolution);
        }

        /// Creates a pinhole from the camera vertical field of view (in radians) and aspect ratio (width/height).
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_fov_and_aspect_ratio(float fov_y, float aspect_ratio) {
            const float EPSILON = std::numeric_limits<float>::epsilon();
            // `max` has explicit template args to avoid preprocessor replacement when <windows.h> is included without NOMINMAX.
            const float focal_length_y = 0.5f / std::tan(std::max<float>(fov_y * 0.5f, EPSILON));
            return from_focal_length_and_resolution(
                {focal_length_y, focal_length_y},
                {aspect_ratio, 1.0}
            );
        }

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        Pinhole with_resolution(float width, float height) && {
            return std::move(*this).with_resolution(rerun::components::Resolution(width, height));
        }

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        Pinhole with_resolution(int width, int height) && {
            return std::move(*this).with_resolution(rerun::components::Resolution(width, height));
        }

        // END of extensions from pinhole_ext.cpp, start of generated code:

      public:
        Pinhole() = default;
        Pinhole(Pinhole&& other) = default;
        Pinhole(const Pinhole& other) = default;
        Pinhole& operator=(const Pinhole& other) = default;
        Pinhole& operator=(Pinhole&& other) = default;

        explicit Pinhole(rerun::components::PinholeProjection _image_from_camera)
            : image_from_camera(ComponentBatch::from_loggable(
                                    std::move(_image_from_camera), Descriptor_image_from_camera
              )
                                    .value_or_throw()) {}

        /// Update only some specific fields of a `Pinhole`.
        static Pinhole update_fields() {
            return Pinhole();
        }

        /// Clear all the fields of a `Pinhole`.
        static Pinhole clear_fields();

        /// Camera projection, from image coordinates to view coordinates.
        Pinhole with_image_from_camera(
            const rerun::components::PinholeProjection& _image_from_camera
        ) && {
            image_from_camera =
                ComponentBatch::from_loggable(_image_from_camera, Descriptor_image_from_camera)
                    .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `image_from_camera` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_image_from_camera` should
        /// be used when logging a single row's worth of data.
        Pinhole with_many_image_from_camera(
            const Collection<rerun::components::PinholeProjection>& _image_from_camera
        ) && {
            image_from_camera =
                ComponentBatch::from_loggable(_image_from_camera, Descriptor_image_from_camera)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// Example:
        /// ```text
        /// [1920.0, 1440.0]
        /// ```
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        Pinhole with_resolution(const rerun::components::Resolution& _resolution) && {
            resolution =
                ComponentBatch::from_loggable(_resolution, Descriptor_resolution).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `resolution` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_resolution` should
        /// be used when logging a single row's worth of data.
        Pinhole with_many_resolution(const Collection<rerun::components::Resolution>& _resolution
        ) && {
            resolution =
                ComponentBatch::from_loggable(_resolution, Descriptor_resolution).value_or_throw();
            return std::move(*this);
        }

        /// Sets the view coordinates for the camera.
        ///
        /// All common values are available as constants on the `components::ViewCoordinates` class.
        ///
        /// The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
        /// This means that the camera frustum will point along the positive Z axis of the parent space,
        /// and the cameras "up" direction will be along the negative Y axis of the parent space.
        ///
        /// The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
        /// When logging a depth image under this entity, this is the direction the point cloud will be projected.
        /// With `RDF`, the default forward is +Z.
        ///
        /// The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
        /// This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
        /// With `RDF`, the default is up is -Y.
        ///
        /// The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
        /// This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
        /// With `RDF`, the default right is +x.
        ///
        /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).
        ///
        /// NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
        /// and make the pinhole matrix not match up with the coordinate system of the pinhole entity.
        ///
        /// The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
        /// but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
        Pinhole with_camera_xyz(const rerun::components::ViewCoordinates& _camera_xyz) && {
            camera_xyz =
                ComponentBatch::from_loggable(_camera_xyz, Descriptor_camera_xyz).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `camera_xyz` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_camera_xyz` should
        /// be used when logging a single row's worth of data.
        Pinhole with_many_camera_xyz(
            const Collection<rerun::components::ViewCoordinates>& _camera_xyz
        ) && {
            camera_xyz =
                ComponentBatch::from_loggable(_camera_xyz, Descriptor_camera_xyz).value_or_throw();
            return std::move(*this);
        }

        /// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
        ///
        /// This is only used for visualization purposes, and does not affect the projection itself.
        Pinhole with_image_plane_distance(
            const rerun::components::ImagePlaneDistance& _image_plane_distance
        ) && {
            image_plane_distance = ComponentBatch::from_loggable(
                                       _image_plane_distance,
                                       Descriptor_image_plane_distance
            )
                                       .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `image_plane_distance` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_image_plane_distance` should
        /// be used when logging a single row's worth of data.
        Pinhole with_many_image_plane_distance(
            const Collection<rerun::components::ImagePlaneDistance>& _image_plane_distance
        ) && {
            image_plane_distance = ComponentBatch::from_loggable(
                                       _image_plane_distance,
                                       Descriptor_image_plane_distance
            )
                                       .value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Pinhole> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Pinhole& archetype);
    };
} // namespace rerun
