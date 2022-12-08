use arrow2::{
    array::{MutableArray, MutableStructArray, TryPush},
    datatypes::DataType,
};
use arrow2_convert::{arrow_enable_vec_for_type, field::ArrowField, serialize::ArrowSerialize};

use crate::Tuid;

arrow_enable_vec_for_type!(Tuid);

impl ArrowField for Tuid {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        DataType::Extension(
            "Tuid".to_owned(),
            Box::new(DataType::Struct(vec![
                <u64 as ArrowField>::field("time_ns"),
                <u64 as ArrowField>::field("inc"),
            ])),
            None,
        )
    }
}

impl ArrowSerialize for Tuid {
    type MutableArrayType = MutableStructArray;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        let time_ns = Box::new(<u64 as ArrowSerialize>::new_array()) as Box<dyn MutableArray>;
        let inc = Box::new(<u64 as ArrowSerialize>::new_array()) as Box<dyn MutableArray>;
        MutableStructArray::from_data(<Tuid as ArrowField>::data_type(), vec![time_ns, inc], None)
    }

    #[inline]
    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        array
            .value::<<u64 as ArrowSerialize>::MutableArrayType>(0)
            .unwrap()
            .try_push(Some(v.time_ns))?;
        array
            .value::<<u64 as ArrowSerialize>::MutableArrayType>(1)
            .unwrap()
            .try_push(Some(v.inc))?;
        array.push(true);
        Ok(())
    }
}

//TODO(john) enable this when ArrowDeserialize is implemented for Tuid
#[cfg(feature = "disabled")]
#[test]
fn test_tuid_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let tuid_in = vec![Tuid::random(), Tuid::random()];
    let array: Box<dyn Array> = tuid_in.try_into_arrow().unwrap();
    let tuid_out: Vec<Tuid> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(tuid_in, tuid_out);
}
