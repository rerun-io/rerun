#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <sstream>
#include <string>

#include <rerun.hpp>
#include <rerun/third_party/cxxopts.hpp>
#include <string_view>

void set_time_from_args(const rerun::RecordingStream& rec, cxxopts::ParseResult& args) {
    if (args.count("time_sequence")) {
        const auto sequences = args["time_sequence"].as<std::vector<std::string>>();
        for (const auto& sequence_str : sequences) {
            auto pos = sequence_str.find('=');
            if (pos != std::string::npos) {
                auto timeline_name = sequence_str.substr(0, pos);
                int64_t sequence = std::stol(sequence_str.substr(pos + 1));
                rec.set_time_sequence(timeline_name, sequence);
            }
        }
    }
    if (args.count("time_duration_ns")) {
        const auto times = args["time_duration_ns"].as<std::vector<std::string>>();
        for (const auto& time_str : times) {
            auto pos = time_str.find('=');
            if (pos != std::string::npos) {
                auto timeline_name = time_str.substr(0, pos);
                int64_t time = std::stol(time_str.substr(pos + 1));
                rec.set_time_duration_nanos(timeline_name, time);
            }
        }
    }
    if (args.count("time_timestamp_ns")) {
        const auto times = args["time_timestamp_ns"].as<std::vector<std::string>>();
        for (const auto& time_str : times) {
            auto pos = time_str.find('=');
            if (pos != std::string::npos) {
                auto timeline_name = time_str.substr(0, pos);
                int64_t time = std::stol(time_str.substr(pos + 1));
                rec.set_time_timestamp_nanos_since_epoch(timeline_name, time);
            }
        }
    }
}

int main(int argc, char* argv[]) {
    // The Rerun Viewer will always pass these two pieces of information:
    // 1. The path to be loaded, as a positional arg.
    // 2. A shared recording ID, via the `--recording-id` flag.
    //
    // It is up to you whether you make use of that shared recording ID or not.
    // If you use it, the data will end up in the same recording as all other plugins interested in
    // that file, otherwise you can just create a dedicated recording for it. Or both.
    //
    // Check out `re_data_source::DataLoaderSettings` documentation for an exhaustive listing of
    // the available CLI parameters.

    cxxopts::Options options(
        "rerun-loader-cpp-file",
        R"(
This is an example executable data-loader plugin for the Rerun Viewer.
Any executable on your `$PATH` with a name that starts with `rerun-loader-` will be treated as an
external data-loader.

This particular one will log C++ source code files as markdown documents, and return a
special exit code to indicate that it doesn't support anything else.

To try it out, compile it and place it in your $PATH as `rerun-loader-cpp-file`, then open a C++ source
file with Rerun (`rerun file.cpp`).
)"
    );

    // clang-format off
    options.add_options()
      ("h,help", "Print usage")
      ("filepath", "The filepath to be loaded and logged", cxxopts::value<std::string>())
      ("application-id", "Optional recommended ID for the application", cxxopts::value<std::string>())
      ("recording-id", "Optional recommended ID for the recording", cxxopts::value<std::string>())
      ("entity-path-prefix", "Optional prefix for all entity paths", cxxopts::value<std::string>())
      ("static", "Optionally mark data to be logged as static", cxxopts::value<bool>()->default_value("false"))
      ("time_sequence", "Optional sequences to log at (e.g. `--time_sequence sim_frame=42`) (repeatable)", cxxopts::value<std::vector<std::string>>())
      ("time_duration_ns", "Optional durations (nanoseconds) to log at (e.g. `--time_duration_ns sim_time=123`) (repeatable)", cxxopts::value<std::vector<std::string>>())
      ("time_timestamp_ns", "Optional timestamps (nanos since epoch) to log at (e.g. `--time_timestamp_ns sim_time=1709203426123456789`) (repeatable)", cxxopts::value<std::vector<std::string>>())
    ;
    // clang-format on

    options.parse_positional({"filepath"});

    auto args = options.parse(argc, argv);

    if (args.count("help")) {
        std::cout << options.help() << std::endl;
        exit(0);
    }

    const auto filepath = args["filepath"].as<std::string>();

    bool is_file = std::filesystem::is_regular_file(filepath);
    bool is_cpp_file = std::filesystem::path(filepath).extension().string() == ".cpp";

    // Inform the Rerun Viewer that we do not support that kind of file.
    if (!is_file || !is_cpp_file) {
        return rerun::EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE;
    }

    std::ifstream file(filepath);
    std::stringstream body;
    body << file.rdbuf();

    std::string text = "## Some C++ code\n```cpp\n" + body.str() + "\n```\n";

    auto application_id = std::string_view("rerun_example_external_data_loader");
    if (args.count("application-id")) {
        application_id = args["application-id"].as<std::string>();
    }
    auto recording_id = std::string_view();
    if (args.count("recording-id")) {
        recording_id = args["recording-id"].as<std::string>();
    }
    const auto rec = rerun::RecordingStream(application_id, recording_id);
    // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
    rec.to_stdout().exit_on_failure();

    set_time_from_args(rec, args);

    auto entity_path = std::string(filepath);
    if (args.count("entity-path-prefix")) {
        entity_path = args["entity-path-prefix"].as<std::string>() + "/" + filepath;
    }
    rec.log_with_static(
        entity_path,
        args["static"].as<bool>(),
        rerun::TextDocument(text).with_media_type(rerun::MediaType::markdown())
    );
}
