// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/view_coordinates.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class FixedSizeListBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: How we interpret the coordinate system of an entity/space.
    ///
    /// For instance: What is "up"? What does the Z axis mean?
    ///
    /// The three coordinates are always ordered as [x, y, z].
    ///
    /// For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
    /// down, and the Z axis points forward.
    ///
    /// ⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).
    ///
    /// The following constants are used to represent the different directions:
    ///  * Up = 1
    ///  * Down = 2
    ///  * Right = 3
    ///  * Left = 4
    ///  * Forward = 5
    ///  * Back = 6
    struct ViewCoordinates {
        /// The directions of the [x, y, z] axes.
        std::array<uint8_t, 3> coordinates;

      public: // START of extensions from view_coordinates_ext.cpp:
        /// Construct Vec3D from x/y/z values.
        explicit constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
            : coordinates{axis0, axis1, axis2} {}

        // END of extensions from view_coordinates_ext.cpp, start of generated code:

      public:
        ViewCoordinates() = default;

        ViewCoordinates(std::array<uint8_t, 3> coordinates_) : coordinates(coordinates_) {}

        ViewCoordinates& operator=(std::array<uint8_t, 3> coordinates_) {
            coordinates = coordinates_;
            return *this;
        }
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::ViewCoordinates> {
        static constexpr ComponentDescriptor Descriptor = "rerun.datatypes.ViewCoordinates";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::ViewCoordinates` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::ViewCoordinates* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const datatypes::ViewCoordinates* elements,
            size_t num_elements
        );
    };
} // namespace rerun
