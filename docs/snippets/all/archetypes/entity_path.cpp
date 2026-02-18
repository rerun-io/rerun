// Log a `TextDocument`

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_entity_path");
    rec.spawn().exit_on_failure();

    rec.log(
        R"(world/42/escaped\ string\!)",
        rerun::TextDocument("This entity path was escaped manually")
    );
    rec.log(
        rerun::new_entity_path({"world", std::to_string(42), "unescaped string!"}),
        rerun::TextDocument("This entity path was provided as a list of unescaped strings")
    );

    assert(rerun::escape_entity_path_part("my string!") == R"(my\ string\!)");
    assert(rerun::new_entity_path({"world", "42", "my string!"}) == R"(/world/42/my\ string\!)");
}
