#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rr_stream = rr::RecordingStream("rerun_example_text_document");
    rr_stream.save(argv[1]).throw_on_failure();
    rr_stream.log("text_document", rr::archetypes::TextDocument("Hello, TextDocument!"));
}
