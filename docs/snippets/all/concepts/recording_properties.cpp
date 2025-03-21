// Sets the recording properties.

#include <rerun.hpp>

#include <arrow/array/builder_binary.h>
#include <arrow/array/builder_primitive.h>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_recording_properties");
    rec.spawn().exit_on_failure();

    // Overwrites the name from above.
    rec.send_recording_name("My recording");

    // Start time is set automatically, but we can overwrite it at any time.
    rec.send_recording_start_time_nanos(1742539110661000000);

    auto points = rerun::Points3D({{1.0, 0.1, 1.0}});
    rec.send_property("camera_left", points);

    std::shared_ptr<arrow::Array> arrow_array;

    arrow::DoubleBuilder confidences_builder;
    ARROW_RETURN_NOT_OK(confidences_builder.AppendValues({0.3, 0.4, 0.5, 0.6}));
    ARROW_RETURN_NOT_OK(confidences_builder.Finish(&arrow_array));
    auto confidences =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "confidence");

    arrow::StringBuilder traffic_builder;
    ARROW_RETURN_NOT_OK(traffic_builder.Append("low"));
    ARROW_RETURN_NOT_OK(traffic_builder.Finish(&arrow_array));
    auto traffic = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("traffic")
    );

    arrow::StringBuilder weather_builder;
    ARROW_RETURN_NOT_OK(weather_builder.Append("sunny"));
    ARROW_RETURN_NOT_OK(weather_builder.Finish(&arrow_array));
    auto weather = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("weather")
    );

    rec.send_property("situation", confidences, traffic, weather);

    // Properties, including the name, can be overwritten at any time.
    rec.send_recording_name("My episode");

    return arrow::Status::OK();
}

int main() {
    arrow::Status status = run_main();
    if (!status.ok()) {
        printf("%s\n", status.ToString().c_str());
        return 1;
    }
    return 0;
}
