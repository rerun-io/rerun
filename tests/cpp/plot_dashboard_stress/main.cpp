// Plot dashboard stress test.
//
// Usage:
// ```text
// just cpp-plot-dashboard --help
// ```
//
// Example:
// ```text
// just cpp-plot-dashboard --num-plots 10 --num-series-per-plot 5 --num-points-per-series 5000 --freq 1000
// ```

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <random>
#include <thread>
#include <vector>

#include <cxxopts.hpp>
#include <rerun.hpp>

int main(int argc, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_plot_dashboard_stress");

    cxxopts::Options options("plot_dashboard_stress", "Plot dashboard stress test");

    // clang-format off
    options.add_options()
      ("h,help", "Print usage")
      // Rerun
      ("spawn", "Start a new Rerun Viewer process and feed it data in real-time")
      ("connect", "Connects and sends the logged data to a remote Rerun viewer")
      ("stdout", "Log data to standard output, to be piped into a Rerun Viewer")
      // Dashboard
      ("num-plots", "How many different plots?", cxxopts::value<uint64_t>()->default_value("1"))
      ("num-series-per-plot", "How many series in each single plot?", cxxopts::value<uint64_t>()->default_value("1"))
      ("num-points-per-series", "How many points in each single series?", cxxopts::value<uint64_t>()->default_value("100000"))
      ("freq", "Frequency of logging (applies to all series)", cxxopts::value<double>()->default_value("1000.0"))
    ("order", "What order to log the data in ('forwards', 'backwards', 'random') (applies to all series)", cxxopts::value<std::string>()->default_value("forwards"))
    ;
    // clang-format on

    auto args = options.parse(argc, argv);

    if (args.count("help")) {
        std::cout << options.help() << std::endl;
        exit(0);
    }

    // TODO(#4602): need common rerun args helper library
    if (args["spawn"].as<bool>()) {
        rec.spawn().exit_on_failure();
    } else if (args["connect"].as<bool>()) {
        rec.connect().exit_on_failure();
    } else if (args["stdout"].as<bool>()) {
        rec.to_stdout().exit_on_failure();
    } else {
        rec.spawn().exit_on_failure();
    }

    const auto num_plots = args["num-plots"].as<uint64_t>();
    const auto num_series_per_plot = args["num-series-per-plot"].as<uint64_t>();
    const auto num_points_per_series = args["num-points-per-series"].as<uint64_t>();

    std::vector<std::string> plot_paths;
    plot_paths.reserve(num_plots);
    for (uint64_t i = 0; i < num_plots; ++i) {
        plot_paths.push_back("plot_" + std::to_string(i));
    }

    std::vector<std::string> series_paths;
    series_paths.reserve(num_series_per_plot);
    for (uint64_t i = 0; i < num_series_per_plot; ++i) {
        series_paths.push_back("series_" + std::to_string(i));
    }

    const auto freq = args["freq"].as<double>();

    const auto num_series = num_plots * num_series_per_plot;
    const auto time_per_tick = 1.0 / freq;
    const auto expected_total_freq = freq * num_series;

    std::random_device rd;
    std::mt19937 rng(rd());
    std::uniform_real_distribution<double> uniform_pi(0.0, M_PI);

    std::vector<int64_t> sim_times;
    const auto order = args["order"].as<std::string>();

    if (order == "forwards") {
        for (uint64_t i = 0; i < num_points_per_series; ++i) {
            sim_times.push_back(i);
        }
    } else if (order == "backwards") {
        for (uint64_t i = num_points_per_series; i > 0; --i) {
            sim_times.push_back(i - 1);
        }
    } else if (order == "random") {
        for (uint64_t i = 0; i < num_points_per_series; ++i) {
            sim_times.push_back(i);
        }
        std::shuffle(sim_times.begin(), sim_times.end(), rng);
    }

    uint64_t total_num_scalars = 0;
    auto total_start_time = std::chrono::high_resolution_clock::now();
    double max_load = 0.0;

    auto tick_start_time = std::chrono::high_resolution_clock::now();

    for (auto sim_time : sim_times) {
        rec.set_time_sequence("sim_time", sim_time);

        // Log

        for (auto plot_path : plot_paths) {
            for (auto series_path : series_paths) {
                double value = std::sin(uniform_pi(rng));
                rec.log(plot_path + "/" + series_path, rerun::TimeSeriesScalar(value));
            }
        }

        // Progress report

        total_num_scalars += num_series;
        auto total_elapsed = std::chrono::high_resolution_clock::now() - total_start_time;
        if (total_elapsed >= std::chrono::seconds(1)) {
            double total_elapsed_secs =
                std::chrono::duration_cast<std::chrono::duration<double>>(total_elapsed).count();
            std::cout << "logged " << total_num_scalars << " scalars over " << total_elapsed_secs
                      << "s (freq=" << total_num_scalars / total_elapsed_secs
                      << "Hz, expected=" << expected_total_freq << "Hz, load=" << max_load * 100.0
                      << "%)" << std::endl;

            auto elapsed_debt = std::chrono::duration<double>(
                total_elapsed_secs - static_cast<uint64_t>(total_elapsed_secs)
            ); // just keep the fractional part
            total_start_time = std::chrono::high_resolution_clock::now() -
                               std::chrono::duration_cast<std::chrono::nanoseconds>(elapsed_debt);

            total_start_time = std::chrono::high_resolution_clock::now();
            total_num_scalars = 0;
            max_load = 0.0;
        }

        // Throttle

        auto elapsed = std::chrono::high_resolution_clock::now() - tick_start_time;
        double sleep_time =
            time_per_tick -
            std::chrono::duration_cast<std::chrono::duration<double>>(elapsed).count();

        if (sleep_time > 0.0) {
            auto sleep_duration = std::chrono::duration<double>(sleep_time);

            auto sleep_start_time = std::chrono::high_resolution_clock::now();
            std::this_thread::sleep_for(sleep_duration);
            auto sleep_elapsed = std::chrono::high_resolution_clock::now() - sleep_start_time;

            // We will very likely be put to sleep for more than we asked for, and therefore need
            // to pay off that debt in order to meet our frequency goal.
            auto sleep_debt = sleep_elapsed - sleep_duration;
            tick_start_time = std::chrono::high_resolution_clock::now() -
                              std::chrono::duration_cast<std::chrono::nanoseconds>(sleep_debt);
        } else {
            tick_start_time = std::chrono::high_resolution_clock::now();
        }

        max_load = std::max(
            max_load,
            std::chrono::duration_cast<std::chrono::duration<double>>(elapsed).count() /
                time_per_tick
        );
    }

    auto total_elapsed = std::chrono::high_resolution_clock::now() - total_start_time;
    double total_elapsed_secs =
        std::chrono::duration_cast<std::chrono::duration<double>>(total_elapsed).count();
    std::cout << "logged " << total_num_scalars << " scalars over " << total_elapsed_secs
              << "s (freq=" << total_num_scalars / total_elapsed_secs
              << "Hz, expected=" << expected_total_freq << "Hz, load=" << max_load * 100.0 << "%)"
              << std::endl;
}
