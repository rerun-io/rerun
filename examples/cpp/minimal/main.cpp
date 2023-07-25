#include <loguru.hpp>
#include <rerun.hpp>

#include <components/point2d.hpp>

arrow::Result<std::shared_ptr<arrow::Table>> points2(size_t num_points, const float* xy) {
    arrow::MemoryPool* pool = arrow::default_memory_pool();

    ARROW_ASSIGN_OR_RAISE(auto builder, rr::components::Point2D::new_arrow_array_builder(pool));
    ARROW_RETURN_NOT_OK(rr::components::Point2D::fill_arrow_array_builder(
        builder.get(),
        (const rr::components::Point2D*)xy, // TODO(andreas): Hack to get Points2D C-style array in an easy fashion
        num_points
    ));

    std::shared_ptr<arrow::Array> array;
    ARROW_RETURN_NOT_OK(builder->Finish(&array));

    auto name = "points"; // Unused, but should be the name of the field in the archetype
    auto schema =
        arrow::schema({arrow::field(name, rr::components::Point2D::to_arrow_datatype(), false)});

    return arrow::Table::Make(schema, {array});
}

int main(int argc, char** argv) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    LOG_F(INFO, "Rerun C++ SDK version: %s", rr::version_string());

    auto rr_stream = rr::RecordingStream{"c-example-app", "127.0.0.1:9876"};

    // Points3D.
    {
        float xyz[9] = {0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 5.0, 5.0, 5.0};
        auto points = rr::points3(3, xyz).ValueOrDie(); // TODO(andreas): phase this out.
        auto buffer = rr::ipc_from_table(*points).ValueOrDie();

        const rr::DataCell data_cells[1] = {rr::DataCell{
            .component_name = "rerun.point3d",
            .num_bytes = static_cast<size_t>(buffer->size()),
            .bytes = buffer->data(),
        }};

        uint32_t num_instances = 3;
        rr_stream.log_data_row("3d/points", num_instances, 1, data_cells);
    }

    // Points2D.
    {
        float xy[6] = {0.0, 0.0, 1.0, 3.0, 5.0, 5.0};
        auto points = points2(3, xy).ValueOrDie();
        auto buffer = rr::ipc_from_table(*points).ValueOrDie();

        const rr::DataCell data_cells[1] = {rr::DataCell{
            .component_name = "rerun.point2d",
            .num_bytes = static_cast<size_t>(buffer->size()),
            .bytes = buffer->data(),
        }};

        uint32_t num_instances = 3;
        rr_stream.log_data_row("2d/points", num_instances, 1, data_cells);
    }

    // Test some type instantiation
    auto tls = rr::datatypes::TranslationRotationScale3D{};
    tls.translation = {1.0, 2.0, 3.0};
    rr::datatypes::Transform3D t = std::move(tls);
}
