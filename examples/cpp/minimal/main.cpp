#include <loguru.hpp>
#include <rerun.hpp>

#include <components/point2d.hpp>

int main(int argc, char** argv) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    LOG_F(INFO, "Rerun C++ SDK version: %s", rr::version_string());

    auto rr_stream = rr::RecordingStream{"c-example-app", "127.0.0.1:9876"};

    // Points3D.
    {
        float xyz[9] = {0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 5.0, 5.0, 5.0};
        const rr::DataCell data_cells[1] = {
            rr::components::Point3D::to_data_cell((const rr::components::Point3D*)xyz, 3)
                .ValueOrDie()};

        uint32_t num_instances = 3;
        rr_stream.log_data_row("3d/points", num_instances, 1, data_cells);
    }

    // Points2D.
    {
        float xy[6] = {0.0, 0.0, 1.0, 3.0, 5.0, 5.0};
        uint32_t num_instances = 3;
        const rr::DataCell data_cells[1] = {
            rr::components::Point2D::to_data_cell((const rr::components::Point2D*)xy, num_instances)
                .ValueOrDie()};

        rr_stream.log_data_row("2d/points", num_instances, 1, data_cells);
    }

    // Test some type instantiation
    auto tls = rr::datatypes::TranslationRotationScale3D{};
    tls.translation = {1.0, 2.0, 3.0};
    rr::datatypes::Transform3D t = std::move(tls);
}
