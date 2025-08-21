-- rerun entity_path="/world/Lantern/**"
SELECT
  * EXCEPT "Mesh3D:vertex_positions",
  describe("Mesh3D:vertex_positions", 'rerun.archetypes.Points3D', 'Points3D:positions', 'rerun.components.Position3D')
FROM input
