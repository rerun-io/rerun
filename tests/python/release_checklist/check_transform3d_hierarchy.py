from __future__ import annotations

import os
import zipfile
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Transforms 3D hierarchy

This test adds more and more transforms in a hierarchy on each step on the `steps` timeline.

Enable the origin axis on the view to get a better impression of what happens in the scene.

What you should see on each step on the `steps` timeline:
* 0: There's a Rerun logo is at the origin, the `e` sits roughly above the origin.
* 1: The logo is translated a few units diagonally positively on x/y/z.
* 2: The logo is back to normal (frame 0) again.
* 3: The logo is squished along its height
* 4: The logo is back to normal (frame 0) again.
* 5: The logo rotated 90 degrees around the y axis (green). It reads now along the z axis (blue).
* 6: The logo rotated 45 degrees around the y axis (green).
* 7: The logo is back to normal (frame 0) again.
"""

rerun_obj_path = f"{os.path.dirname(os.path.realpath(__file__))}/../../assets/rerun.obj"


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    # To disable the transform axis everywhere, set the default to `AxisLength(0.0)`.
    disabled_transform_axis = [rr.components.AxisLength(0.0)]

    return rrb.Blueprint(
        rrb.Horizontal(
            rrb.TextDocumentView(origin="readme"),
            rrb.Spatial3DView(
                name="Long hierarchy with various types of transforms",
                defaults=disabled_transform_axis,
            ),
        )
    )


def log_data() -> None:
    # The Rerun logo obj's convention is y up.
    rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Y_UP, static=True)

    path = "/"

    # Log a bunch of transforms that undo each other in total, roughly arriving at the identity transform.
    # This is done using various types of transforms, to test out that they all work as expected.

    rr.set_time_sequence("steps", 1)
    path += "translate/"
    rr.log(path, rr.Transform3D(translation=[4, 4, 4]))

    rr.set_time_sequence("steps", 2)
    path += "translate_back/"
    rr.log(path, rr.Transform3D(translation=[-4, -4, -4]))  # TODO(#3559): Use a Mat4x4 here to translate back.

    rr.set_time_sequence("steps", 3)
    path += "scale/"
    rr.log(path, rr.Transform3D(scale=[1.0, 0.2, 1.0]))

    rr.set_time_sequence("steps", 4)
    path += "scale_back_mat3x3/"
    # fmt: off
    rr.log(path, rr.Transform3D(mat3x3=[1.0, 0.0, 0.0,
                                        0.0, 5.0, 0.0,
                                        0.0, 0.0, 1.0]))
    # fmt: on

    rr.set_time_sequence("steps", 5)
    path += "rotate_axis_origin/"
    rr.log(path, rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[0, 1, 0], degrees=90)))

    rr.set_time_sequence("steps", 6)
    path += "rotate_quat/"
    rr.log(
        # -45 degrees around the y axis.
        # Via https://www.andre-gaschler.com/rotationconverter/
        path,
        rr.Transform3D(rotation=rr.Quaternion(xyzw=[0, -0.3826834, 0, 0.9238796])),
    )
    path += "rotate_mat3x3/"

    rr.set_time_sequence("steps", 7)
    # fmt: off
    rr.log(
        path,
        rr.Transform3D(
            # -45 degrees around the y axis.
            # Via https://www.andre-gaschler.com/rotationconverter/
            mat3x3=[0.7071069, 0.0000000, -0.7071066,
                    0.0000000, 1.0000000, 0.0000000,
                    0.7071066, 0.0000000, 0.7071069]
        ),
    )
    # fmt: on

    # Add the Rerun asset at the end of the hierarchy.
    # (We're using a 3D model because it's easier to see the effect of arbitrary transforms here!)
    rr.set_time_sequence("steps", 0)
    rr.log(path + "/asset", rr.Asset3D(path=rerun_obj_path))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    # Extract the rerun_obj.zip file
    with zipfile.ZipFile(f"{rerun_obj_path}.zip", "r") as zip_ref:
        zip_ref.extractall(os.path.dirname(rerun_obj_path))

    log_readme()
    log_data()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
