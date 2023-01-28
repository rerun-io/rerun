---
title: For Python Users
order: 2
---

Everything you need to use rerun is available via the [rerun-sdk](https://pypi.org/project/rerun-sdk/) python package:
```bash
$ pip install rerun-sdk
```

And now you can log some data:
```python
import rerun as rr
import numpy as np

rr.init("python_example", True)
rr.log_points("points", np.random.rand(20, 3))
```
TODO(jleibs): Image of the output

For more on using the Rerun viewer, checkout the [quick tour](getting-started/quick-tour) or the
[viewer reference](reference/viewer).

Or, to find out about how to log data with Rerun see [Logging Data from Python](getting-started/logging-data-python)
