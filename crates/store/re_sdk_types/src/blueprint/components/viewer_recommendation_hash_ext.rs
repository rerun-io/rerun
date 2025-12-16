use super::ViewerRecommendationHash;

impl std::hash::Hash for ViewerRecommendationHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.0);
    }
}

impl nohash_hasher::IsEnabled for ViewerRecommendationHash {}

impl Default for ViewerRecommendationHash {
    #[inline]
    fn default() -> Self {
        // Not a great default either way and we don't need it in the ui really.
        // But making an exception to the rule of having a default for all components just for this one isn't worth it.
        Self(0.into())
    }
}
