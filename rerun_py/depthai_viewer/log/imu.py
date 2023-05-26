from typing import Any, Dict, Union

import numpy as np
import numpy.typing as npt

from depthai_viewer import bindings
from depthai_viewer.components.imu import Imu
from depthai_viewer.log.log_decorator import log_decorator


@log_decorator
def log_imu(
    accel: npt.ArrayLike, gyro: npt.ArrayLike, orientation: npt.ArrayLike, mag: Union[npt.ArrayLike, None] = None
) -> None:
    """
    Log an IMU sensor reading.

    Parameters
    ----------
    entity_path:
        Path to the IMU sensor in the space hierarchy.
    accel:
        Acceleration vector in m/s^2.
    gyro:
        Angular velocity vector in rad/s.
    orientation:
        Orientation quaternion.
    mag:
        Magnetometer vector in uT.
    """

    if accel is not None:
        accel = np.require(accel, dtype=np.float32)
    else:
        raise ValueError("Acceleration vector cannot be None")
    if gyro is not None:
        gyro = np.require(gyro, dtype=np.float32)
    else:
        raise ValueError("angular velocity vector cannot be None")
    if orientation is not None:
        orientation = np.require(orientation, dtype=np.float32)
    else:
        raise ValueError("orientation vector cannot be None")

    instanced: Dict[str, Any] = {}
    if accel.size != 3:
        raise ValueError(f"Acceleration vector must have a length of 3, got: {accel.size}")
    if gyro.size != 3:
        raise ValueError(f"Angular velocity vector must have a length of 3, got: {gyro.size}")

    if orientation.size != 4:
        raise ValueError(f"Orientation quaternion must have a length of 4, got: {orientation.size}")

    instanced["rerun.imu"] = Imu.create(accel, gyro, orientation, mag)  # type: ignore[arg-type]
    # Fixed imu entity path
    bindings.log_arrow_msg("imu_data", components=instanced, timeless=False)
