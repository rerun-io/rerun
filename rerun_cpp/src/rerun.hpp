// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

#include <cstdint>
#include <optional>

namespace rr {
    /// The Rerun C++ SDK version as a human-readable string.
    const char* version_string();

    enum StoreKind {
        Recording,
        Blueprint,
    };

    struct DataCell {
        const char* component_name;
        size_t num_bytes;
        const uint8_t* bytes;
    };

    class RecordingStream {
      public:
        RecordingStream(const char* app_id, const char* addr,
                        StoreKind store_kind = StoreKind::Recording);
        ~RecordingStream();

        /// Must be called first, if at all.
        static void init_global(const char* app_id, const char* addr);

        /// Access the global recording stream.
        /// Aborts if `init_global` has not yet been called.
        static RecordingStream global();

        void log_data_row(const char* entity_path, uint32_t num_instances, size_t num_data_cells,
                          const DataCell* data_cells);

      private:
        RecordingStream() : _id{0} {}
        RecordingStream(uint32_t id) : _id{id} {}

        uint32_t _id;

        static RecordingStream s_global;
    };
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
