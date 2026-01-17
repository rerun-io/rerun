import rerun as rr
from pathlib import Path
import time
import numpy as np

def log_urdf(rec: rr.RecordingStream, urdf_path: Path, entity_path_prefix: str) -> None:
    # this creates a transform tree based on coordinate frame names
    # look at entity_path_prefix/<urdf_root> for coordinate frame, everything here is relative to that
    rec.log_file_from_path(urdf_path, entity_path_prefix=entity_path_prefix, static=True)

def log_disjoint_transforms(rec: rr.RecordingStream) -> None:
    # this creates a transform tree based on entity paths
    # / -> #tf
    # /world -> #tf/world
    # etc.
    rec.log("/world/depth_1", rr.Transform3D(translation=[-.1, 0., 0.], parent_frame="root"), static=True)
    rec.log("/world/depth_1/depth_2", rr.Transform3D(translation=[0., -.1, 0.]), static=True)
    K = np.zeros((3, 3))
    K[0, 0] = 500.0  # fx
    K[1, 1] = 500.0  # fy
    K[0, 2] = 320.0  # cx
    K[1, 2] = 240.0  # cy
    K[2, 2] = 1.0
    rec.log("/world/depth_1/depth_2/camera", rr.Transform3D(translation=[0., 0., .1]), rr.Pinhole(resolution=[640, 480], image_from_camera=K), static=True)

def main() -> None:
    with rr.RecordingStream("mixed_transform_example") as rec:
        rec.spawn()

        # Add URDF
        log_urdf(rec, Path("examples") / "rust" / "animated_urdf" / "data" / "so100.urdf", entity_path_prefix="/world/robot")
        # Add manual transforms (based on entity paths)
        log_disjoint_transforms(rec)
        # Connect our two trees
        #   First make / root instead of #tf
        rec.log("/", rr.CoordinateFrame("root"), static=True)
        #   Then connect / to to entity_path_prefix/<urdf_root> via name
        rec.log("/", rr.Transform3D(translation=[0., 0., 0.], parent_frame="root", child_frame="base"), static=True)
        #   Then connect /world to / via mixed names and entity paths
        rec.log("/world", rr.Transform3D(translation=[0., 0., 0.], parent_frame="root",), static=True)
        # This appears to not work because each entity can only have one coordinate frame
        if False:
            rec.log("/world", rr.Transform3D(translation=[0., 0., 0.], parent_frame="world", child_frame="base"), rr.CoordinateFrame("world"), static=True)

        rec.set_time("frame", sequence=0)
        rec.log("/world/depth_1/depth_2/camera", rr.Image((255 * np.random.rand(480, 640)).astype(np.uint8)))

        rec.set_time("frame", sequence=1)
        rec.log("/world/depth_1/depth_2/camera", rr.Image((255 * np.random.rand(480, 640)).astype(np.uint8)))

        rec.flush()
    while True:
        time.sleep(1)

if __name__ == "__main__":
    main()
