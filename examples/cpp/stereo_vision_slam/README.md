<!--[metadata]
title = "Stereo vision SLAM"
source = "https://github.com/rerun-io/StereoVision-SLAM"
tags = ["3D", "Point cloud", "C++"]
thumbnail = "https://static.rerun.io/stereovision_slam/c36cfcf8bc7ec9f03b40559d596d7fee97907ba8/480w.png"
thumbnail_dimensions = [480, 273]
-->

Visualizes stereo vision SLAM on the [KITTI dataset](https://www.cvlibs.net/datasets/kitti/).

<picture>
  <img src="https://static.rerun.io/stereovision_slam_full/675db4870c12da348552ac9bcdf4c60228d77322/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/stereovision_slam_full/675db4870c12da348552ac9bcdf4c60228d77322/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/stereovision_slam_full/675db4870c12da348552ac9bcdf4c60228d77322/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/stereovision_slam_full/675db4870c12da348552ac9bcdf4c60228d77322/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/stereovision_slam_full/675db4870c12da348552ac9bcdf4c60228d77322/1200w.png">
</picture>

# Used Rerun types

[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`LineStrips3D`](https://rerun.io/docs/reference/types/archetypes/line_strips3d), [`Scalar`](https://rerun.io/docs/reference/types/archetypes/scalar), [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://rerun.io/docs/reference/types/archetypes/pinhole), [`Points3D`](https://rerun.io/docs/reference/types/archetypes/points3d), [`TextLog`](https://rerun.io/docs/reference/types/archetypes/text_log)


# Background

This example shows [farhad-dalirani's stereo visual SLAM implementation](https://github.com/farhad-dalirani/StereoVision-SLAM). It's input is the video footage from a stereo camera and it produces the trajectory of the vehicle and a point cloud of the surrounding environment.

# Logging and visualizing with Rerun

To easily use Opencv/Eigen types and avoid copying images/points when logging to Rerun it uses [`CollectionAdapter`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1CollectionAdapter.html) with the following code:
```cpp

template <>
struct rerun::CollectionAdapter<uint8_t, cv::Mat>
{
    /* Adapters to borrow an OpenCV image into Rerun
     * images without copying */

    Collection<uint8_t> operator()(const cv::Mat& img)
    {
        // Borrow for non-temporary.

        assert("OpenCV matrix expected have bit depth CV_U8" && CV_MAT_DEPTH(img.type()) == CV_8U);
        return Collection<uint8_t>::borrow(img.data, img.total() * img.channels());
    }

    Collection<uint8_t> operator()(cv::Mat&& img)
    {
        /* Do a full copy for temporaries (otherwise the data
         * might be deleted when the temporary is destroyed). */

        assert("OpenCV matrix expected have bit depth CV_U8" && CV_MAT_DEPTH(img.type()) == CV_8U);
        std::vector<uint8_t> img_vec(img.total() * img.channels());
        img_vec.assign(img.data, img.data + img.total() * img.channels());
        return Collection<uint8_t>::take_ownership(std::move(img_vec));
    }
};


template <>
struct rerun::CollectionAdapter<rerun::Position3D, std::vector<Eigen::Vector3f>>
{
    /* Adapters to log eigen vectors as rerun positions*/

    Collection<rerun::Position3D> operator()(const std::vector<Eigen::Vector3f>& container)
    {
        // Borrow for non-temporary.
        return Collection<rerun::Position3D>::borrow(container.data(), container.size());
    }

    Collection<rerun::Position3D> operator()(std::vector<Eigen::Vector3f>&& container)
    {
        /* Do a full copy for temporaries (otherwise the data
         * might be deleted when the temporary is destroyed). */
        std::vector<rerun::Position3D> positions(container.size());
        memcpy(positions.data(), container.data(), container.size() * sizeof(Eigen::Vector3f));
        return Collection<rerun::Position3D>::take_ownership(std::move(positions));
    }
};


template <>
struct rerun::CollectionAdapter<rerun::Position3D, Eigen::Matrix3Xf>
{
    /* Adapters so we can log an eigen matrix as rerun positions */

    // Sanity check that this is binary compatible.
    static_assert(
        sizeof(rerun::Position3D) == sizeof(Eigen::Matrix3Xf::Scalar) * Eigen::Matrix3Xf::RowsAtCompileTime
    );

    Collection<rerun::Position3D> operator()(const Eigen::Matrix3Xf& matrix)
    {
        // Borrow for non-temporary.
        static_assert(alignof(rerun::Position3D) <= alignof(Eigen::Matrix3Xf::Scalar));
        return Collection<rerun::Position3D>::borrow(
            // Cast to void because otherwise Rerun will try to do above sanity checks with the wrong type (scalar).
            reinterpret_cast<const void*>(matrix.data()),
            matrix.cols()
        );
    }

    Collection<rerun::Position3D> operator()(Eigen::Matrix3Xf&& matrix)
    {
        /* Do a full copy for temporaries (otherwise the
         * data might be deleted when the temporary is destroyed). */
        std::vector<rerun::Position3D> positions(matrix.cols());
        memcpy(positions.data(), matrix.data(), matrix.size() * sizeof(rerun::Position3D));
        return Collection<rerun::Position3D>::take_ownership(std::move(positions));
    }
};

```

## Images
```cpp
// Draw stereo left image
rec.log(entity_name,
        rerun::Image(tensor_shape(kf_sort[0].second->left_img_),
                    rerun::TensorBuffer::u8(kf_sort[0].second->left_img_)));
```

## Pinhole camera

The camera frames shown in the view is generated by the following code:

```cpp
rec.log(entity_name,
    rerun::Transform3D(
        rerun::Vec3D(camera_position.data()),
        rerun::Mat3x3(camera_orientation.data()), true)
);
// …
rec.log(entity_name,
        rerun::Pinhole::from_focal_length_and_resolution({fx, fy}, {img_num_cols, img_num_rows}));
```

## Time series
```cpp
void Viewer::Plot(std::string plot_name, double value, unsigned long maxkeyframe_id)
{
    // …
    rec.set_time_sequence("max_keyframe_id", maxkeyframe_id);
    rec.log(plot_name, rerun::Scalar(value));
}
```

## Trajectory
```cpp
rec.log("world/path",
    rerun::Transform3D(
        rerun::Vec3D(camera_position.data()),
        rerun::Mat3x3(camera_orientation.data()), true));

std::vector<rerun::datatypes::Vec3D> path;
// …
rec.log("world/path", rerun::LineStrips3D(rerun::LineStrip3D(path)));
```

## Point cloud
```cpp
rec.log("world/landmarks",
    rerun::Transform3D(
        rerun::Vec3D(camera_position.data()),
        rerun::Mat3x3(camera_orientation.data()), true));

std::vector<Eigen::Vector3f> points3d_vector;
// …
rec.log("world/landmarks", rerun::Points3D(points3d_vector));
```

## Text log

```cpp
rec.log("world/log", rerun::TextLog(msg).with_color(log_color.at(log_type)));
// …
rec.log("world/log", rerun::TextLog("Finished"));
```

# Run the code

This is an external example, check the [repository](https://github.com/rerun-io/StereoVision-SLAM) on how to run the code.
