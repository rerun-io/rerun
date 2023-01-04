use arrow2::datatypes::DataType;
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// A String label component
///
/// ```
/// use re_log_types::field_types::Label;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Label::data_type(), DataType::Utf8);
/// ```
#[derive(Debug, Clone, derive_more::From, derive_more::Into)]
pub struct Label(pub String);

arrow_enable_vec_for_type!(Label);

impl ArrowField for Label {
    type Type = Self;
    fn data_type() -> DataType {
        <String as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Label {
    type MutableArrayType = <String as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        <String as ArrowSerialize>::arrow_serialize(&v.0, array)
    }
}

impl ArrowDeserialize for Label {
    type ArrayType = <String as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <String as ArrowDeserialize>::arrow_deserialize(v).map(Label)
    }
}

impl Component for Label {
    fn name() -> crate::ComponentName {
        "rerun.label".into()
    }
}
