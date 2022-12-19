use arrow2::{array::TryPush, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// The Instance used to identify an entity within a batch
///
/// ```
/// use re_log_types::field_types::Instance;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Instance::data_type(), DataType::UInt64);
/// ```
#[derive(Debug)]
pub struct Instance(pub u64);

arrow_enable_vec_for_type!(Instance);

impl ArrowField for Instance {
    type Type = Self;
    fn data_type() -> DataType {
        <u64 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Instance {
    type MutableArrayType = <u64 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for Instance {
    type ArrayType = <u64 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u64 as ArrowDeserialize>::arrow_deserialize(v).map(Instance)
    }
}

impl Component for Instance {
    fn name() -> crate::ComponentName {
        "rerun.instance".into()
    }
}
