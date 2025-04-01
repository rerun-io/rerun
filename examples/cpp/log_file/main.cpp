#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

#include <rerun.hpp>
#include <rerun/third_party/cxxopts.hpp>

int main(int argc, char** argv) {
    // Create a new `RecordingStream` which sends data over gRPC to the viewer process.
    const auto rec = rerun::RecordingStream("rerun_example_log_file");

    cxxopts::Options options(
        "rerun_example_log_file",
        "Demonstrates how to log any file from the SDK using the `DataLoader` machinery."
    );

    // clang-format off
    options.add_options()
      ("h,help", "Print usage")
      // Rerun
      ("spawn", "Start a new Rerun Viewer process and feed it data in real-time")
      ("connect", "Connects and sends the logged data to a remote Rerun viewer")
      ("save", "Log data to an rrd file", cxxopts::value<std::string>())
      ("stdout", "Log data to standard output, to be piped into a Rerun Viewer")
      // Example
      ("from-contents", "Log the contents of the file directly (files only -- not supported by external loaders)", cxxopts::value<bool>()->default_value("false"))
      ("filepaths", "The filepaths to be loaded and logged", cxxopts::value<std::vector<std::string>>())
    ;
    // clang-format on

    options.parse_positional({"filepaths"});

    auto args = options.parse(argc, argv);

    if (args.count("help")) {
        std::cout << options.help() << std::endl;
        exit(0);
    }

    // TODO(#4602): need common rerun args helper library
    if (args["spawn"].as<bool>()) {
        rec.spawn().exit_on_failure();
    } else if (args["connect"].as<bool>()) {
        rec.connect_grpc().exit_on_failure();
    } else if (args["stdout"].as<bool>()) {
        rec.to_stdout().exit_on_failure();
    } else if (args.count("save")) {
        rec.save(args["save"].as<std::string>()).exit_on_failure();
    } else {
        rec.spawn().exit_on_failure();
    }

    const auto from_contents = args["from-contents"].as<bool>();
    if (args.count("filepaths")) {
        const auto filepaths = args["filepaths"].as<std::vector<std::string>>();
        for (const auto& filepath : filepaths) {
            if (!from_contents) {
                // Either log the file using its path…
                rec.log_file_from_path(filepath, "log_file_example");
            } else {
                // …or using its contents if you already have them loaded for some reason.
                if (std::filesystem::is_regular_file(filepath)) {
                    std::ifstream file(filepath);
                    std::stringstream contents;
                    contents << file.rdbuf();

                    const auto data = contents.str();
                    rec.log_file_from_contents(
                        filepath,
                        reinterpret_cast<const std::byte*>(data.c_str()),
                        data.size(),
                        "log_file_example"
                    );
                }
            }
        }
    }
}
