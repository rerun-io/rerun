<!--[metadata]
title = "MCAP"
tags = ["MCAP", "RRD", "ROS", "ROS 2", "Rosbag", "Tutorial"]
source = "https://github.com/rerun-io/mcap_example
thumbnail = "https://static.rerun.io/mcap_example/7a3207652fa411979a96d5c5a25a43be29f1fdfb/480w.png"
thumbnail_dimensions = [480, 305]
-->

https://vimeo.com/1152501098?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2864:1840

## Used Rerun types

[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`GeoPoints`](https://www.rerun.io/docs/reference/types/archetypes/geo_points), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`EncodedImage`](https://www.rerun.io/docs/reference/types/archetypes/encoded_image), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars)

## Background

This example demonstrates how to visualize and work with [MCAP](https://mcap.dev/) files in Rerun. From [mcap.dev](https://mcap.dev/):

> MCAP (pronounced "em-cap") is an open source container file format for multimodal log data. It supports multiple channels of timestamped pre-serialized data, and is ideal for use in pub/sub or robotics applications.

MCAP is the default bag format in ROS 2 and is rapidly gaining adoption. You can read more about [Rerun's MCAP support here](https://rerun.io/docs/howto/mcap).

In this guide, you will learn:

1. How to **load MCAP files** directly into the Rerun viewer.
2. How to **convert MCAP files** into native Rerun data files (**RRD**).
3. How to **convert older ROS bags** (ROS 1 and ROS 2 SQLite3) into MCAP.
4. How to read and deserialize MCAP/RRD data in Python for programmatic processing and advanced visualization in Rerun.

We will use a dataset from the [JKK Research Center](https://jkk-research.github.io/dataset/jkk_dataset_01/) containing LiDAR, images, GPS, and IMU data. The workflow involves converting the original ROS 1 bag → MCAP → RRD, and then using Python to log the RRD data with specific Rerun components for optimal visualization.

## Useful resources

Below you will find a collection of useful Rerun resources for this example:

* [Blog post introducing MCAP support](https://rerun.io/blog/introducing-experimental-support-for-mcap-file-format)
* MCAP
  * [Working with MCAP](https://rerun.io/docs/howto/mcap)
  * [Supported message formats](https://rerun.io/docs/reference/mcap/message-formats)
  * [MCAP layers explained](https://rerun.io/docs/reference/mcap/layers-explained)
  * [CLI reference for MCAP](https://rerun.io/docs/reference/mcap/cli-reference)
* Logging API
  * [Send from Python](https://rerun.io/docs/getting-started/data-in/python)
  * [Send entire columns at once](https://rerun.io/docs/howto/logging/send-columns)
* Dataframe
  * [Dataframes](https://rerun.io/docs/reference/dataframes)
  * [Query data out of Rerun](https://rerun.io/docs/howto/get-data-out)
* Blueprints
  * [Building blueprints programmatically](https://rerun.io/docs/howto/build-a-blueprint-programmatically)

### ROS message <-> Rerun archetype

In the table below, the mapping between some ROS messages types and some Rerun archetypes is presented. In a RRD file, you may find the fields of a Rerun archetype spread across multiple columns. So you could have, e.g., a `Points3D:positions` column in the RRD file. This would then correspond to the positions field of the [Points3D](https://rerun.io/docs/reference/types/archetypes/points3d) archetype.

| ROS message                                                                                                   | Rerun archtype                                                                       | Rerun fields                                                |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------- |
| [geometry_msgs/msg/PoseStamped](https://docs.ros.org/en/jazzy/p/geometry_msgs/msg/PoseStamped.html)           | [InstancePoses3D](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) | translations, quaternions                                   |
| [geometry_msgs/msg/TransformStamped](https://docs.ros.org/en/jazzy/p/geometry_msgs/msg/TransformStamped.html) | [Transform3D](https://rerun.io/docs/reference/types/archetypes/transform3d)          | child_frame, parent_frame, quaternion, translation          |
| frame_id                                                                                                      | [CoordinateFrame](https://rerun.io/docs/reference/types/archetypes/coordinate_frame) | frame                                                       |
| [sensor_msgs/msg/CameraInfo](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/CameraInfo.html)                 | [Pinhole](https://rerun.io/docs/reference/types/archetypes/pinhole)                  | child_frame, image_from_camera, parent_frame, resolution    |
| [sensor_msgs/msg/CompressedImage](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/CompressedImage.html)       | [EncodedImage](https://rerun.io/docs/reference/types/archetypes/encoded_image)       | blob                                                        |
| [sensor_msgs/msg/Imu](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/Imu.html)                               | [Scalars](https://rerun.io/docs/reference/types/archetypes/scalars)                  | scalars                                                     |
| [sensor_msgs/msg/NavSatFix](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/NavSatFix.html)                   | [GeoPoints](https://rerun.io/docs/reference/types/archetypes/geo_points)             | positions                                                   |
| [sensor_msgs/msg/PointCloud2](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/PointCloud2.html)               | [Points3D](https://rerun.io/docs/reference/types/archetypes/points3d)                | ambient, intensity, positions, range, reflectivity, ring, t |

## Visualize MCAP files with Rerun

Rerun supports visualizing MCAP files in several ways:

### Directly in the viewer (drag-and-drop)

You can drag-and-drop MCAP files directly into the Rerun viewer or use the `File > Open` menu option.

### From the command-line interface (CLI)

Load the file directly when starting the Rerun viewer:

```sh
rerun recording.mcap
```

### From code

You can also load an MCAP file from code, for example in Python you initialize Rerun and load the file path:

```python
import rerun as rr

rr.init("mcap_example/load_mcap", spawn=True)
rr.log_file_from_path("recording.mcap")
```

## Convert ROS bags to MCAP

If you have older ROS (1) bags or ROS 2 SQLite3 (`.db`) bags, you can convert them to MCAP using available libraries.

### Option 1: using Rosbag2 (requires ROS 2 installation)

Convert ROS 2 SQLite3 (`.db`) bags using the [Rosbag2](https://github.com/ros2/rosbag2?tab=readme-ov-file#convert) `ros2 bag convert` command:

```sh
ros2 bag convert -i ros2_bag -o out.yaml
```

where `out.yaml` may contain something like:

```yaml
output_bags:
- uri: ros2_mcap_bag
  storage_id: mcap
  all_topics: true
  all_services: true
```

### Option 2: using Rosbags (pure Python library)

[Rosbags](https://ternaris.gitlab.io/rosbags/) is a pure Python solution for converting, reading, and writing ROS 1 and ROS 2 bags without requiring a full ROS installation. To install:

```sh
pip install rosbags
```

Using the CLI, you can convert a ROS (1) or ROS 2 SQLite3 (`.db`) bag to MCAP by running:

```sh
# For ROS (1) bag
rosbags-convert --dst-storage mcap --src input.bag --dst output
# For ROS 2 SQLite3 bag
rosbags-convert --dst-storage mcap --src input --dst output
```

You can run:

```sh
rosbags-convert --help
```

to see all the options.

### Option 3: using the MCAP CLI tool

The [MCAP command line tool](https://mcap.dev/guides/cli) also does not require a ROS installation:

```sh
# For ROS(1) bag
mcap convert input.bag output.mcap
# For ROS 2 SQLite3 bag
mcap convert input.db3 output.mcap
```

## Convert MCAP to native Rerun file (RRD)

Converting an MCAP file to RRD enables more Rerun capabilities and provides faster loading. Use the CLI `mcap convert` command:

```sh
rerun mcap convert input.mcap -o output.rrd
```

You can also convert specific **layers** of the MCAP file:

```sh
# Use only specific layers
rerun mcap convert input.mcap -l stats -o output.rrd

# Use multiple layers for different perspectives
rerun mcap convert input.mcap -l ros2msg -l raw -l recording_info -o output.rrd
```

Read more about [MCAP layers here](https://rerun.io/docs/reference/mcap/layers-explained).

You can see the options for the `mcap convert` command by running:

```sh
rerun mcap convert --help
```

## Work with MCAP/RRD files in Python

For advanced processing and visualization with Rerun, we recommend converting the MCAP file to an **RRD file first**.

During the conversion from MCAP to RRD, Rerun automatically interprets common ROS messages and converts them into native Rerun types. This allows you to use Rerun's [data-loaders](https://rerun.io/docs/reference/data-loaders/overview) to easily retrieve structures like `Points3D` and `EncodedImage` instead of processing raw binary blobs.

**Tips**: You can use the `rerun rrd stats [PATH_TO_INPUT_RRDS]` to see what entities and components are in a RRD file.

> For details on accessing data from the original bag format, you can refer to the documentation for [Rosbag2](https://docs.ros.org/en/rolling/Tutorials/Advanced/Reading-From-A-Bag-File-Python.html), [Rosbags](https://ternaris.gitlab.io/rosbags/topics/highlevel.html#all-in-one-reader), or the [MCAP library](https://mcap.dev/docs/python/raw_reader_writer_example#reading-messages) itself.

## Example: JKK research dataset

This example demonstrates the full ROS 1 Bag -> MCAP -> RRD -> Rerun visualization workflow.

### 1. Setup

First you will need to clone this repo:

```sh
git clone https://github.com/rerun-io/mcap_example.git
cd mcap_example
```

We assume you are running all commands within the `mcap_example` repository folder and using the [Pixi](https://pixi.sh/latest/#installation) package manager for environment setup.

### 2. Download dataset

Download the ROS 1 bag file (`leaf-2022-03-18-gyor.bag`) from the [JKK Research Center](https://jkk-research.github.io/dataset/jkk_dataset_01/):

```sh
wget https://laesze-my.sharepoint.com/:u:/g/personal/herno_o365_sze_hu/EVlk6YgDtj9BrzIE8djt-rwBZ47q9NwcbgxU_zOuBji9IA?download=1 -O leaf-2022-03-18-gyor.bag
```

This dataset includes camera images, LiDAR point clouds, poses, IMU, and GPS.

### 3. Convert to MCAP (via Rosbags)

Use [Rosbags's](https://ternaris.gitlab.io/rosbags/) `rosbags-convert` tool within the Pixi environment to convert the ROS 1 bag to MCAP:

```sh
pixi run rosbags-convert --dst-storage mcap --src leaf-2022-03-18-gyor.bag --dst leaf-2022-03-18-gyor
```

**NOTE**: `pixi run` in the above command means that we are running the `rosbags-convert` command inside the Pixi environment where we have installed the [Rosbags](https://ternaris.gitlab.io/rosbags/) library.

This creates a folder `leaf-2022-03-18-gyor` containing the `leaf-2022-03-18-gyor.mcap` file. You can now visualize it:

```sh
pixi run rerun leaf-2022-03-18-gyor/leaf-2022-03-18-gyor.mcap
```

or drag the file into Rerun.

### 4. Convert to RRD

Next, convert the MCAP file into a native Rerun file (RRD):

```sh
pixi run rerun mcap convert leaf-2022-03-18-gyor/leaf-2022-03-18-gyor.mcap -o leaf-2022-03-18-gyor.rrd
```

Try opening the RRD file directly:

```sh
pixi run rerun leaf-2022-03-18-gyor.rrd
```

or drag the file into Rerun.

### 5. Working with the data

While Rerun can present data directly, programmatic access is necessary for complex processing or to apply a custom visualization blueprint. We will access the converted RRD file using Rerun's dataframe and data-loading capabilities.

> **Key Concept**: Rerun converts common ROS messages (like [`sensor_msgs/msg/NavSatFix`](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/NavSatFix.html) or [`sensor_msgs/msg/PointCloud2`](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/PointCloud2.html)) into native Rerun types ([`GeoPoints`](https://www.rerun.io/docs/reference/types/archetypes/geo_points) or [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3D)) during the MCAP -> RRD conversion. This is why we read the RRD file in the following steps.

The main logic resides in the `log_dataset` function (in `mcap_example/__main__.py`), which loads the RRD archive and processes different sensor messages:

```python
with rr.server.Server(datasets={'dataset': [path_to_rrd]}) as server:
    dataset = server.client().get_dataset('dataset')

    log_gps(dataset)
    log_imu(dataset)
    # ... and so on for all entities
```

#### GPS data (`log_gps`)

The GPS coordinates are logged as [`GeoPoints`](https://www.rerun.io/docs/reference/types/archetypes/geo_points). In the original bag file, the GPS data was stored as the ROS message [sensor_msgs/msg/NavSatFix](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/NavSatFix.html). Rerun has native support for this ROS message, meaning when the bag file was converted to a RRD file, the messages were converted to Rerun's `GeoPoints` type. Also, since this message contains a [Header](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Header.html), the `ros2_timestamp` timeline will be populated with data from this header. This makes it easy to log them:

```python
entity = '/gps/duro/fix'
positions_col = f'{entity}:GeoPoints:positions'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

timestamps = rr.TimeColumn('time', timestamp=pa.table(
    df.select(timeline))[timeline].to_numpy())

positions = pa.table(df.select(col(positions_col)[0]))[0].to_pylist()

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle/gps',
    indexes=[timestamps],
    columns=rr.GeoPoints.columns(
        positions=positions,
        radii=[rr.Radius.ui_points(10.0)] * len(positions),
    ),
)
```

First we specify the entity we want, in this case it is the ROS topic `/gps/duro/fix`. We only want the `GeoPoints:positions` component so we specify that (i.e., `f'{entity}:GeoPoints:positions'`). You could also read the MCAP components, however, that is more difficult.

Next, we specify the `ros2_timestamp` timeline. This timeline was populated by the ROS timestamp that was in the [Header](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Header.html) of the [sensor_msgs/msg/NavSatFix](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/NavSatFix.html) messages, when the MCAP was converted to RRD.

Since everything had been converted already to the format we wanted, we can simply log the data using the `send_columns` function.

#### IMU data (`log_imu`)

The IMU data are logged as [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars). Simular to the GPS data, Rerun also has support for the IMU ROS messsage type, [sensor_msgs/msg/Imu](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/Imu.html). This makes it easy to log this data as well:

```python
entity = '/gps/duro/imu'
scalars_col = f'{entity}:Scalars:scalars'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

timestamps = rr.TimeColumn('time', timestamp=pa.table(
    df.select(timeline))[timeline].to_numpy())

data = np.vstack(pa.table(df.select(scalars_col))[0].to_numpy())
angular_velocity = data[:, :3]
linear_acceleration = data[:, 3:6]

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle/imu/angular_velocity',
    indexes=[timestamps],
    columns=rr.Scalars.columns(
        scalars=angular_velocity,
    ),
)

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle/imu/linear_acceleration',
    indexes=[timestamps],
    columns=rr.Scalars.columns(
        scalars=linear_acceleration,
    ),
)
```

The code is pretty much the same as for the GPS data.

#### Speed data (`log_speed`)

The speed data is also logged as [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars). However, this time it is **not** as straight forward. This is because the speed is recorded as a [std_msgs/msg/Float32](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Float32.html), which will not be converted to a Rerun type. Furthermore, the ROS message does not contain a [Header](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Header.html), meaning the `ros2_timestamp` also will not be populated. This message is, however, simple in that it only contains a single `Float32` datafield. To log the speed data we do:

```python
entity = '/vehicle_speed_kmph'
float32_msg_col = f'{entity}:std_msgs.msg.Float32:message'
timeline = 'message_publish_time'

df = dataset.filter_contents([entity]).reader(index=timeline)

timestamps = rr.TimeColumn('time', timestamp=pa.table(
    df.select(timeline))[timeline].to_numpy())

data = pa.table(df.select(col(float32_msg_col)[0]))[0].to_pylist()
speeds = [msg['data'] for msg in data]

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle/speed',
    indexes=[timestamps],
    columns=rr.Scalars.columns(
        scalars=speeds,
    ),
)
```

As you can see, we read the ROS [std_msgs/msg/Float32](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Float32.html) message component and select the `message_publish_time` timeline instead of `ros2_timestamp`. [You might instead want to use the `message_log_time` timeline](https://rerun.io/docs/reference/mcap/message-formats#timelines). We also need to extract the `Float32` data from each message, this we do on the line `[msg[0]['data'] for msg in data]`.

#### Pose data (`log_pose`)

The pose data is logged as [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/Transform3D). The pose data comes in the form of [geometry_msgs/msg/PoseStamped](https://docs.ros.org/en/jazzy/p/geometry_msgs/msg/PoseStamped.html) messages, a message type that Rerun converts to [`InstancePoses3D`](https://www.rerun.io/docs/reference/types/archetypes/Instance_Poses3D). Below you can see how to extract the data from this message type.

```python
entity = '/current_pose'
quaternions_col = f'{entity}:InstancePoses3D:quaternions'
translations_col = f'{entity}:InstancePoses3D:translations'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

timestamps = rr.TimeColumn('time', timestamp=pa.table(
    df.select(timeline))[timeline].to_numpy())

table = pa.table(df.select(col(quaternions_col)[0], col(translations_col)[0]))

quaternions = table[0].to_pylist()
translations = table[1].to_pylist()

rr.send_columns(
    'world/ego_fix_rot',
    indexes=[timestamps],
    columns=rr.Transform3D.columns(
        translation=translations,
    ),
)

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle',
    indexes=[timestamps],
    columns=rr.Transform3D.columns(
        quaternion=quaternions,
    ),
)
```

We separate the translation from rotation for a fixed-orientation top-down view in the Rerun viewer, as you will see later. We also subtract all the positions with the intial position, to start the run from the origin.

#### Camera sensor (`log_camera`)

Pinhole cameras and sensor poses are initialized to offer a 3D view and camera perspective. This is achieved using the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes. The camera intrinsics are stored as a [sensor_msgs/msg/CameraInfo](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/CameraInfo.html) message in ROS. Rerun will automatically convert this into the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) Rerun type, so we can query for that. As the camera intrinsics does not change, we only read the first message and statically log the information.

```python
rr.log(
    'world/ego_fix_rot/ego_vehicle/camera',
    rr.Transform3D(
        translation=[0, 0, 1],
        relation=rr.TransformRelation.ParentFromChild,
    ),
    static=True,
)

entity = '/zed_node/left/camera_info'
image_from_camera_col = f'{entity}:Pinhole:image_from_camera'
resolution_col = f'{entity}:Pinhole:resolution'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

table = pa.table(df.select(col(image_from_camera_col)[0], col(resolution_col)[0]))

image_from_camera = table[0][0].as_py()
resolution = table[1][0].as_py()

rr.log(
    'world/ego_fix_rot/ego_vehicle/camera/image',
    rr.Pinhole(
        image_from_camera=image_from_camera,
        resolution=resolution,
        camera_xyz=rr.components.ViewCoordinates.FLU,
        image_plane_distance=1.5,
    ),
    static=True
)
```

For simplicity, we set the camera 1 meter above the center of the ego vehicle in this case. As an excersice you can try reading the data from the ROS `/tf` topic to apply the correct transformations.

#### Camera data (`log_images`)

Camera data is logged as encoded images using [`EncodedImage`](https://www.rerun.io/docs/reference/types/archetypes/encoded_image). Rerun support both [sensor_msgs/msg/Image](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/Image.html) and [sensor_msgs/msg/CompressedImage](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/CompressedImage.html) (the one encountered in this dataset), making it easy to read images from RRD files into Python.

```python
entity = '/zed_node/left/image_rect_color/compressed'
blob_col = f'{entity}:EncodedImage:blob'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

timestamps = rr.TimeColumn('time', timestamp=pa.table(
    df.select(timeline))[timeline].to_numpy())

images = pa.table(df.select(blob_col))[blob_col].to_numpy()
images = np.concatenate(images).tolist()

rr.send_columns(
    'world/ego_fix_rot/ego_vehicle/camera/image',
    indexes=[timestamps],
    columns=rr.EncodedImage.columns(
        blob=images,
    ),
)
```

If you want to perform some processing, for example segmentation or classification, using the images, you can easily convert them into PIL or OpenCV images and go from there.

#### LiDAR data (`log_point_clouds`)

The [sensor_msgs/msg/PointCloud2](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/PointCloud2.html) is supported and converted to the [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype. Here, we iterate through batches and calculate the minimum/maximum height to apply color coding based on the Z-coordinate:

```python
entity = '/left_os1/os1_cloud_node/points'
positions_col = f'{entity}:Points3D:positions'
timeline = 'ros2_timestamp'

df = dataset.filter_contents([entity]).reader(index=timeline)

for stream in df.select(timeline, positions_col).repartition(10).execute_stream_partitioned():
    for batch in stream:
        pa = batch.to_pyarrow()
        for i in range(pa.num_rows):
            positions = np.array(pa[positions_col][i].as_py())

            min_z = np.min(positions[:, 2])
            max_z = np.max(positions[:, 2])
            colors = (positions[:, 2] - min_z) / (max_z - min_z)
            colors = mpl.colormaps['turbo'](colors)[:, :3]

            rr.set_time('time', timestamp=pa[timeline][i])
            rr.log(
                'world/ego_fix_rot/ego_vehicle/points',
                rr.Points3D(
                    positions=positions,
                    colors=colors,
                    radii=rr.Radius.ui_points(2.0),
                ),
            )
```

#### Setting up the default blueprint

The default blueprint is configured to provide an optimal view of the data:

* **3D View**: Set with `origin='/world/ego_fix_rot/ego_vehicle'` to follow the vehicle from a third-person perspective.
* **Top Down 3D View**: Set with `origin='/world/ego_fix_rot'` (the entity path with only translation) to provide a fixed, map-aligned bird's-eye view, regardless of the vehicle's yaw.
* **Time Series Views**: For plotting IMU and speed.
* **Map View**: For visualizing GPS.

```python
rrb.Blueprint(
    rrb.Horizontal(
        rrb.Vertical(
            rrb.Spatial3DView(
                name='3D View',
                origin='/world/ego_fix_rot/ego_vehicle',
                contents=["+ /**"],
                defaults=[
                    rr.Pinhole.from_fields(
                        image_plane_distance=1.0,
                        color=[0, 0, 0, 0],
                        line_width=0.0,
                    ),
                ],
                eye_controls=rrb.EyeControls3D(
                    position=(-5, 0, 2.5),
                    look_target=(0.0, 0.0, 2),
                    eye_up=(0.0, 0.0, 1.0),
                    spin_speed=0.0,
                    kind=rrb.Eye3DKind.FirstPerson,
                    speed=20.0,
                )
            ),
            rrb.Spatial3DView(
                name='Top Down 3D View',
                origin='/world/ego_fix_rot',
                contents=["+ /**"],
                defaults=[
                    rr.Pinhole.from_fields(
                        image_plane_distance=1.0,
                        color=[0, 0, 0, 0],
                        line_width=0.0,
                    ),
                ],
                eye_controls=rrb.EyeControls3D(
                    position=(0, 0, 60),
                    look_target=(-.18, 0.93, -0.07),
                    eye_up=(0.0, 0.0, 1.0),
                    spin_speed=0.0,
                    kind=rrb.Eye3DKind.FirstPerson,
                    speed=20.0,
                ),
            ),
        ),
        rrb.Vertical(
            rrb.TextDocumentView(
                name='Description',
                contents='description',
            ),
            rrb.Horizontal(
                rrb.TimeSeriesView(
                    name='Angular Velocity View',
                    contents='world/ego_fix_rot/ego_vehicle/imu/angular_velocity',
                    axis_x=rrb.archetypes.TimeAxis(
                        view_range=rr.TimeRange(
                            start=rr.TimeRangeBoundary.cursor_relative(
                                seconds=-3),
                            end=rr.TimeRangeBoundary.cursor_relative(
                                seconds=3)
                        )
                    ),
                ),
                rrb.TimeSeriesView(
                    name='Linear Acceleration View',
                    contents='world/ego_fix_rot/ego_vehicle/imu/linear_acceleration',
                    axis_x=rrb.archetypes.TimeAxis(
                        view_range=rr.TimeRange(
                            start=rr.TimeRangeBoundary.cursor_relative(
                                seconds=-3),
                            end=rr.TimeRangeBoundary.cursor_relative(
                                seconds=3)
                        )
                    ),
                )
            ),
            rrb.Horizontal(
                rrb.MapView(
                    name='Map View',
                    zoom=18,
                ),
                rrb.TimeSeriesView(
                    name='Speed View',
                    contents='world/ego_fix_rot/ego_vehicle/speed',
                ),
            ),
            row_shares=[0.2, 0.4, 0.4],
        ),
    ),
    rrb.TimePanel(
        state=rrb.components.PanelState.Collapsed,
        play_state=rrb.components.PlayState.Playing,
        loop_mode=rrb.components.LoopMode.All,
    ),
    collapse_panels=True,
)
```

### Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/mcap_example) for more information.

To run this example, make sure you have the [Pixi](https://pixi.sh/latest/#installation) package manager installed.

```sh
pixi run example
```

You can type:

```sh
pixi run example -h
```

to see all available commands. For example, if you placed the `RRD` file in a different location, you want to provide the `--root-dir` option, and if you renamed the file, you will want to provide the `--dataset-file` option.

## Tips

Here are some tips for how to work with an RRD file. The video goes over these in more detail.

### Stats tool

You can see the entities and components you are interested in using the `stats` tool:

```sh
rerun rrd stats [PATH_TO_INPUT_RRDS]
```

### How to find the component for an entity

If you are unsure what components are available for an entity, a script is provided, `mcap_example/rrd_info.py`, in this repository. To run it:

```sh
pixi run rrd_info [PATH_TO_INPUT_RRDS ...]
```

So for the RRD file we created in this example you would run:

```sh
pixi run rrd_info leaf-2022-03-18-gyor.rrd
```

and you will see output in the form of:

```
/ENTITY_1:
  ├─ COMPONENT_1: [FIELDS...]
  ...
  └─ COMPONENT_N: [FIELDS...]
...
/ENTITY_M:
  ├─ COMPONENT_1: [FIELDS...]
  ...
  └─ COMPONENT_L: [FIELDS...]
```

### Convert Rerun `EncodedImage` to PIL

Here follows an example for how you can open the Rerun `EncodedImage:blob` component as a PIL image. This is useful if you, for example, want to process the images in some fashion, maybe for segmentation or classification.

```python
import rerun as rr
from datafusion import col
import pyarrow as pa
from PIL import Image
import numpy as np
import io

path_to_rrd = 'leaf-2022-03-18-gyor.rrd'

with rr.server.Server(datasets={'dataset': [path_to_rrd]}) as server:
    dataset = server.client().get_dataset('dataset')

    entity = '/zed_node/left/image_rect_color/compressed'
    blob_col = f'{entity}:EncodedImage:blob'
    timeline = 'ros2_timestamp'

    df = dataset.filter_contents([entity]).reader(index=timeline)

    image = pa.table(df.select(blob_col))[blob_col][0]

    image_data = np.array(image, np.uint8)
    image = Image.open(io.BytesIO(image_data))
    image.show()
```

## In-depth elaboration on the MCAP -> RRD workflow

### 1. The importance of RRD conversion

The core of this example is demonstrating how Rerun can efficiently handle complex robotics data logs. While Rerun can view MCAP files directly, the conversion to RRD (Rerun Data file) is crucial for two main reasons:

* **Semantic Interpretation**: MCAP files store data as opaque, pre-serialized blobs corresponding to specific ROS message types (e.g., [sensor_msgs/msg/PointCloud2](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/PointCloud2.html)). When Rerun's `mcap convert` utility processes this, it semantically interprets these blobs. It understands that a `PointCloud2` message should be converted into the native Rerun `Points3D` archetype. This process lifts the data out of the ROS ecosystem and into a Rerun-native, high-level format.
* **Time Synchronization and Timelines**: ROS messages often contain timestamps in a [std_msgs/msg/Header](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Header.html) field that represent when the data was captured (sensor time). The MCAP container itself has separate timestamps for when the data was logged (`message_log_time`) and published (`message_publish_time`). During RRD conversion, Rerun automatically extracts the `Header` timestamp and logs it to either the `ros2_timestamp` or `ros2_duration` timeline ([read more on the timelines here](https://rerun.io/docs/reference/mcap/message-formats#timelines)). This is essential for accurate visualization, allowing you to easily view data across multiple sensors based on the moment of capture, not just the moment of logging.

### 2. The Python data query and processing (RRD -> Rerun)

The `log_dataset` function showcases Rerun's flexibility by mixing automatically converted data with manually processed data.

#### A. Automatic conversion and easy query

For supported messages like [sensor_msgs/msg/NavSatFix](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/NavSatFix.html) (GPS) and [sensor_msgs/msg/PointCloud2](https://docs.ros.org/en/jazzy/p/sensor_msgs/msg/PointCloud2.html) (LiDAR), you query for the Rerun archetype components directly. This simplifies the Python code significantly, turning a complex deserialization and conversion task into a simple dataframe selection.

#### B. Manual processing and custom visualization

For messages not automatically converted (like [std_msgs/msg/Float32](https://docs.ros.org/en/jazzy/p/std_msgs/msg/Float32.html)), you query the raw ROS message payload and apply custom logic:

* **Speed (Float32)**: Since there is no `Header`, the `ros2_timestamp` is unavailable. We must fall back to the `message_publish_time` (or `message_log_time`) timeline and manually unwrap the float value from the message structure (`msg[0]['data']`).
* **LiDAR Coloring**: Even though the point cloud is automatically converted to `Points3D`, the example adds custom logic to calculate the minimum and maximum Z-height across the entire point cloud and use a color map (like `turbo`) to color the points based on their elevation. This improves the clarity of the 3D visualization.

```python
# Example: Coloring based on Z height
min_z = np.min(points[:, 2])
max_z = np.max(points[:, 2])
colors = (points[:, 2] - min_z) / (max_z - min_z)
colors = mpl.colormaps['turbo'](colors)[:, :3]
```
