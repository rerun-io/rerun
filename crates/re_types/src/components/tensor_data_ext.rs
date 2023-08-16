use crate::datatypes;

use super::TensorData;

/*
impl<T: TryInto<datatypes::TensorData>> TryFrom<T> for TensorData {
    type Error = <T as TryInto<datatypes::TensorData>>::Error;

    fn try_from(value: T) -> Result<Self, Self::Error> {
        Ok(Self(value.into()?))
    }
}
*/
