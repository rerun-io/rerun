#include <rerun.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_view_coordinates");
    rec.save(argv[1]).exit_on_failure();
    rec.log_static("/", rerun::archetypes::ViewCoordinates::RDF);
}
