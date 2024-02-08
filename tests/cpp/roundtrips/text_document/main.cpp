#include <rerun.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_text_document");
    rec.save(argv[1]).exit_on_failure();
    rec.log("text_document", rerun::archetypes::TextDocument("Hello, TextDocument!"));
    rec.log(
        "markdown",
        rerun::archetypes::TextDocument("# Hello\n"
                                        "Markdown with `code`!\n"
                                        "\n"
                                        "A random image:\n"
                                        "\n"
                                        ""
                                        "![A random image](https://picsum.photos/640/480)")
            .with_media_type(rerun::components::MediaType::markdown())
    );
}
