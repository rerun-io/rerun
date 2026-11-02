#include "arrow_serialization.hpp"

#include <arrow/builder.h>

namespace rerun::detail {
    Result<std::shared_ptr<arrow::Array>> serialize_arrow(
        const std::shared_ptr<arrow::DataType>& datatype, const void* instances, size_t count,
        ArrowFill fill
    ) {
        ARROW_ASSIGN_OR_RAISE(
            auto builder,
            arrow::MakeBuilder(datatype, arrow::default_memory_pool())
        )
        if (instances && count > 0) {
            RR_RETURN_NOT_OK(fill(builder.get(), instances, count));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    Error check_arrow_builder_arguments(const arrow::ArrayBuilder* builder, const void* elements) {
        if (builder == nullptr) {
            return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return Error(
                ErrorCode::UnexpectedNullArgument,
                "Cannot serialize null pointer to arrow array."
            );
        }
        return Error::ok();
    }
} // namespace rerun::detail
