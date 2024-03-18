// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs".

#pragma once

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
    /// **Datatype**: A Quaternion represented by 4 real numbers.
    ///
    /// Note: although the x,y,z,w components of the quaternion will be passed through to the
    /// datastore as provided, when used in the viewer Quaternions will always be normalized.
    struct Quaternion {
        std::array<float, 4> xyzw;

      public:
        // Extensions to generated type defined in 'quaternion_ext.cpp'

        static const Quaternion IDENTITY;

        /// Construct Quaternion from x/y/z/w values.
        static Quaternion from_xyzw(float x, float y, float z, float w) {
            return Quaternion::from_xyzw({x, y, z, w});
        }

        /// Construct Quaternion from w/x/y/z values.
        static Quaternion from_wxyz(float w, float x, float y, float z) {
            return Quaternion::from_xyzw(x, y, z, w);
        }

        /// Construct Quaternion from x/y/z/w array.
        static Quaternion from_xyzw(std::array<float, 4> xyzw_) {
            Quaternion q;
            q.xyzw = xyzw_;
            return q;
        }

        /// Construct Quaternion from w/x/y/z array.
        static Quaternion from_wxyz(std::array<float, 4> wxyz_) {
            return Quaternion::from_xyzw(wxyz_[1], wxyz_[2], wxyz_[3], wxyz_[0]);
        }

        /// Construct Quaternion from x/y/z/w float pointer.
        static Quaternion from_xyzw(const float* xyzw_) {
            return Quaternion::from_xyzw(xyzw_[0], xyzw_[1], xyzw_[2], xyzw_[3]);
        }

        /// Construct Quaternion from w/x/y/z float pointer.
        static Quaternion from_wxyz(const float* wxyz_) {
            return Quaternion::from_xyzw(wxyz_[1], wxyz_[2], wxyz_[3], wxyz_[0]);
        }

        float x() const {
            return xyzw[0];
        }

        float y() const {
            return xyzw[1];
        }

        float z() const {
            return xyzw[2];
        }

        float w() const {
            return xyzw[3];
        }

      public:
        Quaternion() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::Quaternion> {
        static constexpr const char Name[] = "rerun.datatypes.Quaternion";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::Quaternion` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::Quaternion* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const datatypes::Quaternion* elements,
            size_t num_elements
        );
    };
} // namespace rerun
