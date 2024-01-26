# Spatial Transforms

Spatial transforms are transforms that apply the spatial 2D & 3D space views.
This includes affine 2D/3D transforms as well as camera projections.

Any transform component that is logged a path `parent/entity` it describes the
transform between `parent` to `parent/entity`.


## Topology
We infer a `SpatialTopology` from these transforms.
As we get more information about a scene the topology changes, but all changes are irreversible.
In practical terms this means that once a pinhole is logged we'll always assume that everything under
the entity path of that camera is in 2D space.

The spatial topology is used to determine which (spatial) visualizers can be used in which contents.
A 2D visualizer can only ever be applied to an entity when there is a valid transformation
along the path of the entity to the space view's origin.

Examples for invalid transformation paths are:
* mismatched start space
  * 2D content can not be added to a 3D space and vice versa
* several projections (a pinhole can only be applied once)
* explicit space disconnect

## `DisconnectedTransform` (former `DisconnectedSpace`)
Disconnected transform is a special transform that forbids any transformation path
from an entity to its parent and vice versa.
As such it creates an explicit break in the topology.

Like any other topological break, it is permanent. Once logged, the resulting subspaces can no longer be fused.

## Null transforms
Null transforms are handled like identity transforms, the same as not logging a transform at all.
