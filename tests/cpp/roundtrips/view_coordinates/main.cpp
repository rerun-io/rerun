#include <rerun.hpp>

int main(int argc, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_view_coordinates");
    rec.save(argv[1]).throw_on_failure();
    rec.log("/", rerun::archetypes::ViewCoordinates::RDF);
}
