// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#include "translation_rotation_scale3d.hpp"

#include "../datatypes/quaternion.hpp"
#include "../datatypes/rotation_axis_angle.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> TranslationRotationScale3D::to_arrow_datatype() {
            return arrow::struct_({
                arrow::field(
                    "translation", rr::datatypes::Vec3D::to_arrow_datatype(), true, nullptr),
                arrow::field("rotation",
                             arrow::dense_union({
                                 arrow::field("_null_markers", arrow::null(), true, nullptr),
                                 arrow::field("Quaternion",
                                              rr::datatypes::Quaternion::to_arrow_datatype(),
                                              false,
                                              nullptr),
                                 arrow::field("AxisAngle",
                                              rr::datatypes::RotationAxisAngle::to_arrow_datatype(),
                                              false,
                                              nullptr),
                             }),
                             true,
                             nullptr),
                arrow::field(
                    "scale",
                    arrow::dense_union({
                        arrow::field("_null_markers", arrow::null(), true, nullptr),
                        arrow::field(
                            "ThreeD", rr::datatypes::Vec3D::to_arrow_datatype(), false, nullptr),
                        arrow::field("Uniform", arrow::float32(), false, nullptr),
                    }),
                    true,
                    nullptr),
                arrow::field("from_parent", arrow::boolean(), false, nullptr),
            });
        }
    } // namespace datatypes
} // namespace rr
