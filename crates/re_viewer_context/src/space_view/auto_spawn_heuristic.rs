/// Heuristic result used to determine if a Space View with a given class should be automatically spawned.
///
/// The details of how this is interpreted are up to the code determining candidates and performing
/// Space View spawning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoSpawnHeuristic {
    /// Always spawn a Space View with this class if it is a candidate.
    AlwaysSpawn,

    /// Never spawn a Space View with this class if it is a candidate.
    /// This means that the candidate can only be created manually.
    NeverSpawn,

    /// If there's several candidates for the same root, spawn only one with the highest score.
    SpawnClassWithHighestScoreForRoot(f32),
}
