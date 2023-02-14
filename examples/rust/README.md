# Rerun Rust Examples
These are examples for how to use the [`rerun`](https://github.com/rerun-io/rerun/tree/latest/crates/rerun) crate.

To run the minimal example:
* `cargo r -p minimal`

## Examples:

### [`minimal`](minimal)
A minimal example, showing how to log a point cloud.

### [`objectron`](objectron)
Demonstrates how to log:
* Points
* Images
* Camera extrinsics (rigid transform)
* Camera intrinsics (pinhole transform)

### [`raw_mesh`](raw_mesh)
Reads a GLTF mesh file and logs it to Rerun, preserving the transform hierarchy of the GLTF file.

Demonstrates how to log:
* Triangle meshes
* Rigid transforms

## Others

### [`api_demo`](api_demo)
A mess of an example, showing a lot of different things.
