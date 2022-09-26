#!/usr/bin/env python3

"""
Example of using DeepSDF to compute SDFs for arbitrary meshes, and the Rerun SDK to visualize the
results. log the Objectron dataset.

@InProceedings{Park_2019_CVPR,
    author = {
        Park,
        Jeong Joon and Florence,
        Peter and Straub,
        Julian and Newcombe,
        Richard and Lovegrove,
        Steven,
    },
    title = {DeepSDF: Learning Continuous Signed Distance Functions for Shape Representation},
    booktitle = {The IEEE Conference on Computer Vision and Pattern Recognition (CVPR)},
    month = {June},
    year = {2019}
}

Setup:
```sh
(cd examples/deep_sdf && ./download_dataset.py)
```

Run:
```sh
# assuming your virtual env is up
python3 examples/deep_sdf/main.py examples/deep_sdf/dataset/buddha/buddha.obj
```
"""


import argparse
import logging
import math
import os
import sys

import numpy as np
import rerun_sdk as rerun

from dataclasses import dataclass
from pathlib import Path
from typing import List, Final, Iterator, Iterable

from rerun_sdk import ImageFormat
from scipy.spatial.transform import Rotation as R


def read_mesh(path: Path):
    pass


def log_mesh():
    pass


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK.')
    parser.add_argument('--headless', action='store_true',
                        help="Don't show GUI")
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument('--frames', type=int, default=sys.maxsize,
                        help='If specifies, limits the number of frames logged')
    parser.add_argument('path', type=Path, nargs='+',
                        help='Mesh(es) to log (e.g. `dataset/buddha/buddha.obj`)')
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    for path in args.path:
        mesh = read_mesh(path)
        log_mesh()

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
