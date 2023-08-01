from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from .. import Arrow3DArrayLike

# TODO(#2884): These numpy->AoS->arrow roundtrips are ridiculously inefficient.


def arrow3d_native_to_pa_array(data: Arrow3DArrayLike, data_type: pa.DataType) -> pa.Array:
    from rerun.experimental import dt as rrd

    from .. import Arrow3D

    # TODO(ab): not quite sure why i must unwrap `xyz` or face a cryptic error otherwise.

    if isinstance(data, Arrow3D):
        origins = rrd.Vec3DArray.from_similar(data.origin.xyz).storage
        vectors = rrd.Vec3DArray.from_similar(data.vector.xyz).storage
    else:
        origins = rrd.Vec3DArray.from_similar([d.origin.xyz for d in data]).storage
        vectors = rrd.Vec3DArray.from_similar([d.vector.xyz for d in data]).storage

    return pa.StructArray.from_arrays(
        arrays=[origins, vectors],
        fields=list(data_type),
    )
