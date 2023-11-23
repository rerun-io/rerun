#include "indicator_component.hpp"
#include <arrow/array/array_base.h>

namespace rerun::components {
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype() {
        return arrow::null();
    }

    const std::shared_ptr<arrow::Array>& indicator_arrow_array() {
        // Lazily create an array for the indicator (only once)
        static std::shared_ptr<arrow::Array> single_indicator_array =
            std::make_shared<arrow::NullArray>(1);
        return single_indicator_array;
    }
} // namespace rerun::components
