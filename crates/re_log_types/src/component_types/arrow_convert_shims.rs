//! Assorted shims that should make their way back to [arrow2-convert](https://github.com/DataEngineeringLabs/arrow2-convert/)

use std::ops::Index;

use arrow2::{
    array::{Array, BinaryArray},
    buffer::Buffer,
};
use arrow2_convert::{
    deserialize::{ArrowArray, ArrowDeserialize},
    ArrowField, ArrowSerialize,
};

/// Shim to enable zero-copy arrow deserialization for `Buffer<u8>`
/// Can be removed when: [arrow2-convert#103](https://github.com/DataEngineeringLabs/arrow2-convert/pull/103) lands
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize)]
#[arrow_field(transparent)]
pub struct BinaryBuffer(pub Buffer<u8>);

impl BinaryBuffer {
    #[inline]
    pub fn num_bytes(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &u8> {
        self.0.iter()
    }
}

impl Index<usize> for BinaryBuffer {
    type Output = u8;

    #[inline]
    fn index(&self, i: usize) -> &u8 {
        &self.0[i]
    }
}

impl From<Vec<u8>> for BinaryBuffer {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Self(v.into())
    }
}

/// Iterator for for [`BufferBinaryArray`]
pub struct BufferBinaryArrayIter<'a> {
    index: usize,
    array: &'a BinaryArray<i32>,
}

impl<'a> Iterator for BufferBinaryArrayIter<'a> {
    type Item = Option<Buffer<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.array.len() {
            None
        } else {
            if let Some(validity) = self.array.validity() {
                if !validity.get_bit(self.index) {
                    self.index += 1;
                    return Some(None);
                }
            }
            let (start, end) = self.array.offsets().start_end(self.index);
            self.index += 1;
            Some(Some(self.array.values().clone().slice(start, end - start)))
        }
    }
}

/// Internal `ArrowArray` helper to iterate over a `BinaryArray` while exposing Buffer slices
pub struct BufferBinaryArray;

#[cfg(not(target_os = "windows"))]
extern "C" {
    fn do_not_call_into_iter(); // we never define this function, so the linker will fail
}

impl<'a> IntoIterator for &'a BufferBinaryArray {
    type Item = Option<Buffer<u8>>;

    type IntoIter = BufferBinaryArrayIter<'a>;

    #[cfg(not(target_os = "windows"))]
    fn into_iter(self) -> Self::IntoIter {
        #[allow(unsafe_code)]
        // SAFETY:
        // This exists so we get a link-error if some code tries to call into_iter
        // Iteration should only happen via iter_from_array_ref.
        // This is a quirk of the way the traits work in arrow2_convert.
        unsafe {
            do_not_call_into_iter();
        }
        unreachable!()
    }

    // On windows the above linker trick doesn't work.
    // We'll still catch the issue on build in Linux, but on windows just fall back to panic.
    #[cfg(target_os = "windows")]
    fn into_iter(self) -> Self::IntoIter {
        panic!("Use iter_from_array_ref. This is a quirk of the way the traits work in arrow2_convert.");
    }
}

impl ArrowArray for BufferBinaryArray {
    type BaseArrayType = BinaryArray<i32>;
    #[inline]
    fn iter_from_array_ref(a: &dyn Array) -> <&Self as IntoIterator>::IntoIter {
        let b = a.as_any().downcast_ref::<Self::BaseArrayType>().unwrap();

        BufferBinaryArrayIter { index: 0, array: b }
    }
}

impl ArrowDeserialize for BinaryBuffer {
    type ArrayType = BufferBinaryArray;

    #[inline]
    fn arrow_deserialize(v: Option<Buffer<u8>>) -> Option<Self> {
        v.map(BinaryBuffer)
    }
}
