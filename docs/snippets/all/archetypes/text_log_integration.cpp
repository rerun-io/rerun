/// Shows integration of Rerun's `TextLog` with the C++ Loguru logging library
/// (https://github.com/emilk/loguru).

#include <loguru.hpp>
#include <rerun.hpp>

void loguru_to_rerun(void* user_data, const loguru::Message& message) {
    // NOTE: `rerun::RecordingStream` is thread-safe.
    const rerun::RecordingStream* rec = reinterpret_cast<const rerun::RecordingStream*>(user_data);

    rerun::TextLogLevel level;
    if (message.verbosity == loguru::Verbosity_FATAL) {
        level = rerun::TextLogLevel::Critical;
    } else if (message.verbosity == loguru::Verbosity_ERROR) {
        level = rerun::TextLogLevel::Error;
    } else if (message.verbosity == loguru::Verbosity_WARNING) {
        level = rerun::TextLogLevel::Warning;
    } else if (message.verbosity == loguru::Verbosity_INFO) {
        level = rerun::TextLogLevel::Info;
    } else if (message.verbosity == loguru::Verbosity_1) {
        level = rerun::TextLogLevel::Debug;
    } else if (message.verbosity == loguru::Verbosity_2) {
        level = rerun::TextLogLevel::Trace;
    } else {
        level = rerun::TextLogLevel(std::to_string(message.verbosity));
    }

    rec->log(
        "logs/handler/text_log_integration",
        rerun::TextLog(message.message).with_level(level)
    );
}

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_text_log_integration");
    rec.spawn().exit_on_failure();

    // Log a text entry directly:
    rec.log(
        "logs",
        rerun::TextLog("this entry has loglevel TRACE").with_level(rerun::TextLogLevel::Trace)
    );

    loguru::add_callback(
        "rerun",
        loguru_to_rerun,
        const_cast<void*>(reinterpret_cast<const void*>(&rec)),
        loguru::Verbosity_INFO
    );

    LOG_F(INFO, "This INFO log got added through the standard logging interface");

    loguru::remove_callback("rerun"); // we need to do this before `rec` goes out of scope
}
