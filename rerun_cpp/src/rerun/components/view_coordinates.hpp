// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/view_coordinates.fbs".

#pragma once

#include "../rerun_sdk_export.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
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
        /// X=Up, Y=Left, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates ULF;

        /// X=Up, Y=Forward, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UFL;

        /// X=Left, Y=Up, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LUF;

        /// X=Left, Y=Forward, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LFU;

        /// X=Forward, Y=Up, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FUL;

        /// X=Forward, Y=Left, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FLU;

        /// X=Up, Y=Left, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates ULB;

        /// X=Up, Y=Back, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UBL;

        /// X=Left, Y=Up, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LUB;

        /// X=Left, Y=Back, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LBU;

        /// X=Back, Y=Up, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BUL;

        /// X=Back, Y=Left, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BLU;

        /// X=Up, Y=Right, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates URF;

        /// X=Up, Y=Forward, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UFR;

        /// X=Right, Y=Up, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RUF;

        /// X=Right, Y=Forward, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RFU;

        /// X=Forward, Y=Up, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FUR;

        /// X=Forward, Y=Right, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FRU;

        /// X=Up, Y=Right, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates URB;

        /// X=Up, Y=Back, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UBR;

        /// X=Right, Y=Up, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RUB;

        /// X=Right, Y=Back, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RBU;

        /// X=Back, Y=Up, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BUR;

        /// X=Back, Y=Right, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BRU;

        /// X=Down, Y=Left, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DLF;

        /// X=Down, Y=Forward, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DFL;

        /// X=Left, Y=Down, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LDF;

        /// X=Left, Y=Forward, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LFD;

        /// X=Forward, Y=Down, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FDL;

        /// X=Forward, Y=Left, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FLD;

        /// X=Down, Y=Left, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DLB;

        /// X=Down, Y=Back, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DBL;

        /// X=Left, Y=Down, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LDB;

        /// X=Left, Y=Back, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LBD;

        /// X=Back, Y=Down, Z=Left
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BDL;

        /// X=Back, Y=Left, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BLD;

        /// X=Down, Y=Right, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DRF;

        /// X=Down, Y=Forward, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DFR;

        /// X=Right, Y=Down, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RDF;

        /// X=Right, Y=Forward, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RFD;

        /// X=Forward, Y=Down, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FDR;

        /// X=Forward, Y=Right, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FRD;

        /// X=Down, Y=Right, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DRB;

        /// X=Down, Y=Back, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DBR;

        /// X=Right, Y=Down, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RDB;

        /// X=Right, Y=Back, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RBD;

        /// X=Back, Y=Down, Z=Right
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BDR;

        /// X=Back, Y=Right, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BRD;

        /// X=Up, Y=Right, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_X_UP;

        /// X=Down, Y=Right, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_X_DOWN;

        /// X=Right, Y=Up, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Y_UP;

        /// X=Right, Y=Down, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Y_DOWN;

        /// X=Right, Y=Forward, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Z_UP;

        /// X=Right, Y=Back, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Z_DOWN;

        /// X=Up, Y=Right, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_X_UP;

        /// X=Down, Y=Right, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_X_DOWN;

        /// X=Right, Y=Up, Z=Forward
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Y_UP;

        /// X=Right, Y=Down, Z=Back
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Y_DOWN;

        /// X=Right, Y=Back, Z=Up
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Z_UP;

        /// X=Right, Y=Forward, Z=Down
        RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Z_DOWN;

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

        /// Serializes an array of `rerun::components::ViewCoordinates` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::ViewCoordinates* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const components::ViewCoordinates* elements,
            size_t num_elements
        );
    };
} // namespace rerun
