// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/float32.fbs".

#include "float32.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {
    const std::shared_ptr<arrow::DataType>& Float32::arrow_datatype() {
        static const auto datatype = arrow::float32();
        return datatype;
    }

    rerun::Error Float32::fill_arrow_array_builder(
        arrow::FloatBuilder* builder, const Float32* elements, size_t num_elements
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

        static_assert(sizeof(*elements) == sizeof(elements->value));
        ARROW_RETURN_NOT_OK(
            builder->AppendValues(&elements->value, static_cast<int64_t>(num_elements))
        );

        return Error::ok();
    }
} // namespace rerun::datatypes
