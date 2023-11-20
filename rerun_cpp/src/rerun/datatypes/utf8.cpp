// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/utf8.fbs".

#include "utf8.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {
    const std::shared_ptr<arrow::DataType>& Utf8::arrow_datatype() {
        static const auto datatype = arrow::utf8();
        return datatype;
    }

    rerun::Error Utf8::fill_arrow_array_builder(
        arrow::StringBuilder* builder, const Utf8* elements, size_t num_elements
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

        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            ARROW_RETURN_NOT_OK(builder->Append(elements[elem_idx].value));
        }

        return Error::ok();
    }
} // namespace rerun::datatypes
