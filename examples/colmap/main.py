#!/usr/bin/env python3
"""

"""

import rerun as rr

from argparse import ArgumentParser

@rr.script("Visualize Colmap Data")
def main(parser: ArgumentParser) -> None:
    parser.add_argument("--input_model", help="path to input model folder")

    args = parser.parse_args()

    cameras, images, points3D = read_model(path=args.input_model, ext=args.input_format)

if __name__ == "__main__":
    main()
