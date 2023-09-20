"""
Utility converter functions to make attrs and mypy happy.

As of version 1.4.1, mypy (and possibly other tooling) doesn't properly recognize converters passed to attrs fields. For
example, consider this class:

```python
from attrs import define, field
import numpy as np
import numpy.typing as npt

@define
class ClassA:
    xy: npt.NDArray[np.float32]= field(converter=CONVERTER)
```

mypy is only happy if `CONVERTER` is a regular, properly-typed function. In particular, it rejects both of these correct
and more compact alternatives:
- `lambda data: np.array(data, dtype=np.float32)`
- `functools.partial(np.array, dtype=np.float32)`
"""

from __future__ import annotations

from typing import SupportsFloat, SupportsInt, overload

import numpy as np
import numpy.typing as npt

__all__ = [
    "int_or_none",
    "float_or_none",
    "bool_or_none",
    "str_or_none",
    "to_np_uint8",
    "to_np_uint16",
    "to_np_uint32",
    "to_np_uint64",
    "to_np_int8",
    "to_np_int16",
    "to_np_int32",
    "to_np_int64",
    "to_np_bool",
    "to_np_float16",
    "to_np_float32",
    "to_np_float64",
]


@overload
def int_or_none(data: None) -> None:
    ...


@overload
def int_or_none(data: SupportsInt) -> int:
    ...


def int_or_none(data: SupportsInt | None) -> int | None:
    if data is None:
        return None
    return int(data)


@overload
def float_or_none(data: None) -> None:
    ...


@overload
def float_or_none(data: SupportsFloat) -> float:
    ...


def float_or_none(data: SupportsFloat | None) -> float | None:
    if data is None:
        return None
    return float(data)


@overload
def bool_or_none(data: None) -> None:
    ...


@overload
def bool_or_none(data: bool) -> bool:
    ...


def bool_or_none(data: bool | None) -> bool | None:
    if data is None:
        return None
    return bool(data)


@overload
def str_or_none(data: None) -> None:
    ...


@overload
def str_or_none(data: str) -> str:
    ...


def str_or_none(data: str | None) -> str | None:
    if data is None:
        return None
    return str(data)


def to_np_uint8(data: npt.ArrayLike) -> npt.NDArray[np.uint8]:
    """Convert some datat to a numpy uint8 array."""
    return np.asarray(data, dtype=np.uint8)


def to_np_uint16(data: npt.ArrayLike) -> npt.NDArray[np.uint16]:
    """Convert some datat to a numpy uint16 array."""
    return np.asarray(data, dtype=np.uint16)


def to_np_uint32(data: npt.ArrayLike) -> npt.NDArray[np.uint32]:
    """Convert some datat to a numpy uint32 array."""
    return np.asarray(data, dtype=np.uint32)


def to_np_uint64(data: npt.ArrayLike) -> npt.NDArray[np.uint64]:
    """Convert some datat to a numpy uint64 array."""
    return np.asarray(data, dtype=np.uint64)


def to_np_int8(data: npt.ArrayLike) -> npt.NDArray[np.int8]:
    """Convert some datat to a numpy int8 array."""
    return np.asarray(data, dtype=np.int8)


def to_np_int16(data: npt.ArrayLike) -> npt.NDArray[np.int16]:
    """Convert some datat to a numpy int16 array."""
    return np.asarray(data, dtype=np.int16)


def to_np_int32(data: npt.ArrayLike) -> npt.NDArray[np.int32]:
    """Convert some datat to a numpy int32 array."""
    return np.asarray(data, dtype=np.int32)


def to_np_int64(data: npt.ArrayLike) -> npt.NDArray[np.int64]:
    """Convert some datat to a numpy int64 array."""
    return np.asarray(data, dtype=np.int64)


def to_np_bool(data: npt.ArrayLike) -> npt.NDArray[np.bool_]:
    """Convert some datat to a numpy bool array."""
    return np.asarray(data, dtype=np.bool_)


def to_np_float16(data: npt.ArrayLike) -> npt.NDArray[np.float16]:
    """Convert some datat to a numpy float16 array."""
    return np.asarray(data, dtype=np.float16)


def to_np_float32(data: npt.ArrayLike) -> npt.NDArray[np.float32]:
    """Convert some datat to a numpy float32 array."""
    return np.asarray(data, dtype=np.float32)


def to_np_float64(data: npt.ArrayLike) -> npt.NDArray[np.float64]:
    """Convert some datat to a numpy float64 array."""
    return np.asarray(data, dtype=np.float64)
