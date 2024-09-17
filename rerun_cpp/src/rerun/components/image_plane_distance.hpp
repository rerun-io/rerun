// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/image_plane_distance.fbs".

#pragma once

#include "../datatypes/float32.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
    ///
    /// This is only used for visualization purposes, and does not affect the projection itself.
    struct ImagePlaneDistance {
        rerun::datatypes::Float32 image_from_camera;

      public:
        ImagePlaneDistance() = default;

        ImagePlaneDistance(rerun::datatypes::Float32 image_from_camera_)
            : image_from_camera(image_from_camera_) {}

        ImagePlaneDistance& operator=(rerun::datatypes::Float32 image_from_camera_) {
            image_from_camera = image_from_camera_;
            return *this;
        }

        ImagePlaneDistance(float value_) : image_from_camera(value_) {}

        ImagePlaneDistance& operator=(float value_) {
            image_from_camera = value_;
            return *this;
        }

        /// Cast to the underlying Float32 datatype
        operator rerun::datatypes::Float32() const {
            return image_from_camera;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Float32) == sizeof(components::ImagePlaneDistance));

    /// \private
    template <>
    struct Loggable<components::ImagePlaneDistance> {
        static constexpr const char Name[] = "rerun.components.ImagePlaneDistance";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Float32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::ImagePlaneDistance` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::ImagePlaneDistance* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Float32>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Float32>::to_arrow(
                    &instances->image_from_camera,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
