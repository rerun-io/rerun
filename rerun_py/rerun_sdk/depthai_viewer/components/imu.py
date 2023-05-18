from typing import Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory
from depthai_viewer.components.point import Point3DArray, Point3DType
from depthai_viewer.components.quaternion import QuaternionArray

__all__ = ["ImuType", "Imu"]


class Imu(pa.ExtensionArray):  # type: ignore[misc]
    def create(
        accel: npt.NDArray[np.float32],
        gyro: npt.NDArray[np.float32],
        orientation: npt.NDArray[np.float32],
        mag: Union[npt.NDArray[np.float32], None] = None,
    ) -> "Imu":
        """Build Imu data from acceleration and gyroscope data."""
        assert accel.shape[0] == 3
        assert gyro.shape[0] == 3
        accel_point = Point3DArray.from_numpy(accel.reshape(1, 3))
        gyro_point = Point3DArray.from_numpy(gyro.reshape(1, 3))
        quat = QuaternionArray.from_numpy(np.array(orientation, dtype=np.float32).reshape(1, 4))
        mag_point = pa.nulls(1, type=Point3DType.storage_type)
        if mag is not None:
            mag_point = Point3DArray.from_numpy(np.array(mag, dtype=np.float32).reshape(1, 3))
        return pa.StructArray.from_arrays(  # type: ignore[no-any-return]
            fields=ImuType.storage_type,
            arrays=[accel_point, gyro_point, mag_point, quat],
            mask=pa.array([False, False, mag is None, False], type=pa.bool_()),
        )


ImuType = ComponentTypeFactory("ImuType", Imu, REGISTERED_COMPONENT_NAMES["rerun.imu"])
pa.register_extension_type(ImuType())
