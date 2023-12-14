#include <iostream>
#include <string>

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_stdio");
    rec.stdout().exit_on_failure();

    std::string input;
    std::string line;
    while (std::getline(std::cin, line)) {
        input += line + '\n';
    }

    rec.log("stdin", rerun::TextDocument(input));
}
