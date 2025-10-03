use std::fmt;

use serde::de::{self, Visitor};

pub(super) struct PrimitiveVisitor<T>(std::marker::PhantomData<T>);

impl<T> PrimitiveVisitor<T> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

macro_rules! impl_primitive_visitor {
    ($t:ty, $m:ident) => {
        impl Visitor<'_> for PrimitiveVisitor<$t> {
            type Value = $t;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, stringify!($t))
            }

            fn $m<E>(self, v: $t) -> Result<$t, E> {
                Ok(v)
            }
        }
    };
}

impl_primitive_visitor!(i8, visit_i8);
impl_primitive_visitor!(u8, visit_u8);
impl_primitive_visitor!(i16, visit_i16);
impl_primitive_visitor!(u16, visit_u16);
impl_primitive_visitor!(i32, visit_i32);
impl_primitive_visitor!(u32, visit_u32);
impl_primitive_visitor!(i64, visit_i64);
impl_primitive_visitor!(u64, visit_u64);
impl_primitive_visitor!(f32, visit_f32);
impl_primitive_visitor!(f64, visit_f64);

impl Visitor<'_> for PrimitiveVisitor<bool> {
    type Value = bool;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bool")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(v)
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v != 0)
    }
}

pub(super) struct StringVisitor;

impl Visitor<'_> for StringVisitor {
    type Value = String;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "string")
    }

    fn visit_string<E>(self, v: String) -> Result<String, E> {
        Ok(v)
    }

    fn visit_str<E>(self, v: &str) -> Result<String, E>
    where
        E: de::Error,
    {
        Ok(v.to_owned())
    }
}
