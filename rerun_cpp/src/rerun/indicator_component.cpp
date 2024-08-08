#include "indicator_component.hpp"
#include <arrow/array/array_base.h>

#if __has_feature(address_sanitizer)
#define RR_DISABLE_ADDRESS_SANITIZER __attribute__((no_sanitize("address")))
#else
#define RR_DISABLE_ADDRESS_SANITIZER
#endif
namespace rerun::components {
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype() {
        static const auto datatype = arrow::null();
        return datatype;
    }

    // ASAN indicates that we're leaking memory here.
    // It's a bit surprising because we use this pattern of single global allocation
    // for almost all arrow data types to avoid repeated shared_ptr allocations.
    RR_DISABLE_ADDRESS_SANITIZER
    const std::shared_ptr<arrow::Array>& indicator_arrow_array() {
        // Lazily create an array for the indicator (only once)
        static const auto single_indicator_array = std::make_shared<arrow::NullArray>(1);
        return single_indicator_array;
    }
} // namespace rerun::components
