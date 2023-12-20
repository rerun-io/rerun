// Log a `TextDocument`

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_text_document");
    rec.spawn().exit_on_failure();

    rec.log(
        R"(world/escaped\ string\!)",
        rerun::TextDocument("This entity path was escaped manually")
    );
    // TODO: figure this one out
    // rec.log(
    //     rerun::entity_path ![ "world", "unescaped string!" ],
    //     rerun::TextDocument("This entity path was provided as a list of unescaped strings")
    // );
}
