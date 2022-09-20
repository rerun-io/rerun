#!/usr/bin/env python3

"""Example of using the Rerun SDK to log the Objectron dataset."""

import os
import argparse
import numpy as np

from pathlib import Path

import rerun_sdk as rerun

def relpath(path: Path):
    return os.path.join(
        os.path.abspath(os.path.dirname(__file__)),
        path
    )

def log_dir(dirpath: Path, nb_frames: int):
    log_geometry(dirpath, nb_frames)

def log_geometry(dirpath: Path, nb_frames: int):
    path = os.path.join(relpath(dirpath), 'geometry.pbdata')
    print(f"logging geometry: {path}")

    frame_idx = 0
    frame_times = []

    data = Path(path).read_bytes()

    while len(data) > 0 and frame_idx < nb_frames:
        next_len = int.from_bytes(data[:4], byteorder='little', signed=False)
        data = data[4:]

        from proto.research.compvideo.arcapture import ARFrame
        frame = ARFrame().parse(data[:next_len])
        data = data[next_len:]

        rerun.set_time_sequence("frame", frame_idx)
        rerun.set_time_seconds("time", frame.timestamp)

        log_image(os.path.join(dirpath, f"video/{frame_idx}.jpg"))

        frame_idx += 1

def log_image(path: str):
    print(f"logging image: {path}")

    from PIL import Image
    img = Image.open(path)
    assert img.mode == 'RGB'

    rgb = np.asarray(img)
    rerun.log_image("video", rgb, space="image")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs Objectron data using the Rerun SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument('--frames', type=int, default=2**32-1,
                        help='If specifies, limits the number of frames logged')
    parser.add_argument('dir', type=Path, nargs='+',
                        help='Directories to log (e.g. `dataset/bike/batch-8/16/`)')
    args = parser.parse_args()

    for dirpath in args.dir:
        log_dir(dirpath, args.frames)

    if args.save is not None:
        rerun.save(args.save)
    elif not args.connect:
        # Show the logged data inside the Python process:
        # TODO: this seem to crash when quitting?
        rerun.show()
