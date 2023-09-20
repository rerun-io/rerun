#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rr_stream = rr::RecordingStream("rerun_example_roundtrip_view_coordinates");
    rr_stream.save(argv[1]).throw_on_failure();
    rr_stream.log("/", rr::archetypes::ViewCoordinates::RDF);
}
