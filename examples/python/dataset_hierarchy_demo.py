"""Demo: hierarchical dataset names using ':' as separator."""

from __future__ import annotations

import rerun as rr

server = rr.server.Server()
client = server.client()

# Create datasets with hierarchical names using '.' as separator.
# These will appear as a collapsible tree in the recording panel.
DATASET_NAMES = [
    # Top-level (no hierarchy)
    "standalone_dataset",
    "raw_logs",
    "scratchpad",
    # One level of nesting
    "robotics.lidar_scans",
    "robotics.camera_feeds",
    "robotics.imu_data",
    "robotics.gps_traces",
    "robotics.wheel_odometry",
    # Two levels of nesting
    "perception.detection.pedestrians",
    "perception.detection.vehicles",
    "perception.detection.cyclists",
    "perception.detection.traffic_signs",
    "perception.segmentation.semantic",
    "perception.segmentation.instance",
    "perception.segmentation.panoptic",
    "perception.tracking.short_term",
    "perception.tracking.long_term",
    # Shared prefix with top-level sibling
    "maps.indoor",
    "maps.outdoor.parking_lot",
    "maps.outdoor.highway",
    "maps.outdoor.urban.downtown",
    "maps.outdoor.urban.residential",
    "maps.outdoor.rural.dirt_road",
    # Three levels deep, multiple branches
    "simulation.scenarios.highway.merge",
    "simulation.scenarios.highway.exit",
    "simulation.scenarios.intersection.unprotected_left",
    "simulation.scenarios.intersection.four_way_stop",
    "simulation.scenarios.parking.parallel",
    "simulation.scenarios.parking.perpendicular",
    "simulation.weather.rain",
    "simulation.weather.snow",
    "simulation.weather.fog",
    # Benchmarks with versions
    "benchmarks.kitti.v1",
    "benchmarks.kitti.v2",
    "benchmarks.nuscenes.mini",
    "benchmarks.nuscenes.full",
    "benchmarks.waymo.validation",
    "benchmarks.waymo.test",
    # Teams / ownership
    "teams.planning.trajectories",
    "teams.planning.behavior_trees",
    "teams.control.pid_tuning",
    "teams.control.mpc_experiments",
    # Single-child folders (edge case)
    "solo_folder.only_child",
    # Deep chain (stress test)
    "deep.a.b.c.d.leaf",
]

for name in DATASET_NAMES:
    client.create_dataset(name)
    print(f"Created dataset: {name}")

print(f"\nServer URL: {server.url()}")
print("Open the viewer and connect to this server to see the hierarchy.")
print("Press Ctrl+C to stop.")

try:
    import time

    while True:
        time.sleep(1)
except KeyboardInterrupt:
    pass
finally:
    server.shutdown()
