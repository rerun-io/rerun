#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_view_coordinates");
    rec.save(argv[1]).throw_on_failure();
    rec.log("/", rr::archetypes::ViewCoordinates::RDF);
}
