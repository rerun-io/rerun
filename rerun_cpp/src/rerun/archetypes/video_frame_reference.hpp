// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/video_frame_reference.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/entity_path.hpp"
#include "../components/video_timestamp.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: References a single video frame.
    ///
    /// Used to display individual video frames from a `archetypes::AssetVideo`.
    /// To show an entire video, a video frame reference for each frame of the video should be logged.
    ///
    /// See <https://rerun.io/docs/reference/video> for details of what is and isn't supported.
    ///
    /// ## Examples
    ///
    /// ### Video with automatically determined frames
    /// ![image](https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <iostream>
    ///
    /// using namespace std::chrono_literals;
    ///
    /// int main(int argc, char* argv[]) {
    ///     if (argc <2) {
    ///         // TODO(#7354): Only mp4 is supported for now.
    ///         std::cerr <<"Usage: " <<argv[0] <<" <path_to_video.[mp4]>" <<std::endl;
    ///         return 1;
    ///     }
    ///
    ///     const auto path = argv[1];
    ///
    ///     const auto rec = rerun::RecordingStream("rerun_example_asset_video_auto_frames");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Log video asset which is referred to by frame references.
    ///     auto video_asset = rerun::AssetVideo::from_file(path).value_or_throw();
    ///     rec.log_static("video", video_asset);
    ///
    ///     // Send automatically determined video frame timestamps.
    ///     std::vector<std::chrono::nanoseconds> frame_timestamps_ns =
    ///         video_asset.read_frame_timestamps_ns().value_or_throw();
    ///     // Note timeline values don't have to be the same as the video timestamps.
    ///     auto time_column =
    ///         rerun::TimeColumn::from_times("video_time", rerun::borrow(frame_timestamps_ns));
    ///
    ///     std::vector<rerun::components::VideoTimestamp> video_timestamps(frame_timestamps_ns.size());
    ///     for (size_t i = 0; i <frame_timestamps_ns.size(); i++) {
    ///         video_timestamps[i] = rerun::components::VideoTimestamp(frame_timestamps_ns[i]);
    ///     }
    ///     auto video_frame_reference_indicators =
    ///         rerun::ComponentColumn::from_indicators<rerun::VideoFrameReference>(
    ///             static_cast<uint32_t>(video_timestamps.size())
    ///         );
    ///
    ///     rec.send_columns(
    ///         "video",
    ///         time_column,
    ///         rerun::VideoFrameReference().with_many_timestamp(rerun::borrow(video_timestamps)).columns()
    ///     );
    /// }
    /// ```
    ///
    /// ### Demonstrates manual use of video frame references
    /// ![image](https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <iostream>
    ///
    /// using namespace std::chrono_literals;
    ///
    /// int main(int argc, char* argv[]) {
    ///     if (argc <2) {
    ///         // TODO(#7354): Only mp4 is supported for now.
    ///         std::cerr <<"Usage: " <<argv[0] <<" <path_to_video.[mp4]>" <<std::endl;
    ///         return 1;
    ///     }
    ///
    ///     const auto path = argv[1];
    ///
    ///     const auto rec = rerun::RecordingStream("rerun_example_asset_video_manual_frames");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Log video asset which is referred to by frame references.
    ///     rec.log_static("video_asset", rerun::AssetVideo::from_file(path).value_or_throw());
    ///
    ///     // Create two entities, showing the same video frozen at different times.
    ///     rec.log("frame_1s", rerun::VideoFrameReference(1.0s).with_video_reference("video_asset"));
    ///     rec.log("frame_2s", rerun::VideoFrameReference(2.0s).with_video_reference("video_asset"));
    ///
    ///     // TODO(#5520): log blueprint once supported
    /// }
    /// ```
    struct VideoFrameReference {
        /// References the closest video frame to this timestamp.
        ///
        /// Note that this uses the closest video frame instead of the latest at this timestamp
        /// in order to be more forgiving of rounding errors for inprecise timestamp types.
        ///
        /// Timestamps are relative to the start of the video, i.e. a timestamp of 0 always corresponds to the first frame.
        /// This is oftentimes equivalent to presentation timestamps (known as PTS), but in the presence of B-frames
        /// (bidirectionally predicted frames) there may be an offset on the first presentation timestamp in the video.
        std::optional<ComponentBatch> timestamp;

        /// Optional reference to an entity with a `archetypes::AssetVideo`.
        ///
        /// If none is specified, the video is assumed to be at the same entity.
        /// Note that blueprint overrides on the referenced video will be ignored regardless,
        /// as this is always interpreted as a reference to the data store.
        ///
        /// For a series of video frame references, it is recommended to specify this path only once
        /// at the beginning of the series and then rely on latest-at query semantics to
        /// keep the video reference active.
        std::optional<ComponentBatch> video_reference;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.VideoFrameReferenceIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.VideoFrameReference";

        /// `ComponentDescriptor` for the `timestamp` field.
        static constexpr auto Descriptor_timestamp = ComponentDescriptor(
            ArchetypeName, "timestamp",
            Loggable<rerun::components::VideoTimestamp>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `video_reference` field.
        static constexpr auto Descriptor_video_reference = ComponentDescriptor(
            ArchetypeName, "video_reference",
            Loggable<rerun::components::EntityPath>::Descriptor.component_name
        );

      public:
        VideoFrameReference() = default;
        VideoFrameReference(VideoFrameReference&& other) = default;
        VideoFrameReference(const VideoFrameReference& other) = default;
        VideoFrameReference& operator=(const VideoFrameReference& other) = default;
        VideoFrameReference& operator=(VideoFrameReference&& other) = default;

        explicit VideoFrameReference(rerun::components::VideoTimestamp _timestamp)
            : timestamp(ComponentBatch::from_loggable(std::move(_timestamp), Descriptor_timestamp)
                            .value_or_throw()) {}

        /// Update only some specific fields of a `VideoFrameReference`.
        static VideoFrameReference update_fields() {
            return VideoFrameReference();
        }

        /// Clear all the fields of a `VideoFrameReference`.
        static VideoFrameReference clear_fields();

        /// References the closest video frame to this timestamp.
        ///
        /// Note that this uses the closest video frame instead of the latest at this timestamp
        /// in order to be more forgiving of rounding errors for inprecise timestamp types.
        ///
        /// Timestamps are relative to the start of the video, i.e. a timestamp of 0 always corresponds to the first frame.
        /// This is oftentimes equivalent to presentation timestamps (known as PTS), but in the presence of B-frames
        /// (bidirectionally predicted frames) there may be an offset on the first presentation timestamp in the video.
        VideoFrameReference with_timestamp(const rerun::components::VideoTimestamp& _timestamp) && {
            timestamp =
                ComponentBatch::from_loggable(_timestamp, Descriptor_timestamp).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `timestamp` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_timestamp` should
        /// be used when logging a single row's worth of data.
        VideoFrameReference with_many_timestamp(
            const Collection<rerun::components::VideoTimestamp>& _timestamp
        ) && {
            timestamp =
                ComponentBatch::from_loggable(_timestamp, Descriptor_timestamp).value_or_throw();
            return std::move(*this);
        }

        /// Optional reference to an entity with a `archetypes::AssetVideo`.
        ///
        /// If none is specified, the video is assumed to be at the same entity.
        /// Note that blueprint overrides on the referenced video will be ignored regardless,
        /// as this is always interpreted as a reference to the data store.
        ///
        /// For a series of video frame references, it is recommended to specify this path only once
        /// at the beginning of the series and then rely on latest-at query semantics to
        /// keep the video reference active.
        VideoFrameReference with_video_reference(
            const rerun::components::EntityPath& _video_reference
        ) && {
            video_reference =
                ComponentBatch::from_loggable(_video_reference, Descriptor_video_reference)
                    .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `video_reference` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_video_reference` should
        /// be used when logging a single row's worth of data.
        VideoFrameReference with_many_video_reference(
            const Collection<rerun::components::EntityPath>& _video_reference
        ) && {
            video_reference =
                ComponentBatch::from_loggable(_video_reference, Descriptor_video_reference)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::VideoFrameReference> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const archetypes::VideoFrameReference& archetype
        );
    };
} // namespace rerun
