# re_hdf5 test assets

`h5py_compat.h5` is the libhdf5-compat fixture read by `tests/integration.rs`.
Unlike every other test fixture (written in-test by `hdf5-pure`'s own writer), it was written by h5py/libhdf5, so it exercises libhdf5 idiosyncrasies such as v1 symbol-table groups.
It was generated once with the following script (h5py is deliberately not a repo dependency):

```python
# uv run --with h5py==3.16.0 --with numpy gen_h5py_compat.py h5py_compat.h5
import sys

import h5py
import numpy as np

out = sys.argv[1]
with h5py.File(out, "w") as f:
    f.attrs["version"] = np.int64(2)
    obs = f.create_group("observations")
    obs.attrs["frequency"] = np.float64(50.0)
    obs.create_dataset("qpos", data=np.arange(12, dtype=np.float64).reshape(4, 3))
    obs.create_dataset("qvel", data=np.arange(4, dtype=np.float32), compression="gzip")
```
