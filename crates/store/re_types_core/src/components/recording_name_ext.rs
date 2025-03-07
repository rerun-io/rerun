use crate::AsComponents;

impl std::fmt::Display for super::RecordingName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsComponents for super::RecordingName {
    fn as_serialized_batches(&self) -> Vec<crate::SerializedComponentBatch> {
        self.as_serialized_batches()
    }
}
