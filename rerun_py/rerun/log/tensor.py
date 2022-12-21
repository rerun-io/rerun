from typing import Iterable, Optional, Union

import numpy as np
import numpy.typing as npt

from rerun import rerun_bindings  # type: ignore[attr-defined]

__all__ = [
    "log_tensor",
]

TensorDType = Union[
    np.uint8, np.uint16, np.uint32, np.uint64, np.int8, np.int16, np.int32, np.int64, np.float16, np.float32, np.float64
]


def log_tensor(
    obj_path: str,
    tensor: npt.NDArray[TensorDType],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    timeless: bool = False,
) -> None:
    _log_tensor(obj_path, tensor=tensor, names=names, meter=meter, timeless=timeless)


def _log_tensor(
    obj_path: str,
    tensor: npt.NDArray[TensorDType],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    meaning: rerun_bindings.TensorDataMeaning = None,
    timeless: bool = False,
) -> None:
    """Log a general tensor, perhaps with named dimensions."""
    if names is not None:
        names = list(names)
        assert len(tensor.shape) == len(names)
    if tensor.dtype == "uint8":
        rerun_bindings.log_tensor_u8(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "uint16":
        rerun_bindings.log_tensor_u16(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "uint32":
        rerun_bindings.log_tensor_u32(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "uint64":
        rerun_bindings.log_tensor_u64(obj_path, tensor, names, meter, meaning, timeless)

    elif tensor.dtype == "int8":
        rerun_bindings.log_tensor_i8(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "int16":
        rerun_bindings.log_tensor_i16(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "int32":
        rerun_bindings.log_tensor_i32(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "int64":
        rerun_bindings.log_tensor_i64(obj_path, tensor, names, meter, meaning, timeless)

    elif tensor.dtype == "float16":
        rerun_bindings.log_tensor_f16(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "float32":
        rerun_bindings.log_tensor_f32(obj_path, tensor, names, meter, meaning, timeless)
    elif tensor.dtype == "float64":
        rerun_bindings.log_tensor_f64(obj_path, tensor, names, meter, meaning, timeless)

    else:
        raise TypeError(f"Unsupported dtype: {tensor.dtype}")
