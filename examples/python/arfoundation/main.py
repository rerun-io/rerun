#!/usr/bin/env python3
"""
Rerun with Unity ARFoundation.

Run:
```sh
pip install -r examples/python/arfoundation/requirements.txt
python examples/python/arfoundation/main.py
```
"""
from __future__ import annotations

import uuid
from concurrent import futures
from typing import Dict

import grpc
import numpy as np
import rerun as rr

from . import service_pb2, service_pb2_grpc

sessions: Dict[str, service_pb2.RegisterRequest] = {}
rr.init("rerun_ar", spawn=True)


class RerunARService(service_pb2_grpc.RerunARService):
    """Rerun AR gRPC service."""

    def register(self, request: service_pb2.RegisterRequest, context: grpc.ServicerContext) -> service_pb2.RegisterResponse:
        uid = str(uuid.uuid4())
        sessions[uid] = request

        print("Registered %s" % uid, request)

        return service_pb2.RegisterResponse(message=uid)

    def data_frame(self, request: service_pb2.DataFrameRequest, context: grpc.ServicerContext) -> service_pb2.DataFrameResponse:
        session_configs = sessions[request.uid]

        # Log RGB image (from YCbCr).
        color_img = np.frombuffer(request.color, dtype=np.uint8)
        color_img_w = session_configs.color_sample_size_x
        color_img_h = session_configs.color_sample_size_y
        p = color_img_w * color_img_h

        y = color_img[:p].reshape((color_img_h, color_img_w))
        cbcr = color_img[p:].reshape((color_img_h // 2, color_img_w // 2, 2))
        cb, cr = cbcr[:, :, 0], cbcr[:, :, 1]

        # Very important! Convert to float32 first!
        cb = np.repeat(cb, 2, axis=0).repeat(2, axis=1).astype(np.float32) - 128
        cr = np.repeat(cr, 2, axis=0).repeat(2, axis=1).astype(np.float32) - 128

        r = np.clip(y + 1.403 * cr, 0, 255)
        g = np.clip(y - 0.344 * cb - 0.714 * cr, 0, 255)
        b = np.clip(y + 1.772 * cb, 0, 255)

        color_rgb = np.stack([r, g, b], axis=-1)
        color_rgb = color_rgb.astype(np.uint8)
        # rr.log("rgb", rr.Image(color_rgb))

        # Log depth image.
        depth_img = np.frombuffer(request.depth, dtype=np.float32)
        depth_img = depth_img.reshape(
            (session_configs.depth_resolution_y, session_configs.depth_resolution_x)
        )
        rr.log("depth", rr.DepthImage(depth_img, meter=1.0))

        # Log point cloud.
        sx = session_configs.color_resolution_x / session_configs.color_sample_size_x
        sy = session_configs.color_resolution_y / session_configs.color_sample_size_y

        u, v = np.meshgrid(np.arange(color_img_w), np.arange(color_img_h))
        fx, fy = (
            session_configs.focal_length_x / sx,
            session_configs.focal_length_y / sy,
        )
        cx, cy = (
            session_configs.principal_point_x / sx,
            session_configs.principal_point_y / sy,
        )

        # Flip image is needed for point cloud generation.
        color_rgb = np.flipud(color_rgb)
        depth_img = np.flipud(depth_img)

        y_down_to_y_up = np.array(
            [
                [1.0, -0.0, 0.0, 0],
                [0.0, -1.0, 0.0, 0],
                [0.0, 0.0, 1.0, 0],
                [0.0, 0.0, 0, 1.0],
            ],
            dtype=np.float32,
        )

        t = np.frombuffer(request.transform, dtype=np.float32)
        transform = np.eye(4)
        transform[:3, :] = t.reshape((3, 4))
        transform[:3, 3] = 0
        transform = y_down_to_y_up @ transform
        k = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])

        entity_3d = "world"
        rr.log(entity_3d, rr.ViewCoordinates.RIGHT_HAND_Y_DOWN)

        rr.log(
            f"{entity_3d}/camera",
            rr.Transform3D(mat3x3=transform[:3, :3], translation=transform[:3, 3]),
        )
        rr.log(
            f"{entity_3d}/camera",
            rr.Pinhole(resolution=[color_img_w, color_img_h], image_from_camera=k),
        )
        rr.log(f"{entity_3d}/camera", rr.Image(color_rgb))

        z = depth_img.copy()
        x = ((u - cx) * z) / fx
        y = ((v - cy) * z) / fy
        pcd = np.stack([x, y, z], axis=-1).reshape(-1, 3)
        pcd = np.matmul(transform[:3, :3], pcd.T).T + transform[:3, 3]
        clr = color_rgb.reshape(-1, 3)
        rr.log(f"{entity_3d}/point_cloud", rr.Points3D(pcd, colors=clr))

        return service_pb2.DataFrameResponse(message="OK")


def main() -> None:
    """Run gRPC server."""
    port = 8500
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    service_pb2_grpc.add_RerunARServiceServicer_to_server(RerunARService(), server)
    server.add_insecure_port("[::]:%s" % port)
    server.start()

    print("Register server started on port %s" % port)
    server.wait_for_termination()


if __name__ == "__main__":
    main()
