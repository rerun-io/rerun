#!/usr/bin/env python3
"""The simplest example of how to use Rerun."""

import rerun as rr
from PIL import Image

rr.init("my_app", spawn_and_connect = True) # Spawn a Rerun Viewer and stream log events to it

rerun_logo = Image.open('crates/re_ui/data/logo_dark_mode.png')
rr.log_image("rgb_image", rerun_logo)
