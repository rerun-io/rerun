-- rerun entity_path="/robot/sensor/joint"
SELECT
  *,
  describe("jointstate.JointStates:jointstates", 'rerun.archetypes.Scalars', 'Scalars:scalars', 'Scalars:yeah') AS test
FROM input
