"""Shows how to use the rerun SDK."""

from dataclasses import dataclass
from typing import Iterator, Tuple

import argparse
import cv2  # type: ignore
import numpy as np

import rerun_sdk as rerun


def log_dummy_data(args):
    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    """Log a few frames of generated dummy data to show how the Rerun SDK is used."""
    NUM_FRAMES = 40

    for sample in generate_dummy_data(num_frames=NUM_FRAMES):
        # This will assign logged objects a "time source" called `frame_nr`.
        # In the viewer you can select how to view objects - by frame_nr or the built-in `log_time`.
        rerun.set_time_sequence("frame_nr", sample.frame_idx)

        rerun.log_image("rgb", sample.rgb_image)

        ((car_x, car_y), (car_w, car_h)) = sample.car_bbox
        rerun.log_rect("bbox", [car_x, car_y],
                       [car_w, car_h], label="A car", color=(0, 128, 255))

        # Lets log the projected points into a separate "space", called 'projected_space'.
        # We also set its up-axis.
        # The default spaces are "2D" and "3D" (based on what you log).
        rerun.set_space_up("projected_space", [0, -1, 0])
        rerun.log_points("points", sample.point_cloud, space="projected_space")

        rerun.log_camera("rgbd_camera",
                         resolution=sample.camera.resolution,
                         intrinsics=sample.camera.intrinsics,
                         rotation_q=sample.camera.rotation_q,
                         position=sample.camera.position,
                         camera_space_convention=rerun.CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
                         space="projected_space")

        # The depth image is in millimeters, so we set meter=1000
        rerun.log_depth_image("depth", sample.depth_image_mm, meter=1000)

    with open('crates/re_viewer/data/camera.glb', mode='rb') as file:
        mesh_file = file.read()
        # Optional transformation matrix to apply (in this case: scale it up by a factor x2)
        transform = [
            [2, 0, 0, 0],
            [0, 2, 0, 0],
            [0, 0, 2, 0],
            [0, 0, 0, 1]]
        rerun.log_mesh_file("example_mesh", rerun.MeshFormat.GLB,
                            mesh_file, transform=transform)

    rerun.log_path("a_box", [
        [0, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [1, 0, 0],
        [0, 0, 0]])

    if not args.connect:
        # Show the logged data inside the Python process:
        rerun.show()


class DummyCar:
    """Class representing a dummy car for generating dummy data to log."""
    BODY_COLOR = (79, 91, 102)
    TIRES_COLOR = (0, 0, 0)

    def __init__(self, center: Tuple[int, int], size: Tuple[int, int], distance_mm: float):
        self.center = np.array(center)
        self.size = np.array(size)
        self.distance_mm = distance_mm

    @property
    def min(self) -> np.ndarray:
        return self.center - self.size / 2

    @property
    def max(self) -> np.ndarray:
        return self.center + self.size / 2

    def drive_one_step(self):
        self.center[0] += 5
        self.distance_mm -= 1

    def draw(self, depth_image_mm: np.ndarray, rgb: np.ndarray):
        # 1. Draw the tires of the car
        tire_radius = (self.size[1] * (1./5)).astype(int)
        tire_center_y = self.max[1] - tire_radius
        tire_distance_mm = self.distance_mm + 200

        # 1.1 Draw the left tire
        left_tire_center = np.array(
            (self.center[0] - 2 * tire_radius, tire_center_y)).astype(int)
        cv2.circle(depth_image_mm, left_tire_center, tire_radius,
                   tire_distance_mm, cv2.FILLED)
        cv2.circle(rgb, left_tire_center, tire_radius,
                   DummyCar.TIRES_COLOR, cv2.FILLED)

        # 1.2 Draw the right tire
        right_tire_center = np.array(
            (self.center[0] + 2 * tire_radius, tire_center_y)).astype(int)
        cv2.circle(depth_image_mm, right_tire_center, tire_radius,
                   tire_distance_mm, cv2.FILLED)
        cv2.circle(rgb, right_tire_center, tire_radius,
                   DummyCar.TIRES_COLOR, cv2.FILLED)

        # 2. Draw the body
        body_section_height = self.size[1] * (2./5)

        # 2.1 Draw the top part of the car
        top_section_distance_mm = self.distance_mm + 100
        top_section_width = self.size[0] * (3./5)
        top_min = self.min + \
            np.array((self.size[1] - top_section_width / 2, 0))
        top_max = top_min + np.array((top_section_width, body_section_height))
        cv2.rectangle(depth_image_mm, top_min.astype(int), top_max.astype(int),
                      top_section_distance_mm, cv2.FILLED)
        cv2.rectangle(rgb, top_min.astype(int), top_max.astype(int),
                      DummyCar.BODY_COLOR, cv2.FILLED)

        # 2.2 Draw the middle part of the car
        middle_min = self.min + np.array((0, body_section_height))
        middle_max = middle_min + \
            np.array((self.size[0], body_section_height))
        cv2.rectangle(depth_image_mm, middle_min.astype(int), middle_max.astype(int),
                      self.distance_mm, cv2.FILLED)
        cv2.rectangle(rgb, middle_min.astype(int), middle_max.astype(int),
                      DummyCar.BODY_COLOR, cv2.FILLED)


@dataclass
class CameraParameters:
    """Holds the intrinsic and extrinsic parameters of a camera."""
    resolution: np.ndarray
    intrinsics: np.ndarray
    rotation_q: np.ndarray
    position: np.ndarray


@ dataclass
class SampleFrame:
    """Holds data for a single frame of data."""
    frame_idx: int
    camera: CameraParameters
    depth_image_mm: np.ndarray
    point_cloud: np.ndarray
    rgb_image: np.ndarray
    car_bbox: Tuple[np.ndarray, np.ndarray]


class SimpleDepthCamera:

    def __init__(self, image_width: int, image_height: int) -> None:
        self.w = image_width
        self.h = image_height

        # Simplest reasonable camera intrinsics given the resolution
        self.u_center = self.w / 2
        self.v_center = self.h / 2
        self.focal_length = (self.h * self.w) ** 0.5

        # Pre-generate image containing the x and y coordinates per pixel
        self.u_coords, self.v_coords = np.meshgrid(
            np.arange(0, self.w), np.arange(0, self.h))

    def back_project(self, depth_image_mm: np.ndarray) -> np.ndarray:
        """Given a depth image, generate a matching point cloud.
            - `depth_image_mm`: Depth image expressed in millimeters
        """

        z = depth_image_mm.reshape(-1) / 1000.
        x = (self.u_coords.reshape(-1).astype(float) -
             self.u_center) * z / self.focal_length
        y = (self.v_coords.reshape(-1).astype(float) -
             self.v_center) * z / self.focal_length

        back_projected = np.vstack((x, y, z)).T
        return back_projected

    def render_dummy_slanted_plane_mm(self) -> np.ndarray:
        """Renders a depth image of a slanted plane in millimeters."""
        return 1000.0 * 1. / (0.01 + 0.4*self.v_coords/self.h)

    @property
    def intrinsics(self) -> np.ndarray:
        """The camera's row-major intrinsics matrix."""
        return np.array((
            (self.focal_length, 0,                 self.u_center),
            (0,                 self.focal_length, self.v_center),
            (0,                 0,                 1)
        ))

    @property
    def rotation_q(self) -> np.ndarray:
        """The camera's rotation (world from camera) as a xyzw encoded quaternion."""
        return np.array((0, 0, 0, 1))  # Dummy "identity" value

    @property
    def position(self) -> np.ndarray:
        """The camera's position in world space."""
        return np.array((0, 0, 0))  # Dummy "identity" value

    @property
    def resolution(self) -> np.ndarray:
        """Image resolution as [width, height]."""
        return np.array([self.w, self.h])

    @property
    def parameters(self) -> CameraParameters:
        """The camera's parameters."""
        return CameraParameters(
            resolution=self.resolution,
            intrinsics=self.intrinsics,
            rotation_q=self.rotation_q,
            position=self.position
        )


def generate_dummy_data(num_frames: int) -> Iterator[SampleFrame]:
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
    rgb_background = intensity[:, :, np.newaxis] * \
        np.array(sand_color)[np.newaxis, np.newaxis, :]
    rgb_background = rgb_background.astype(np.uint8)

    # Generate `num_frames` sample data
    car = DummyCar(center=(140, 100), size=(200, 100), distance_mm=4000)
    for i in range(num_frames):
        depth_image_mm = depth_background_mm.copy()
        rgb = rgb_background.copy()
        car.draw(depth_image_mm=depth_image_mm, rgb=rgb)
        point_cloud = camera.back_project(depth_image_mm=depth_image_mm)
        sample = SampleFrame(frame_idx=i,
                             camera=camera.parameters,
                             depth_image_mm=depth_image_mm,
                             point_cloud=point_cloud,
                             rgb_image=rgb,
                             car_bbox=(car.min, car.size))

        yield sample
        car.drive_one_step()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs rich data using the Rerun SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    args = parser.parse_args()

    log_dummy_data(args)
