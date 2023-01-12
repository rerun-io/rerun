use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::{msg_bundle::Component, ComponentName};

/// A unique id per [`crate::LogMsg`].
///
/// ## Examples
///
/// ```
/// # use re_log_types::field_types::MsgId;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     MsgId::data_type(),
///     DataType::Struct(vec![
///         Field::new("time_ns", DataType::UInt64, false),
///         Field::new("inc", DataType::UInt64, false),
///     ])
/// );
/// ```
#[derive(
    Clone,
    Copy,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
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

impl Component for MsgId {
    fn name() -> ComponentName {
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
