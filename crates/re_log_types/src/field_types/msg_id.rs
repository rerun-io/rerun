use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

/// A unique id per [`crate::LogMsg`].
///
/// ```
/// use re_log_types::field_types::MsgId;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     MsgId::data_type(),
///     DataType::Struct(vec![
///         Field::new("time_ns", DataType::UInt64, false),
///         Field::new("inc", DataType::UInt64, false),
///     ])
/// );
/// ```
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MsgId(re_tuid::Tuid);

impl std::fmt::Display for MsgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0.as_u128())
    }
}

impl MsgId {
    /// All zeroes.
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);

    /// All ones.
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    #[inline]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn random() -> Self {
        Self(re_tuid::Tuid::random())
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        self.0.as_u128()
    }
}

arrow_enable_vec_for_type!(MsgId);

impl ArrowField for MsgId {
    type Type = Self;
    fn data_type() -> arrow2::datatypes::DataType {
        <re_tuid::Tuid as ArrowField>::data_type()
    }
}

impl ArrowSerialize for MsgId {
    type MutableArrayType = <re_tuid::Tuid as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        <re_tuid::Tuid as ArrowSerialize>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        <re_tuid::Tuid as ArrowSerialize>::arrow_serialize(&v.0, array)
    }
}

impl ArrowDeserialize for MsgId {
    type ArrayType = <re_tuid::Tuid as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <re_tuid::Tuid as ArrowDeserialize>::arrow_deserialize(v).map(MsgId)
    }
}

impl crate::msg_bundle::Component for MsgId {
    fn name() -> crate::ComponentName {
        "rerun.msg_id".into()
    }
}

#[test]
fn test_msgid_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let msg_ids_in = vec![MsgId::random(), MsgId::random()];
    let array: Box<dyn Array> = msg_ids_in.try_into_arrow().unwrap();
    let msg_ids_out: Vec<MsgId> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(msg_ids_in, msg_ids_out);
}
