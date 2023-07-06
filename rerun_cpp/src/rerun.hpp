// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

namespace rr {
    const char* version_string();
} // namespace rr

// ----------------------------------------------------------------------------
// Arrow integration

#include <arrow/api.h>

namespace rr {
    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points, const float* xyz);

    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rr

// ----------------------------------------------------------------------------

#endif // RERUN_HPP
