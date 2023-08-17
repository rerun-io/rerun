use super::Tensor;

impl Tensor {
    pub fn try_from<T: TryInto<crate::datatypes::TensorData>>(data: T) -> Result<Self, T::Error> {
        let data: crate::datatypes::TensorData = data.try_into()?;

        Ok(Self { data: data.into() })
    }

    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self {
            data: crate::datatypes::TensorData {
                id,
                shape: self.data.0.shape,
                buffer: self.data.0.buffer,
            }
            .into(),
        }
    }
}
