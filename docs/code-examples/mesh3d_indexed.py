"""Log a simple colored triangle."""
import rerun as rr

rr.init("rerun_example_mesh3d_indexed", spawn=True)

rr.log(
    "triangle",
    rr.Mesh3D(
        [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        vertex_normals=[0.0, 0.0, 1.0],
        vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
        mesh_properties=rr.cmp.MeshProperties(vertex_indices=[2, 1, 0]),
        mesh_material=rr.cmp.Material(albedo_factor=[0xCC, 0x00, 0xCC, 0xFF]),
    ),
)
