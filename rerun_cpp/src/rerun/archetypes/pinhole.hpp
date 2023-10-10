// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/pinhole.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/pinhole_projection.hpp"
#include "../components/resolution.hpp"
#include "../components/view_coordinates.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: Camera perspective projection (a.k.a. intrinsics).
        ///
        /// ## Example
        ///
        /// ### Simple Pinhole Camera
        /// ```cpp,ignore
        /// // Log a pinhole and a random image.
        ///
        /// #include <rerun.hpp>
        ///
        /// #include <algorithm>
        /// #include <cstdlib>
        /// #include <ctime>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///     auto rec = rr::RecordingStream("rerun_example_line_strip3d");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     rec.log("world/image", rerun::Pinhole::focal_length_and_resolution({3.0f, 3.0f},
        ///     {3.0f, 3.0f}));
        ///
        ///     // TODO(andreas): Improve ergonomics.
        ///     rerun::datatypes::TensorData tensor;
        ///     rerun::datatypes::TensorDimension dim3;
        ///     dim3.size = 3;
        ///     tensor.shape = {dim3, dim3, dim3};
        ///     std::srand(static_cast<uint32_t>(std::time(nullptr)));
        ///     std::vector<uint8_t> random_data(3 * 3 * 3);
        ///     std::generate(random_data.begin(), random_data.end(), std::rand);
        ///     tensor.buffer = rerun::datatypes::TensorBuffer::u8(random_data);
        ///
        ///     rec.log("world/image", rerun::Image(tensor));
        /// }
        /// ```
        struct Pinhole {
            /// Camera projection, from image coordinates to view coordinates.
            rerun::components::PinholeProjection image_from_camera;

            /// Pixel resolution (usually integers) of child image space. Width and height.
            ///
            /// Example:
            /// ```text
            /// [1920.0, 1440.0]
            /// ```
            ///
            /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
            std::optional<rerun::components::Resolution> resolution;

            /// Sets the view coordinates for the camera.
            ///
            /// All common values are available as constants on the `components.ViewCoordinates`
            /// class.
            ///
            /// The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is
            /// also the recommended setting. This means that the camera frustum will point along
            /// the positive Z axis of the parent space, and the cameras "up" direction will be
            /// along the negative Y axis of the parent space.
            ///
            /// The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
            /// When logging a depth image under this entity, this is the direction the point cloud
            /// will be projected. With `RDF`, the default forward is +Z.
            ///
            /// The frustum's "up" direction will be whichever axis is set to `U` (or the opposite
            /// of `D`). This will match the negative Y direction of pixel space (all images are
            /// assumed to have xyz=RDF). With `RDF`, the default is up is -Y.
            ///
            /// The frustum's "right" direction will be whichever axis is set to `R` (or the
            /// opposite of `L`). This will match the positive X direction of pixel space (all
            /// images are assumed to have xyz=RDF). With `RDF`, the default right is +x.
            ///
            /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left,
            /// Z=Up).
            ///
            /// NOTE: setting this to something else than `RDF` (the default) will change the
            /// orientation of the camera frustum, and make the pinhole matrix not match up with the
            /// coordinate system of the pinhole entity.
            ///
            /// The pinhole matrix (the `image_from_camera` argument) always project along the third
            /// (Z) axis, but will be re-oriented to project along the forward axis of the
            /// `camera_xyz` argument.
            std::optional<rerun::components::ViewCoordinates> camera_xyz;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            // Extensions to generated type defined in 'pinhole_ext.cpp'

            /// Creates a pinhole from the camera focal length and resolution, both specified in
            /// pixels.
            ///
            /// The focal length is the diagonal of the projection matrix.
            /// Set the same value for x & y value for symmetric cameras, or two values for
            /// anamorphic cameras.
            ///
            /// Assumes the principal point to be in the middle of the sensor.
            static Pinhole focal_length_and_resolution(
                const datatypes::Vec2D& focal_length, const datatypes::Vec2D& resolution
            );

          public:
            Pinhole() = default;
            Pinhole(Pinhole&& other) = default;

            Pinhole(rerun::components::PinholeProjection _image_from_camera)
                : image_from_camera(std::move(_image_from_camera)) {}

            /// Pixel resolution (usually integers) of child image space. Width and height.
            ///
            /// Example:
            /// ```text
            /// [1920.0, 1440.0]
            /// ```
            ///
            /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
            Pinhole with_resolution(rerun::components::Resolution _resolution) && {
                resolution = std::move(_resolution);
                return std::move(*this);
            }

            /// Sets the view coordinates for the camera.
            ///
            /// All common values are available as constants on the `components.ViewCoordinates`
            /// class.
            ///
            /// The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is
            /// also the recommended setting. This means that the camera frustum will point along
            /// the positive Z axis of the parent space, and the cameras "up" direction will be
            /// along the negative Y axis of the parent space.
            ///
            /// The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
            /// When logging a depth image under this entity, this is the direction the point cloud
            /// will be projected. With `RDF`, the default forward is +Z.
            ///
            /// The frustum's "up" direction will be whichever axis is set to `U` (or the opposite
            /// of `D`). This will match the negative Y direction of pixel space (all images are
            /// assumed to have xyz=RDF). With `RDF`, the default is up is -Y.
            ///
            /// The frustum's "right" direction will be whichever axis is set to `R` (or the
            /// opposite of `L`). This will match the positive X direction of pixel space (all
            /// images are assumed to have xyz=RDF). With `RDF`, the default right is +x.
            ///
            /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left,
            /// Z=Up).
            ///
            /// NOTE: setting this to something else than `RDF` (the default) will change the
            /// orientation of the camera frustum, and make the pinhole matrix not match up with the
            /// coordinate system of the pinhole entity.
            ///
            /// The pinhole matrix (the `image_from_camera` argument) always project along the third
            /// (Z) axis, but will be re-oriented to project along the forward axis of the
            /// `camera_xyz` argument.
            Pinhole with_camera_xyz(rerun::components::ViewCoordinates _camera_xyz) && {
                camera_xyz = std::move(_camera_xyz);
                return std::move(*this);
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }
        };

    } // namespace archetypes
    template <typename TComponent>
    struct AsComponents;

    template <>
    struct AsComponents<archetypes::Pinhole> {
        /// Serialize all set component batches.
        Result<std::vector<SerializedComponentBatch>> serialize(const archetypes::Pinhole& archetype
        ) const;
    };
} // namespace rerun
