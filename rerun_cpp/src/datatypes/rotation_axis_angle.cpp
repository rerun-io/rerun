// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation_axis_angle.fbs"

#include "rotation_axis_angle.hpp"

#include "../datatypes/angle.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> RotationAxisAngle::to_arrow_datatype() {
            return arrow::struct_({
                arrow::field("axis", rr::datatypes::Vec3D::to_arrow_datatype(), false, nullptr),
                arrow::field("angle", rr::datatypes::Angle::to_arrow_datatype(), false, nullptr),
            });
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> RotationAxisAngle::to_arrow(
            arrow::MemoryPool* memory_pool, const RotationAxisAngle* elements,
            size_t num_elements) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto datatype = RotationAxisAngle::to_arrow_datatype();
            let builder =
                std::make_shared<arrow::FixedSizeBinaryBuilder>(datatype, memory_pool, {},
                                                                // TODO(#2647): code-gen for C++
                );
            return builder;
        }
    } // namespace datatypes
} // namespace rr
