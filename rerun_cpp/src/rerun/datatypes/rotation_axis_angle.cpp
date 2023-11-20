// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/rotation_axis_angle.fbs".

#include "rotation_axis_angle.hpp"

#include "angle.hpp"
#include "vec3d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {
    const std::shared_ptr<arrow::DataType>& RotationAxisAngle::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field("axis", rerun::datatypes::Vec3D::arrow_datatype(), false),
            arrow::field("angle", rerun::datatypes::Angle::arrow_datatype(), false),
        });
        return datatype;
    }

    rerun::Error RotationAxisAngle::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const RotationAxisAngle* elements, size_t num_elements
    ) {
        if (builder == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return rerun::Error(
                ErrorCode::UnexpectedNullArgument,
                "Cannot serialize null pointer to arrow array."
            );
        }

        {
            auto field_builder =
                static_cast<arrow::FixedSizeListBuilder*>(builder->field_builder(0));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                RR_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
                    field_builder,
                    &elements[elem_idx].axis,
                    1
                ));
            }
        }
        {
            auto field_builder = static_cast<arrow::DenseUnionBuilder*>(builder->field_builder(1));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                RR_RETURN_NOT_OK(rerun::datatypes::Angle::fill_arrow_array_builder(
                    field_builder,
                    &elements[elem_idx].angle,
                    1
                ));
            }
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }
} // namespace rerun::datatypes
