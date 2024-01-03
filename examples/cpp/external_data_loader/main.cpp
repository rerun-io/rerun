#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

#include <rerun.hpp>

static const char* USAGE = R"(
This is an example executable data-loader plugin for the Rerun Viewer.
Any executable on your `$PATH` with a name that starts with `rerun-loader-` will be treated as an
external data-loader.

This particular one will log C++ source code files as markdown documents, and return a
special exit code to indicate that it doesn't support anything else.

To try it out, compile it and place it in your $PATH as `rerun-loader-cpp-file`, then open a C++ source
file with Rerun (`rerun file.cpp`).

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

    // Inform the Rerun Viewer that we do not support that kind of file.
    if (!is_file || is_cpp_file) {
        return rerun::EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE;
    }

    std::ifstream file(filepath);
    std::stringstream body;
    body << file.rdbuf();

    std::string text = "## Some C++ code\n```cpp\n" + body.str() + "\n```\n";

    const auto rec = rerun::RecordingStream("rerun_example_external_data_loader", recording_id);
    // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
    rec.to_stdout().exit_on_failure();

    rec.log_timeless(
        filepath,
        rerun::TextDocument(text).with_media_type(rerun::MediaType::markdown())
    );
}
