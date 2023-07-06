#include <iostream>

#define RERUN_WITH_ARROW 1

#include <rerun.h> // TODO: use C++ wrappers instead

#include <loguru.hpp>
#include <rerun.hpp>

int main(int argc, char** argv) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    LOG_F(INFO, "Rerun C++ SDK version: %s", rerun::version_string());

    const rr_store_info store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    rr_recording_stream rec_stream = rr_recording_stream_new(&store_info, "0.0.0.0:9876");

    float xyz[9] = {0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 5.0, 5.0, 5.0};
    auto points = rerun::points3(3, xyz).ValueOrDie();
    auto buffer = rerun::ipc_from_table(*points).ValueOrDie();

    const rr_data_cell data_cells[1] = {rr_data_cell{
        .component_name = "rerun.point3d",
        .num_bytes = static_cast<uint64_t>(buffer->size()),
        .bytes = buffer->data(),
    }};

    const rr_data_row data_row = {
        .entity_path = "points", .num_instances = 3, .num_data_cells = 1, .data_cells = data_cells};
    rr_log(rec_stream, &data_row);

    rr_recording_stream_free(rec_stream);
}
