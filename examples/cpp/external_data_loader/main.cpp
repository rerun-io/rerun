#include <fstream>
#include <iostream>
#include <string>

#include <rerun.hpp>

const char* USAGE = R"(
This is an example executable data-loader plugin for the Rerun Viewer.

It will log C++ source code files as markdown documents.
To try it out, compile it and place it in your $PATH, then open a C++ source file with Rerun (`rerun file.cpp`).

USAGE:
  rerun-loader-cpp-file [OPTIONS] FILEPATH

FLAGS:
  -h, --help                    Prints help information

OPTIONS:
  --recording-id RECORDING_ID   ID of the shared recording

ARGS:
  <FILEPATH>
)";

int main(int argc, char* argv[]) {
    // The Rerun Viewer will always pass these two pieces of information:
    // 1. The path to be loaded, as a positional arg.
    // 2. A shared recording ID, via the `--recording-id` flag.
    //
    // It is up to you whether you make use of that shared recording ID or not.
    // If you use it, the data will end up in the same recording as all other plugins interested in
    // that file, otherwise you can just create a dedicated recording for it. Or both.
    std::string filepath;
    std::string recording_id;

    for (int i = 1; i < argc; ++i) {
        std::string arg(argv[i]);

        if (arg == "--recording-id") {
            if (i + 1 < argc) {
                recording_id = argv[i + 1];
                ++i;
            } else {
                std::cerr << USAGE << std::endl;
                return 1;
            }
        } else {
            filepath = arg;
        }
    }

    if (filepath.empty()) {
        std::cerr << USAGE << std::endl;
        return 1;
    }

    bool is_file = std::filesystem::is_regular_file(filepath);
    bool is_cpp_file = std::filesystem::path(filepath).extension().string() == ".cpp";

    // We're not interested: just exit silently.
    // Don't return an error, as that would show up to the end user in the Rerun Viewer!
    if (!(is_file && is_cpp_file)) {
        return 0;
    }

    std::ifstream file(filepath);
    std::stringstream body;
    body << file.rdbuf();

    std::string text = "## Some C++ code\n```cpp\n" + body.str() + "\n```\n";

    const auto rec = rerun::RecordingStream("rerun_example_external_data_loader", recording_id);
    // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
    rec.stdout().exit_on_failure();

    rec.log_timeless(
        filepath,
        rerun::TextDocument(text).with_media_type(rerun::MediaType::markdown())
    );
}
