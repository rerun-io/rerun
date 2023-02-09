#!/usr/bin/env python3

"""Demonstrates the most barebone usage of the Rerun SDK."""

import numpy as np

import rerun as rr

SIZE = 10

rr.spawn()

x, y, z = np.meshgrid(np.linspace(-5, 5, SIZE), np.linspace(-5, 5, SIZE), np.linspace(-5, 5, SIZE))
positions = np.array(list(zip(x.reshape(-1), y.reshape(-1), z.reshape(-1))))

r, g, b = np.meshgrid(np.linspace(0, 255, SIZE), np.linspace(0, 255, SIZE), np.linspace(0, 255, SIZE))
colors = np.array(list(zip(r.reshape(-1), g.reshape(-1), b.reshape(-1))), dtype=np.uint8)

rr.log_points("my_points", positions=positions, colors=colors)
