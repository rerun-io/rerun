#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_text_document");
    rec.save(argv[1]).throw_on_failure();
    rec.log("text_document", rr::archetypes::TextDocument("Hello, TextDocument!"));
    rec.log(
        "markdown",
        rr::archetypes::TextDocument("# Hello\n"
                                     "Markdown with `code`!\n"
                                     "\n"
                                     "A random image:\n"
                                     "\n"
                                     ""
                                     "![A random image](https://picsum.photos/640/480)")
            .with_media_type(rr::components::MediaType::markdown())
    );
}
