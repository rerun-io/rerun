// Log a `TextDocument`

#include <rerun.hpp>

#include <cmath>

namespace rr = rerun;
namespace rrd = rr::datatypes;

int main() {
    auto rr_stream = rr::RecordingStream("rerun_example_text_document");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    rr_stream.log("text_document", rr::archetypes::TextDocument("Hello, TextDocument!"));
    rr_stream.log(
        "markdown",
        rr::archetypes::TextDocument("# Hello\n"
                                     "Markdown with `code`!\n"
                                     "\n"
                                     "A random image:\n"
                                     "\n"
                                     "![A random image](https://picsum.photos/640/480)")
            .with_media_type(rr::components::MediaType::markdown())
    );
}
