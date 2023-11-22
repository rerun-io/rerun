// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/view_coordinates.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: How we interpret the coordinate system of an entity/space.
    ///
    /// For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?
    ///
    /// The three coordinates are always ordered as [x, y, z].
    ///
    /// For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
    /// down, and the Z axis points forward.
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

      public:
        // Extensions to generated type defined in 'view_coordinates_ext.cpp'

        enum ViewDir : uint8_t {
            Up = 1,
            Down = 2,
            Right = 3,
            Left = 4,
            Forward = 5,
            Back = 6,
        };

        /// Construct Vec3D from x/y/z values.
        constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
            : coordinates{axis0, axis1, axis2} {}

        /// Construct Vec3D from x/y/z values.
        constexpr ViewCoordinates(ViewDir axis0, ViewDir axis1, ViewDir axis2)
            : coordinates{axis0, axis1, axis2} {}

        // <BEGIN_GENERATED:declarations>
        // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
        static const rerun::components::ViewCoordinates ULF;
        static const rerun::components::ViewCoordinates UFL;
        static const rerun::components::ViewCoordinates LUF;
        static const rerun::components::ViewCoordinates LFU;
        static const rerun::components::ViewCoordinates FUL;
        static const rerun::components::ViewCoordinates FLU;
        static const rerun::components::ViewCoordinates ULB;
        static const rerun::components::ViewCoordinates UBL;
        static const rerun::components::ViewCoordinates LUB;
        static const rerun::components::ViewCoordinates LBU;
        static const rerun::components::ViewCoordinates BUL;
        static const rerun::components::ViewCoordinates BLU;
        static const rerun::components::ViewCoordinates URF;
        static const rerun::components::ViewCoordinates UFR;
        static const rerun::components::ViewCoordinates RUF;
        static const rerun::components::ViewCoordinates RFU;
        static const rerun::components::ViewCoordinates FUR;
        static const rerun::components::ViewCoordinates FRU;
        static const rerun::components::ViewCoordinates URB;
        static const rerun::components::ViewCoordinates UBR;
        static const rerun::components::ViewCoordinates RUB;
        static const rerun::components::ViewCoordinates RBU;
        static const rerun::components::ViewCoordinates BUR;
        static const rerun::components::ViewCoordinates BRU;
        static const rerun::components::ViewCoordinates DLF;
        static const rerun::components::ViewCoordinates DFL;
        static const rerun::components::ViewCoordinates LDF;
        static const rerun::components::ViewCoordinates LFD;
        static const rerun::components::ViewCoordinates FDL;
        static const rerun::components::ViewCoordinates FLD;
        static const rerun::components::ViewCoordinates DLB;
        static const rerun::components::ViewCoordinates DBL;
        static const rerun::components::ViewCoordinates LDB;
        static const rerun::components::ViewCoordinates LBD;
        static const rerun::components::ViewCoordinates BDL;
        static const rerun::components::ViewCoordinates BLD;
        static const rerun::components::ViewCoordinates DRF;
        static const rerun::components::ViewCoordinates DFR;
        static const rerun::components::ViewCoordinates RDF;
        static const rerun::components::ViewCoordinates RFD;
        static const rerun::components::ViewCoordinates FDR;
        static const rerun::components::ViewCoordinates FRD;
        static const rerun::components::ViewCoordinates DRB;
        static const rerun::components::ViewCoordinates DBR;
        static const rerun::components::ViewCoordinates RDB;
        static const rerun::components::ViewCoordinates RBD;
        static const rerun::components::ViewCoordinates BDR;
        static const rerun::components::ViewCoordinates BRD;
        static const rerun::components::ViewCoordinates RIGHT_HAND_X_UP;
        static const rerun::components::ViewCoordinates RIGHT_HAND_X_DOWN;
        static const rerun::components::ViewCoordinates RIGHT_HAND_Y_UP;
        static const rerun::components::ViewCoordinates RIGHT_HAND_Y_DOWN;
        static const rerun::components::ViewCoordinates RIGHT_HAND_Z_UP;
        static const rerun::components::ViewCoordinates RIGHT_HAND_Z_DOWN;
        static const rerun::components::ViewCoordinates LEFT_HAND_X_UP;
        static const rerun::components::ViewCoordinates LEFT_HAND_X_DOWN;
        static const rerun::components::ViewCoordinates LEFT_HAND_Y_UP;
        static const rerun::components::ViewCoordinates LEFT_HAND_Y_DOWN;
        static const rerun::components::ViewCoordinates LEFT_HAND_Z_UP;
        static const rerun::components::ViewCoordinates LEFT_HAND_Z_DOWN;
        // <END_GENERATED:declarations>

      public:
        ViewCoordinates() = default;

        ViewCoordinates(std::array<uint8_t, 3> coordinates_) : coordinates(coordinates_) {}

        ViewCoordinates& operator=(std::array<uint8_t, 3> coordinates_) {
            coordinates = coordinates_;
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::ViewCoordinates> {
        static constexpr const char Name[] = "rerun.components.ViewCoordinates";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const components::ViewCoordinates* elements,
            size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::ViewCoordinates` components.
        static Result<rerun::DataCell> to_data_cell(
            const components::ViewCoordinates* instances, size_t num_instances
        );
    };
} // namespace rerun
