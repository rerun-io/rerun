// Log a simple MCAP schema definition.

#include <rerun.hpp>
#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_mcap_schema");
    rec.spawn().exit_on_failure();

    // Example ROS2 message definition for a simple Point message
    const char* point_schema = "float64 x\nfloat64 y\nfloat64 z";
    const std::vector<uint8_t> schema_data(point_schema, point_schema + strlen(point_schema));

    rec.log(
        "mcap/schemas/geometry_point",
        rerun::archetypes::McapSchema(
            42,
            "geometry_msgs/msg/Point",
            "ros2msg",
            rerun::components::Blob(schema_data)
        )
    );
}
