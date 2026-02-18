// Create and log a bar chart.

#include <rerun.hpp>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_bar_chart");
    rec.spawn().exit_on_failure();

    rec.log("bar_chart", rerun::BarChart::i64({8, 4, 0, 9, 1, 4, 1, 6, 9, 0}));

    auto abscissa = std::vector<int64_t>{0, 1, 3, 4, 7, 11};
    auto abscissa_data = rerun::TensorData(rerun::Collection{abscissa.size()}, abscissa);
    rec.log(
        "bar_chart_custom_abscissa",
        rerun::BarChart::i64({8, 4, 0, 9, 1, 4}).with_abscissa(abscissa_data)
    );

    auto widths = std::vector<float>{1, 2, 1, 3, 4, 1};
    rec.log(
        "bar_chart_custom_abscissa_and_widths",
        rerun::BarChart::i64({8, 4, 0, 9, 1, 4}).with_abscissa(abscissa_data).with_widths(widths)
    );
}
