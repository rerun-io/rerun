#include <loguru.hpp>
#include <rerun.hpp>

int main(int argc, char** argv) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    LOG_F(INFO, "Rerun C++ SDK version: %s", rr::version_string());

    auto rr_stream = rr::RecordingStream{"c-example-app", "127.0.0.1:9876"};

    float xyz[9] = {0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 5.0, 5.0, 5.0};
    auto points = rr::points3(3, xyz).ValueOrDie();
    auto buffer = rr::ipc_from_table(*points).ValueOrDie();

    const rr::DataCell data_cells[1] = {rr::DataCell{
        .component_name = "rerun.point3d",
        .num_bytes = static_cast<size_t>(buffer->size()),
        .bytes = buffer->data(),
    }};

    uint32_t num_instances = 3;
    rr_stream.log_data_row("points", num_instances, 1, data_cells);
}
