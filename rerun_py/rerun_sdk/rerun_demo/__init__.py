"""
The `rerun_demo` package contains utilities for easily demonstrating rerun features.

__main__.py is a program which tries to load a pre-baked .rrd file into the viewer

The `data` module contains a collection of reference objects and helpers that can be
easily logged with a few lines of code, but still produce visually interesting
content.

As an example, consider:
``` python
import rerun as rr
from rerun_demo.data import color_grid

rr.init("log_points", True)

rr.log_points("my_points", color_grid.positions, colors=color_grid.colors)
```

Note that because this package is shipped with the rerun-sdk pypi package, it
cannot carry any dependencies beyond those of rerun itself. This generally limits
demos to only using the standard library and numpy for data generation.
"""
__all__ = ["data", "turbo", "util"]
