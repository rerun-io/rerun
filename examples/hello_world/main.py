#!/usr/bin/env python3
"""The simplest example of how to use Rerun."""

import pathlib

from PIL import Image

import rerun as rr

rr.init("hello_world", spawn_and_connect=True, application_path=pathlib.Path(__file__).resolve())  # Spawn a Rerun Viewer and stream log events to it

rerun_logo = Image.open("crates/re_ui/data/logo_dark_mode.png")
rr.log_image("rgb_image", rerun_logo)
