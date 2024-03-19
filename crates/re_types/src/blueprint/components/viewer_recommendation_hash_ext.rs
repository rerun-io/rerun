use super::ViewerRecommendationHash;

impl std::hash::Hash for ViewerRecommendationHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0 .0);
    }
}

impl nohash_hasher::IsEnabled for ViewerRecommendationHash {}
