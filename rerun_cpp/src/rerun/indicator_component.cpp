#include "indicator_component.hpp"
#include "compiler_utils.hpp"

#include <arrow/array/array_base.h>

namespace rerun::components {
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype() {
        static const std::shared_ptr<arrow::DataType> datatype = arrow::null();
        return datatype;
    }

    RR_DISABLE_ADDRESS_SANITIZER
    const std::shared_ptr<arrow::Array>& indicator_arrow_array() {
        // Lazily create an array for the indicator (only once)
        static const std::shared_ptr<arrow::Array> single_indicator_array =
            std::make_shared<arrow::NullArray>(1);
        return single_indicator_array;
    }
} // namespace rerun::components
