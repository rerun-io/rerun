#include "indicator_component.hpp"
#include "compiler_utils.hpp"

#include <arrow/array/array_base.h>

namespace rerun::components {
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype() {
        return arrow::null(); // Note that this already returns a shared_ptr reference.
    }

    const std::shared_ptr<arrow::Array>& indicator_arrow_array() {
        static const std::shared_ptr<arrow::Array> single_indicator_array =
            std::make_shared<arrow::NullArray>(1);
        return single_indicator_array;
    }

    std::shared_ptr<arrow::Array> indicator_arrow_array(size_t num_instances) {
        return std::make_shared<arrow::NullArray>(num_instances);
    }
} // namespace rerun::components
