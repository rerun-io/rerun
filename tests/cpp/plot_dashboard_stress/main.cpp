// Plot dashboard stress test.
//
// Usage:
// ```text
// pixi run -e cpp cpp-plot-dashboard --help
// ```
//
// Example:
// ```text
// pixi run -e cpp cpp-plot-dashboard --num-plots 10 --num-series-per-plot 5 --num-points-per-series 5000 --freq 1000
// ```

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <numeric>
#include <random>
#include <thread>
#include <vector>

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>
#include <rerun/third_party/cxxopts.hpp>

int main(int argc, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_plot_dashboard_stress");

    cxxopts::Options options("plot_dashboard_stress", "Plot dashboard stress test");

    // clang-format off
    options.add_options()
      ("h,help", "Print usage")
      // Rerun
      ("spawn", "Start a new Rerun Viewer process and feed it data in real-time")
      ("connect", "Connects and sends the logged data to a remote Rerun viewer")
      ("save", "Log data to an rrd file", cxxopts::value<std::string>())
      ("stdout", "Log data to standard output, to be piped into a Rerun Viewer")
      // Dashboard
      ("num-plots", "How many different plots?", cxxopts::value<uint64_t>()->default_value("1"))
      ("num-series-per-plot", "How many series in each single plot?", cxxopts::value<uint64_t>()->default_value("1"))
      ("num-points-per-series", "How many points in each single series?", cxxopts::value<uint64_t>()->default_value("100000"))
      ("freq", "Frequency of logging (applies to all series)", cxxopts::value<double>()->default_value("1000.0"))
      ("temporal-batch-size", "Number of rows to include in each log call", cxxopts::value<uint64_t>())
    ("order", "What order to log the data in ('forwards', 'backwards', 'random') (applies to all series).", cxxopts::value<std::string>()->default_value("forwards"))
    ("series-type", "The method used to generate time series ('gaussian-random-walk', 'sin-uniform').", cxxopts::value<std::string>()->default_value("gaussian-random-walk"))
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
        rec.connect_grpc().exit_on_failure();
    } else if (args["stdout"].as<bool>()) {
        rec.to_stdout().exit_on_failure();
    } else if (args.count("save")) {
        rec.save(args["save"].as<std::string>()).exit_on_failure();
    } else {
        rec.spawn().exit_on_failure();
    }

    const auto num_plots = args["num-plots"].as<uint64_t>();
    const auto num_series_per_plot = args["num-series-per-plot"].as<uint64_t>();
    const auto num_points_per_series = args["num-points-per-series"].as<uint64_t>();
    const auto temporal_batch_size = args.count("temporal-batch-size")
                                         ? std::optional(args["temporal-batch-size"].as<uint64_t>())
                                         : std::nullopt;

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

    auto time_per_sim_step = 1.0 / freq;

    std::random_device rd;
    std::mt19937 rng(rd());
    std::uniform_real_distribution<double> distr_uniform_pi(0.0, rerun::demo::PI);
    std::normal_distribution<double> distr_std_normal;

    std::vector<double> sim_times;
    const auto order = args["order"].as<std::string>();
    const auto series_type = args["series-type"].as<std::string>();

    if (order == "forwards") {
        for (int64_t i = 0; i < static_cast<int64_t>(num_points_per_series); ++i) {
            sim_times.push_back(static_cast<double>(i) * time_per_sim_step);
        }
    } else if (order == "backwards") {
        for (int64_t i = static_cast<int64_t>(num_points_per_series); i > 0; --i) {
            sim_times.push_back(static_cast<double>(i - 1) * time_per_sim_step);
        }
    } else if (order == "random") {
        for (int64_t i = 0; i < static_cast<int64_t>(num_points_per_series); ++i) {
            sim_times.push_back(static_cast<double>(i) * time_per_sim_step);
        }
        std::shuffle(sim_times.begin(), sim_times.end(), rng);
    }

    const auto num_series = num_plots * num_series_per_plot;
    auto time_per_tick = 1.0 / freq;
    auto scalars_per_tick = num_series;
    if (temporal_batch_size.has_value()) {
        time_per_tick *= static_cast<double>(*temporal_batch_size);
        scalars_per_tick *= *temporal_batch_size;
    }
    const auto expected_total_freq = freq * static_cast<double>(num_series);

    std::vector<std::vector<double>> values_per_series;
    for (uint64_t series_idx = 0; series_idx < num_series; ++series_idx) {
        std::vector<double> values;

        double value = 0.0;
        for (uint64_t i = 0; i < num_points_per_series; ++i) {
            if (series_type == "gaussian-random-walk") {
                value += distr_std_normal(rng);
            } else if (series_type == "sin-uniform") {
                value = distr_uniform_pi(rng);
            } else {
                // Just generate random numbers rather than crash
                value = distr_std_normal(rng);
            }

            values.push_back(value);
        }

        values_per_series.push_back(values);
    }

    std::vector<size_t> offsets;
    if (temporal_batch_size.has_value()) {
        // GCC wrongfully thinks that `temporal_batch_size` is uninitialized despite being initialized upon creation.
        RR_DISABLE_MAYBE_UNINITIALIZED_PUSH
        for (size_t i = 0; i < num_points_per_series; i += *temporal_batch_size) {
            offsets.push_back(i);
        }
        RR_DISABLE_MAYBE_UNINITIALIZED_POP
    } else {
        offsets.resize(sim_times.size());
        std::iota(offsets.begin(), offsets.end(), 0);
    }

    uint64_t total_num_scalars = 0;
    auto total_start_time = std::chrono::high_resolution_clock::now();
    double max_load = 0.0;

    auto tick_start_time = std::chrono::high_resolution_clock::now();

    size_t time_step = 0;
    for (auto offset : offsets) {
        std::optional<rerun::TimeColumn> time_column;
        if (temporal_batch_size.has_value()) {
            time_column = rerun::TimeColumn::from_duration_seconds(
                "sim_time",
                rerun::borrow(sim_times.data() + offset, *temporal_batch_size),
                rerun::SortingStatus::Sorted
            );
        } else {
            rec.set_time_duration_secs("sim_time", sim_times[offset]);
        }

        // Log

        for (size_t plot_idx = 0; plot_idx < plot_paths.size(); ++plot_idx) {
            auto global_plot_idx = plot_idx * num_series_per_plot;

            for (size_t series_idx = 0; series_idx < series_paths.size(); ++series_idx) {
                auto path = plot_paths[plot_idx] + "/" + series_paths[series_idx];
                const auto& series_values = values_per_series[global_plot_idx + series_idx];
                if (temporal_batch_size.has_value()) {
                    rec.send_columns(
                        path,
                        *time_column,
                        rerun::Scalars(
                            rerun::borrow(series_values.data() + time_step, *temporal_batch_size)
                        )
                            .columns()
                    );
                } else {
                    rec.log(path, rerun::Scalars(series_values[time_step]));
                }
            }
        }
        if (temporal_batch_size.has_value()) {
            time_step += *temporal_batch_size;
        } else {
            ++time_step;
        }

        // Measure how long this took and how high the load was.

        auto elapsed = std::chrono::high_resolution_clock::now() - tick_start_time;
        max_load = std::max(
            max_load,
            std::chrono::duration_cast<std::chrono::duration<double>>(elapsed).count() /
                time_per_tick
        );

        // Throttle

        auto sleep_duration = std::chrono::duration<double>(time_per_tick) - elapsed;
        if (sleep_duration.count() > 0.0) {
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

        // Progress report
        //
        // Must come after throttle since we report every wall-clock second:
        // If ticks are large & fast, then after each send we run into throttle.
        // So if this was before throttle, we'd not report the first tick no matter how large it was.

        total_num_scalars += scalars_per_tick;
        auto total_elapsed = std::chrono::high_resolution_clock::now() - total_start_time;
        if (total_elapsed >= std::chrono::seconds(1)) {
            double total_elapsed_secs =
                std::chrono::duration_cast<std::chrono::duration<double>>(total_elapsed).count();
            std::cout << "logged " << total_num_scalars << " scalars over " << total_elapsed_secs
                      << "s (freq=" << static_cast<double>(total_num_scalars) / total_elapsed_secs
                      << "Hz, expected=" << expected_total_freq << "Hz, load=" << max_load * 100.0
                      << "%)" << std::endl;

            auto elapsed_debt = std::chrono::duration<double>(
                total_elapsed_secs - floor(total_elapsed_secs)
            ); // just keep the fractional part
            total_start_time = std::chrono::high_resolution_clock::now() -
                               std::chrono::duration_cast<std::chrono::nanoseconds>(elapsed_debt);
            total_num_scalars = 0;
            max_load = 0.0;
        }
    }

    if (total_num_scalars > 0) {
        auto total_elapsed = std::chrono::high_resolution_clock::now() - total_start_time;
        double total_elapsed_secs =
            std::chrono::duration_cast<std::chrono::duration<double>>(total_elapsed).count();
        std::cout << "logged " << total_num_scalars << " scalars over " << total_elapsed_secs
                  << "s (freq=" << static_cast<double>(total_num_scalars) / total_elapsed_secs
                  << "Hz, expected=" << expected_total_freq << "Hz, load=" << max_load * 100.0
                  << "%)" << std::endl;
    }
}
