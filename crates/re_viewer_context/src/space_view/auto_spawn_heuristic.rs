/// Heuristic result used to determine if a Space View with a given class should be automatically spawned.
///
/// The details of how this is interpreted are up to the code determining candidates and performing
/// Space View spawning.
#[deprecated(note = "Use `SpaceViewSpawnHeuristics` instead")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoSpawnHeuristic {
    /// Always spawn the Space View if it is a candidate.
    AlwaysSpawn,

    /// Never spawn the Space View if it is a candidate.
    /// This means that the candidate can only be created manually.
    NeverSpawn,

    /// If there's several candidates for the same root, spawn only one with the highest score.
    ///
    /// Right now there is an implicit assumption that all SpaceViews which return
    /// [`AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot`] are in some sense equivalent.
    ///
    /// TODO(andreas): This might be true today but ends up being a bit limiting.
    /// Adding something like a `equivalency_id` along with the score would let us be a bit more explicit.
    SpawnClassWithHighestScoreForRoot(f32),
}
