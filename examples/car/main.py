#!/usr/bin/env python3
"""Shows how to use the Rerun SDK."""

import argparse
import pathlib
import sys
from dataclasses import dataclass
from typing import Iterator, Tuple

import cv2
import numpy as np
import numpy.typing as npt

import rerun as rr


def log_car_data() -> None:
    """Log a few frames of generated data to show how the Rerun SDK is used."""
    NUM_FRAMES = 40

    # Set our preferred up-axis on the space that we will log the points to:
    rr.log_view_coordinates("world", up="-Y", timeless=True)

    for sample in generate_car_data(num_frames=NUM_FRAMES):
        # This will assign logged entities a timeline called `frame_nr`.
        # In the viewer you can select how to view entities - by frame_nr or the built-in `log_time`.
        rr.set_time_sequence("frame_nr", sample.frame_idx)

        # We log the projected points in the "world" space:
        rr.log_points("world/points", sample.point_cloud)

        # Log the camera pose:
        rr.log_rigid3(
            "world/camera",
            parent_from_child=(sample.camera.position, sample.camera.rotation_q),
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        # Log the camera projection matrix:
        rr.log_pinhole(
            "world/camera/image",
            child_from_parent=sample.camera.intrinsics,
            width=sample.camera.resolution[0],
            height=sample.camera.resolution[1],
        )

        # We log the rgb image to the image-space of the camera:
        rr.log_image("world/camera/image/rgb", sample.rgb_image)

        # Same with the bounding box:
        ((car_x, car_y), (car_w, car_h)) = sample.car_bbox
        rr.log_rect("world/camera/image/bbox", [car_x, car_y, car_w, car_h], label="A car", color=(0, 128, 255))

        # The depth image is in millimeters, so we set meter=1000
        rr.log_depth_image("world/camera/image/depth", sample.depth_image_mm, meter=1000)


class DummyCar:
    """Class representing a dummy car for generating dummy data to log."""

    BODY_COLOR = (79, 91, 102)
    TIRES_COLOR = (0, 0, 0)

    def __init__(
        self,
        center: Tuple[int, int],
        size: Tuple[int, int],
        distance_mm: float,
    ):
        self.center: npt.NDArray[np.int32] = np.array(center, dtype=np.int32)
        self.size: npt.NDArray[np.int32] = np.array(size, dtype=np.int32)
        self.distance_mm = distance_mm

    @property
    def min(self) -> npt.NDArray[np.int32]:
        return np.array(self.center - self.size / 2, dtype=np.int32)

    @property
    def max(self) -> npt.NDArray[np.int32]:
        return np.array(self.center + self.size / 2, dtype=np.int32)

    def drive_one_step(self) -> None:
        self.center[0] += 5
        self.distance_mm -= 1

    def draw(
        self,
        depth_image_mm: npt.NDArray[np.float32],
        rgb: npt.NDArray[np.float32],
    ) -> None:
        # 1. Draw the tires of the car
        tire_radius = (self.size[1] * (1.0 / 5)).astype(int)
        tire_center_y = self.max[1] - tire_radius
        tire_distance_mm = self.distance_mm + 200

        # 1.1 Draw the left tire
        left_tire_center = np.array((self.center[0] - 2 * tire_radius, tire_center_y)).astype(int)
        cv2.circle(depth_image_mm, left_tire_center, tire_radius, tire_distance_mm, cv2.FILLED)
        cv2.circle(rgb, left_tire_center, tire_radius, DummyCar.TIRES_COLOR, cv2.FILLED)

        # 1.2 Draw the right tire
        right_tire_center = np.array((self.center[0] + 2 * tire_radius, tire_center_y)).astype(int)
        cv2.circle(depth_image_mm, right_tire_center, tire_radius, tire_distance_mm, cv2.FILLED)
        cv2.circle(rgb, right_tire_center, tire_radius, DummyCar.TIRES_COLOR, cv2.FILLED)

        # 2. Draw the body
        body_section_height = self.size[1] * (2.0 / 5)

        # 2.1 Draw the top part of the car
        top_section_distance_mm = self.distance_mm + 100
        top_section_width = self.size[0] * (3.0 / 5)
        top_min = self.min + np.array((self.size[1] - top_section_width / 2, 0))
        top_max = top_min + np.array((top_section_width, body_section_height))
        cv2.rectangle(
            depth_image_mm,
            top_min.astype(int),
            top_max.astype(int),
            top_section_distance_mm,
            cv2.FILLED,
        )
        cv2.rectangle(
            rgb,
            top_min.astype(int),
            top_max.astype(int),
            DummyCar.BODY_COLOR,
            cv2.FILLED,
        )

        # 2.2 Draw the middle part of the car
        middle_min = self.min + np.array((0, body_section_height))
        middle_max = middle_min + np.array((self.size[0], body_section_height))
        cv2.rectangle(
            depth_image_mm,
            middle_min.astype(int),
            middle_max.astype(int),
            self.distance_mm,
            cv2.FILLED,
        )
        cv2.rectangle(
            rgb,
            middle_min.astype(int),
            middle_max.astype(int),
            DummyCar.BODY_COLOR,
            cv2.FILLED,
        )


@dataclass
class CameraParameters:
    """Holds the intrinsic and extrinsic parameters of a camera."""

    resolution: npt.NDArray[np.int32]
    intrinsics: npt.NDArray[np.float32]
    rotation_q: npt.NDArray[np.float32]
    position: npt.NDArray[np.float32]


@dataclass
class SampleFrame:
    """Holds data for a single frame of data."""

    frame_idx: int
    camera: CameraParameters
    depth_image_mm: npt.NDArray[np.float32]
    point_cloud: npt.NDArray[np.float32]
    rgb_image: npt.NDArray[np.float32]
    car_bbox: Tuple[npt.NDArray[np.int32], npt.NDArray[np.int32]]


class SimpleDepthCamera:
    def __init__(self, image_width: int, image_height: int) -> None:
        self.w = image_width
        self.h = image_height

        # Simplest reasonable camera intrinsics given the resolution
        self.u_center = self.w / 2
        self.v_center = self.h / 2
        self.focal_length = (self.h * self.w) ** 0.5

        # Pre-generate image containing the x and y coordinates per pixel
        self.u_coords, self.v_coords = np.meshgrid(np.arange(0, self.w), np.arange(0, self.h))

    def back_project(
        self,
        depth_image_mm: npt.NDArray[np.float32],
    ) -> npt.NDArray[np.float32]:
        """
        Given a depth image, generate a matching point cloud.

        Args:
        - `depth_image_mm`: Depth image expressed in millimeters
        """

        # Apply inverse of the `intrinsics` matrix:
        z = depth_image_mm.reshape(-1) / 1000.0
        x = (self.u_coords.reshape(-1).astype(float) - self.u_center) * z / self.focal_length
        y = (self.v_coords.reshape(-1).astype(float) - self.v_center) * z / self.focal_length

        back_projected = np.vstack((x, y, z)).T
        return back_projected

    def render_dummy_slanted_plane_mm(self) -> npt.NDArray[np.float32]:
        """Renders a depth image of a slanted plane in millimeters."""
        return 1000.0 * 1.0 / (0.01 + 0.4 * self.v_coords / self.h)

    @property
    def intrinsics(self) -> npt.NDArray[np.float32]:
        """The camera's row-major intrinsics matrix."""
        return np.array(
            (
                (self.focal_length, 0, self.u_center),
                (0, self.focal_length, self.v_center),
                (0, 0, 1),
            ),
            dtype=np.float32,
        )

    @property
    def rotation_q(self) -> npt.NDArray[np.float32]:
        """The camera's rotation (world from camera) as a xyzw encoded quaternion."""
        return np.array((0, 0, 0, 1), dtype=np.float32)  # Dummy "identity" value

    @property
    def position(self) -> npt.NDArray[np.float32]:
        """The camera's position in world space."""
        return np.array((0, 0, 0), dtype=np.float32)  # Dummy "identity" value

    @property
    def resolution(self) -> npt.NDArray[np.int32]:
        """Image resolution as [width, height]."""
        return np.array([self.w, self.h])

    @property
    def parameters(self) -> CameraParameters:
        """The camera's parameters."""
        return CameraParameters(
            resolution=self.resolution,
            intrinsics=self.intrinsics,
            rotation_q=self.rotation_q,
            position=self.position,
        )


def generate_car_data(num_frames: int) -> Iterator[SampleFrame]:
    """This function generates dummy data to log."""
    # Generate some fake data
    im_w = 480
    im_h = 270

    camera = SimpleDepthCamera(image_width=im_w, image_height=im_h)

    # Background image as a simple slanted plane
    # 1. Depth
    depth_background_mm = camera.render_dummy_slanted_plane_mm()

    # 2. Color
    sand_color = (194, 178, 128)
    intensity = 1.0 / depth_background_mm
    intensity /= intensity.max()
    rgb_background = intensity[:, :, np.newaxis] * np.array(sand_color)[np.newaxis, np.newaxis, :]
    rgb_background = rgb_background.astype(np.uint8)

    # Generate `num_frames` sample data
    car = DummyCar(center=(140, 100), size=(200, 100), distance_mm=4000)
    for i in range(num_frames):
        depth_image_mm = depth_background_mm.copy()
        rgb = rgb_background.copy()
        car.draw(depth_image_mm=depth_image_mm, rgb=rgb)
        point_cloud = camera.back_project(depth_image_mm=depth_image_mm)
        sample = SampleFrame(
            frame_idx=i,
            camera=camera.parameters,
            depth_image_mm=depth_image_mm,
            point_cloud=point_cloud,
            rgb_image=rgb,
            car_bbox=(car.min, car.size),
        )

        yield sample
        car.drive_one_step()


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "car")
    log_car_data()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
