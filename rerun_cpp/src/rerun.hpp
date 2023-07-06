// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

namespace rerun {
    const char* version_string();
} // namespace rerun

// ----------------------------------------------------------------------------
// Arrow integration

#include <arrow/api.h>

namespace rerun {
    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points, const float* xyz);

    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rerun

// ----------------------------------------------------------------------------

#endif // RERUN_HPP
