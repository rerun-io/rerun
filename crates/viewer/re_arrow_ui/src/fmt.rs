// TODO: Remove unsafe
#![allow(unsafe_code)]
// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Functions for printing array values as human-readable strings.
//!
//! This is often used for debugging or logging purposes.
//!
//! See the [`pretty`] crate for additional functions for
//! record batch pretty printing.
//!
//! [`pretty`]: crate::pretty
// use arrow::array::{
//     Array, AsArray, BooleanArray, DictionaryArray, FixedSizeBinaryArray, FixedSizeListArray,
//     GenericListArray, MapArray, NullArray, OffsetSizeTrait, PrimitiveArray, RunArray, StructArray,
//     UnionArray, as_boolean_array, as_generic_list_array, as_map_array, as_null_array,
//     as_struct_array, as_union_array, downcast_run_array,
// };
// use arrow::datatypes::{
//     ArrowDictionaryKeyType, ArrowNativeType, DataType, Decimal128Type, Decimal256Type, Float16Type,
//     Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type, RunEndIndexType,
//     UInt8Type, UInt16Type, UInt32Type, UInt64Type, UnionMode,
// };
// use arrow::error::ArrowError;
// use arrow::{downcast_dictionary_array, downcast_primitive_array};
use arrow::array::cast::*;
use arrow::array::temporal_conversions::*;
use arrow::array::timezone::Tz;
use arrow::array::types::*;
use arrow::array::*;
// use arrow::buffer::ArrowNativeType;
use arrow::*;
// use chrono::{NaiveDate, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use arrow::datatypes::{ArrowNativeType, DataType, UnionMode};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use chrono::{NaiveDate, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use egui::Ui;
use lexical_core::FormattedSize;
use re_ui::UiExt;
use re_ui::list_item::PropertyContent;
use std::fmt::{Display, Formatter, Write};
use std::ops::Range;

type TimeFormat<'a> = Option<&'a str>;

/// Implements [`Display`] for a specific array value
pub struct ValueFormatter<'a> {
    idx: usize,
    formatter: &'a ArrayUi<'a>,
}

impl ValueFormatter<'_> {
    /// Writes this value to the provided [`Write`]
    ///
    /// Note: this ignores [`FormatOptions::with_display_error`] and
    /// will return an error on formatting issue
    pub fn write(&self, s: &mut dyn Write) -> Result<(), ArrowError> {
        match self.formatter.format.write(self.idx, s) {
            Ok(_) => Ok(()),
            Err(FormatError::Arrow(e)) => Err(e),
            Err(FormatError::Format(_)) => Err(ArrowError::CastError("Format error".to_string())),
        }
    }

    /// Fallibly converts this to a string
    pub fn try_to_string(&self) -> Result<String, ArrowError> {
        let mut s = String::new();
        self.write(&mut s)?;
        Ok(s)
    }
}

impl Display for ValueFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.formatter.format.write(self.idx, f) {
            Ok(()) => Ok(()),
            Err(FormatError::Arrow(e)) => {
                write!(f, "ERROR: {e}")
            }
            Err(_) => Err(std::fmt::Error),
        }
    }
}

/// A string formatter for an [`Array`]
///
/// This can be used with [`std::write`] to write type-erased `dyn Array`
///
/// ```
/// # use std::fmt::{Display, Formatter, Write};
/// # use arrow_array::{Array, ArrayRef, Int32Array};
/// # use arrow_cast::display::{ArrayFormatter, FormatOptions};
/// # use arrow_schema::ArrowError;
/// struct MyContainer {
///     values: ArrayRef,
/// }
///
/// impl Display for MyContainer {
///     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
///         let options = FormatOptions::default();
///         let formatter = ArrayFormatter::try_new(self.values.as_ref(), &options)
///             .map_err(|_| std::fmt::Error)?;
///
///         let mut iter = 0..self.values.len();
///         if let Some(idx) = iter.next() {
///             write!(f, "{}", formatter.value(idx))?;
///         }
///         for idx in iter {
///             write!(f, ", {}", formatter.value(idx))?;
///         }
///         Ok(())
///     }
/// }
/// ```
///
/// [`ValueFormatter::write`] can also be used to get a semantic error, instead of the
/// opaque [`std::fmt::Error`]
///
/// ```
/// # use std::fmt::Write;
/// # use arrow_array::Array;
/// # use arrow_cast::display::{ArrayFormatter, FormatOptions};
/// # use arrow_schema::ArrowError;
/// fn format_array(
///     f: &mut dyn Write,
///     array: &dyn Array,
///     options: &FormatOptions,
/// ) -> Result<(), ArrowError> {
///     let formatter = ArrayFormatter::try_new(array, options)?;
///     for i in 0..array.len() {
///         formatter.value(i).write(f)?
///     }
///     Ok(())
/// }
/// ```
///
pub struct ArrayUi<'a> {
    format: Box<dyn ShowIndex + 'a>,
}

impl<'a> ArrayUi<'a> {
    /// Returns an [`ArrayUi`] that can be used to format `array`
    ///
    /// This returns an error if an array of the given data type cannot be formatted
    pub fn try_new(array: &'a dyn Array, options: &FormatOptions<'a>) -> Result<Self, ArrowError> {
        Ok(Self {
            format: make_formatter(array, options)?,
        })
    }

    /// Returns a [`ValueFormatter`] that implements [`Display`] for
    /// the value of the array at `idx`
    pub fn value(&self, idx: usize) -> ValueFormatter<'_> {
        ValueFormatter {
            formatter: self,
            idx,
        }
    }

    pub fn show_value(&self, idx: usize, ui: &mut Ui) {
        // self.format.show(idx, ui);
        let node = ArrowNode::index(idx, &*self.format);
        node.show(ui, idx);
    }
}

fn make_formatter<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    downcast_primitive_array! {
        array => show_primitive(array, options),
        DataType::Null => show_primitive(as_null_array(array), options),
        DataType::Boolean => show_primitive(as_boolean_array(array), options),
        DataType::Utf8 => show_primitive(array.as_string::<i32>(), options),
        DataType::LargeUtf8 => show_primitive(array.as_string::<i64>(), options),
        DataType::Utf8View => show_primitive(array.as_string_view(), options),
        DataType::Binary => show_primitive(array.as_binary::<i32>(), options),
        DataType::BinaryView => show_primitive(array.as_binary_view(), options),
        DataType::LargeBinary => show_primitive(array.as_binary::<i64>(), options),
        DataType::FixedSizeBinary(_) => {
            let a = array.as_any().downcast_ref::<FixedSizeBinaryArray>().unwrap();
            show_primitive(a, options)
        }
        DataType::Dictionary(_, _) => downcast_dictionary_array! {
            array => array_format(array, options),
            _ => unreachable!()
        }
        DataType::List(_) => array_format(as_generic_list_array::<i32>(array), options),
        DataType::LargeList(_) => array_format(as_generic_list_array::<i64>(array), options),
        DataType::FixedSizeList(_, _) => {
            let a = array.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
            array_format(a, options)
        }
        DataType::Struct(_) => array_format(as_struct_array(array), options),
        DataType::Map(_, _) => array_format(as_map_array(array), options),
        DataType::Union(_, _) => array_format(as_union_array(array), options),
        DataType::RunEndEncoded(_, _) => downcast_run_array! {
            array => array_format(array, options),
            _ => unreachable!()
        },
        d => Err(ArrowError::NotYetImplemented(format!("formatting {d} is not yet supported"))),
    }
}

impl<'a> ShowIndex for ArrayFormatter<'a> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        self.value(idx).write(f)?;
        Ok(())
    }
}

fn show_primitive<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    Ok(Box::new(ArrayFormatter::try_new(array, options)?))
}

struct ShowPrimitive<'a> {
    formatter: arrow::util::display::ArrayFormatter<'a>,
    array: &'a dyn Array,
}

impl<'a> ShowIndexState<'a> for ShowPrimitive<'a> {
    type State = arrow::util::display::ArrayFormatter<'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let formatter = arrow::util::display::ArrayFormatter::try_new(self.array, options)?;
        Ok(formatter)
    }

    fn write(&self, state: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        state.value(idx).write(f)?;
        Ok(())
    }
}

/// Either an [`ArrowError`] or [`std::fmt::Error`]
enum FormatError {
    Format(std::fmt::Error),
    Arrow(ArrowError),
}

type FormatResult = Result<(), FormatError>;

impl From<std::fmt::Error> for FormatError {
    fn from(value: std::fmt::Error) -> Self {
        Self::Format(value)
    }
}

impl From<ArrowError> for FormatError {
    fn from(value: ArrowError) -> Self {
        Self::Arrow(value)
    }
}

/// [`Display`] but accepting an index
trait ShowIndex {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult;

    fn show(&self, idx: usize, ui: &mut Ui) {
        let mut text = String::new();
        let result = self.write(idx, &mut text);
        ui.label(text);
    }

    fn is_item_nested(&self) -> bool {
        false
    }
}

/// [`ShowIndex`] with additional state
trait ShowIndexState<'a> {
    type State;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError>;

    fn write(&self, state: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult;

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let mut text = String::new();
        let result = self.write(state, idx, &mut text);
        ui.label(text);
    }

    fn is_item_nested(&self) -> bool {
        false
    }
}

impl<'a, T: ShowIndex> ShowIndexState<'a> for T {
    type State = ();

    fn prepare(&self, _options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok(())
    }

    fn write(&self, _: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        ShowIndex::write(self, idx, f)
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        ShowIndex::show(self, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        ShowIndex::is_item_nested(self)
    }
}

struct ArrayFormat<'a, F: ShowIndexState<'a>> {
    state: F::State,
    array: F,
    null: &'a str,
}

fn array_format<'a, F>(
    array: F,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError>
where
    F: ShowIndexState<'a> + Array + 'a,
{
    let state = array.prepare(options)?;
    Ok(Box::new(ArrayFormat {
        state,
        array,
        null: "null",
    }))
}

impl<'a, F: ShowIndexState<'a> + Array> ShowIndex for ArrayFormat<'a, F> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        if self.array.is_null(idx) {
            if !self.null.is_empty() {
                f.write_str(self.null)?
            }
            return Ok(());
        }
        ShowIndexState::write(&self.array, &self.state, idx, f)
    }

    fn show(&self, idx: usize, ui: &mut Ui) {
        ShowIndexState::show(&self.array, &self.state, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        ShowIndexState::is_item_nested(&self.array)
    }
}

impl ShowIndex for &BooleanArray {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

impl<'a> ShowIndexState<'a> for &'a NullArray {
    type State = &'a str;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok("null")
    }

    fn write(&self, state: &Self::State, _idx: usize, f: &mut dyn Write) -> FormatResult {
        f.write_str(state)?;
        Ok(())
    }
}

macro_rules! primitive_display {
    ($($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
                let value = self.value(idx);
                let mut buffer = [0u8; <$t as ArrowPrimitiveType>::Native::FORMATTED_SIZE];
                let b = lexical_core::write(value, &mut buffer);
                // Lexical core produces valid UTF-8
                let s = unsafe { std::str::from_utf8_unchecked(b) };
                f.write_str(s)?;
                Ok(())
            }
        })+
    };
}

macro_rules! primitive_display_float {
    ($($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
                let value = self.value(idx);
                let mut buffer = ryu::Buffer::new();
                f.write_str(buffer.format(value))?;
                Ok(())
            }
        })+
    };
}

primitive_display!(Int8Type, Int16Type, Int32Type, Int64Type);
primitive_display!(UInt8Type, UInt16Type, UInt32Type, UInt64Type);
primitive_display_float!(Float32Type, Float64Type);

impl ShowIndex for &PrimitiveArray<Float16Type> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

macro_rules! decimal_display {
    ($($t:ty),+) => {
        $(impl<'a> ShowIndexState<'a> for &'a PrimitiveArray<$t> {
            type State = (u8, i8);

            fn prepare(&self, _options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
                Ok((self.precision(), self.scale()))
            }

            fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
                write!(f, "{}", <$t>::format_decimal(self.values()[idx], s.0, s.1))?;
                Ok(())
            }
        })+
    };
}

decimal_display!(Decimal128Type, Decimal256Type);

impl<'a, K: ArrowDictionaryKeyType> ShowIndexState<'a> for &'a DictionaryArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_formatter(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.keys().values()[idx].as_usize();
        s.as_ref().write(value_idx, f)
    }
}

impl<'a, K: RunEndIndexType> ShowIndexState<'a> for &'a RunArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_formatter(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.get_physical_index(idx);
        s.as_ref().write(value_idx, f)
    }
}

fn write_list(f: &mut dyn Write, mut range: Range<usize>, values: &dyn ShowIndex) -> FormatResult {
    f.write_char('[')?;
    if let Some(idx) = range.next() {
        values.write(idx, f)?;
    }
    for idx in range {
        write!(f, ", ")?;
        values.write(idx, f)?;
    }
    f.write_char(']')?;
    Ok(())
}

enum NodeLabel {
    Index(usize),
    Name(String),
}
pub struct ArrowNode<'a> {
    label: NodeLabel,
    values: &'a dyn ShowIndex,
}

impl<'a> ArrowNode<'a> {
    pub fn name(name: &str, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Name(name.to_string()),
            values,
        }
    }

    pub fn index(idx: usize, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Index(idx),
            values,
        }
    }

    pub fn show(&self, ui: &mut Ui, index: usize) {
        let label = match &self.label {
            NodeLabel::Index(idx) => format!("{idx}"),
            NodeLabel::Name(name) => name.clone(),
        };

        let mut value = String::new();
        self.values.write(index, &mut value); // TODO: Handle error

        let nested = self.values.is_item_nested();

        let mut item = ui.list_item();
        let id = ui.unique_id().with(index).with(&label);
        let mut content = PropertyContent::new(label).value_text(value);

        if nested {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                self.values.show(index, ui);
            });
        } else {
            item.show_hierarchical(ui, content);
        }
    }
}

fn list_ui(ui: &mut Ui, mut range: Range<usize>, values: &dyn ShowIndex) {
    let mut label_index = 0;
    for idx in range {
        let node = ArrowNode::index(label_index, values);
        node.show(ui, idx);
        label_index += 1;
    }
}

impl<'a, O: OffsetSizeTrait> ShowIndexState<'a> for &'a GenericListArray<O> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_formatter(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        write_list(f, start..end, s.as_ref())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        list_ui(ui, start..end, state.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        dbg!(self.data_type());
        self.data_type().is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a FixedSizeListArray {
    type State = (usize, Box<dyn ShowIndex + 'a>);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let values = make_formatter(self.values().as_ref(), options)?;
        let length = self.value_length();
        Ok((length as usize, values))
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let start = idx * s.0;
        let end = start + s.0;
        write_list(f, start..end, s.1.as_ref())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let start = idx * state.0;
        let end = start + state.0;
        list_ui(ui, start..end, state.1.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        true
    }
}

/// Pairs a boxed [`ShowIndex`] with its field name
type FieldDisplay<'a> = (&'a str, Box<dyn ShowIndex + 'a>);

impl<'a> ShowIndexState<'a> for &'a StructArray {
    type State = Vec<FieldDisplay<'a>>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let fields = match (*self).data_type() {
            DataType::Struct(f) => f,
            _ => unreachable!(),
        };

        self.columns()
            .iter()
            .zip(fields)
            .map(|(a, f)| {
                let format = make_formatter(a.as_ref(), options)?;
                Ok((f.name().as_str(), format))
            })
            .collect()
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let mut iter = s.iter();
        f.write_char('{')?;
        if let Some((name, display)) = iter.next() {
            write!(f, "{name}: ")?;
            display.as_ref().write(idx, f)?;
        }
        for (name, display) in iter {
            write!(f, ", {name}: ")?;
            display.as_ref().write(idx, f)?;
        }
        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        for (name, display) in state {
            let node = ArrowNode::name(name, display.as_ref());
            node.show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
        data_type.is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a MapArray {
    type State = (Box<dyn ShowIndex + 'a>, Box<dyn ShowIndex + 'a>);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let keys = make_formatter(self.keys().as_ref(), options)?;
        let values = make_formatter(self.values().as_ref(), options)?;
        Ok((keys, values))
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        f.write_char('{')?;
        if let Some(idx) = iter.next() {
            s.0.write(idx, f)?;
            write!(f, ": ")?;
            s.1.write(idx, f)?;
        }

        for idx in iter {
            write!(f, ", ")?;
            s.0.write(idx, f)?;
            write!(f, ": ")?;
            s.1.write(idx, f)?;
        }

        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        for idx in iter {
            let mut key_string = String::new();
            state.0.write(idx, &mut key_string);

            ArrowNode::name(&key_string, state.1.as_ref()).show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
        data_type.is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a UnionArray {
    type State = (Vec<Option<(&'a str, Box<dyn ShowIndex + 'a>)>>, UnionMode);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let (fields, mode) = match (*self).data_type() {
            DataType::Union(fields, mode) => (fields, mode),
            _ => unreachable!(),
        };

        let max_id = fields.iter().map(|(id, _)| id).max().unwrap_or_default() as usize;
        let mut out: Vec<Option<FieldDisplay>> = (0..max_id + 1).map(|_| None).collect();
        for (i, field) in fields.iter() {
            let formatter = make_formatter(self.child(i).as_ref(), options)?;
            out[i as usize] = Some((field.name().as_str(), formatter))
        }
        Ok((out, *mode))
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let id = self.type_id(idx);
        let idx = match s.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = s.0[id as usize].as_ref().unwrap();

        write!(f, "{{{name}=")?;
        field.write(idx, f)?;
        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let id = self.type_id(idx);
        let idx = match state.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = state.0[id as usize].as_ref().unwrap();

        let node = ArrowNode::name(name, field.as_ref());
        node.show(ui, idx);
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
        data_type.is_nested()
    }
}

/// Get the value at the given row in an array as a String.
///
/// Note this function is quite inefficient and is unlikely to be
/// suitable for converting large arrays or record batches.
///
/// Please see [`ArrayUi`] for a more performant interface
pub fn array_value_to_string(column: &dyn Array, row: usize) -> Result<String, ArrowError> {
    let options = FormatOptions::default().with_display_error(true);
    let formatter = ArrayUi::try_new(column, &options)?;
    Ok(formatter.value(row).to_string())
}

/// Converts numeric type to a `String`
pub fn lexical_to_string<N: lexical_core::ToLexical>(n: N) -> String {
    let mut buf = Vec::<u8>::with_capacity(N::FORMATTED_SIZE_DECIMAL);
    unsafe {
        // JUSTIFICATION
        //  Benefit
        //      Allows using the faster serializer lexical core and convert to string
        //  Soundness
        //      Length of buf is set as written length afterwards. lexical_core
        //      creates a valid string, so doesn't need to be checked.
        let slice = std::slice::from_raw_parts_mut(buf.as_mut_ptr(), buf.capacity());
        let len = lexical_core::write(n, slice).len();
        buf.set_len(len);
        String::from_utf8_unchecked(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::builder::StringRunBuilder;

    /// Test to verify options can be constant. See #4580
    const TEST_CONST_OPTIONS: FormatOptions<'static> = FormatOptions::new()
        .with_date_format(Some("foo"))
        .with_timestamp_format(Some("404"));

    #[test]
    fn test_const_options() {
        assert_eq!(TEST_CONST_OPTIONS.date_format, Some("foo"));
    }

    #[test]
    fn test_map_array_to_string() {
        let keys = vec!["a", "b", "c", "d", "e", "f", "g", "h"];
        let values_data = UInt32Array::from(vec![0u32, 10, 20, 30, 40, 50, 60, 70]);

        // Construct a buffer for value offsets, for the nested array:
        //  [[a, b, c], [d, e, f], [g, h]]
        let entry_offsets = [0, 3, 6, 8];

        let map_array =
            MapArray::new_from_strings(keys.clone().into_iter(), &values_data, &entry_offsets)
                .unwrap();
        assert_eq!(
            "{d: 30, e: 40, f: 50}",
            array_value_to_string(&map_array, 1).unwrap()
        );
    }

    fn format_array(array: &dyn Array, fmt: &FormatOptions) -> Vec<String> {
        let fmt = ArrayUi::try_new(array, fmt).unwrap();
        (0..array.len()).map(|x| fmt.value(x).to_string()).collect()
    }

    #[test]
    fn test_array_value_to_string_duration() {
        let iso_fmt = FormatOptions::new();
        let pretty_fmt = FormatOptions::new().with_duration_format(DurationFormat::Pretty);

        let array = DurationNanosecondArray::from(vec![
            1,
            -1,
            1000,
            -1000,
            (45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000_000_000 + 123456789,
            -(45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000_000_000 - 123456789,
        ]);
        let iso = format_array(&array, &iso_fmt);
        let pretty = format_array(&array, &pretty_fmt);

        assert_eq!(iso[0], "PT0.000000001S");
        assert_eq!(pretty[0], "0 days 0 hours 0 mins 0.000000001 secs");
        assert_eq!(iso[1], "-PT0.000000001S");
        assert_eq!(pretty[1], "0 days 0 hours 0 mins -0.000000001 secs");
        assert_eq!(iso[2], "PT0.000001S");
        assert_eq!(pretty[2], "0 days 0 hours 0 mins 0.000001000 secs");
        assert_eq!(iso[3], "-PT0.000001S");
        assert_eq!(pretty[3], "0 days 0 hours 0 mins -0.000001000 secs");
        assert_eq!(iso[4], "PT3938554.123456789S");
        assert_eq!(pretty[4], "45 days 14 hours 2 mins 34.123456789 secs");
        assert_eq!(iso[5], "-PT3938554.123456789S");
        assert_eq!(pretty[5], "-45 days -14 hours -2 mins -34.123456789 secs");

        let array = DurationMicrosecondArray::from(vec![
            1,
            -1,
            1000,
            -1000,
            (45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000_000 + 123456,
            -(45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000_000 - 123456,
        ]);
        let iso = format_array(&array, &iso_fmt);
        let pretty = format_array(&array, &pretty_fmt);

        assert_eq!(iso[0], "PT0.000001S");
        assert_eq!(pretty[0], "0 days 0 hours 0 mins 0.000001 secs");
        assert_eq!(iso[1], "-PT0.000001S");
        assert_eq!(pretty[1], "0 days 0 hours 0 mins -0.000001 secs");
        assert_eq!(iso[2], "PT0.001S");
        assert_eq!(pretty[2], "0 days 0 hours 0 mins 0.001000 secs");
        assert_eq!(iso[3], "-PT0.001S");
        assert_eq!(pretty[3], "0 days 0 hours 0 mins -0.001000 secs");
        assert_eq!(iso[4], "PT3938554.123456S");
        assert_eq!(pretty[4], "45 days 14 hours 2 mins 34.123456 secs");
        assert_eq!(iso[5], "-PT3938554.123456S");
        assert_eq!(pretty[5], "-45 days -14 hours -2 mins -34.123456 secs");

        let array = DurationMillisecondArray::from(vec![
            1,
            -1,
            1000,
            -1000,
            (45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000 + 123,
            -(45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34) * 1_000 - 123,
        ]);
        let iso = format_array(&array, &iso_fmt);
        let pretty = format_array(&array, &pretty_fmt);

        assert_eq!(iso[0], "PT0.001S");
        assert_eq!(pretty[0], "0 days 0 hours 0 mins 0.001 secs");
        assert_eq!(iso[1], "-PT0.001S");
        assert_eq!(pretty[1], "0 days 0 hours 0 mins -0.001 secs");
        assert_eq!(iso[2], "PT1S");
        assert_eq!(pretty[2], "0 days 0 hours 0 mins 1.000 secs");
        assert_eq!(iso[3], "-PT1S");
        assert_eq!(pretty[3], "0 days 0 hours 0 mins -1.000 secs");
        assert_eq!(iso[4], "PT3938554.123S");
        assert_eq!(pretty[4], "45 days 14 hours 2 mins 34.123 secs");
        assert_eq!(iso[5], "-PT3938554.123S");
        assert_eq!(pretty[5], "-45 days -14 hours -2 mins -34.123 secs");

        let array = DurationSecondArray::from(vec![
            1,
            -1,
            1000,
            -1000,
            45 * 60 * 60 * 24 + 14 * 60 * 60 + 2 * 60 + 34,
            -45 * 60 * 60 * 24 - 14 * 60 * 60 - 2 * 60 - 34,
        ]);
        let iso = format_array(&array, &iso_fmt);
        let pretty = format_array(&array, &pretty_fmt);

        assert_eq!(iso[0], "PT1S");
        assert_eq!(pretty[0], "0 days 0 hours 0 mins 1 secs");
        assert_eq!(iso[1], "-PT1S");
        assert_eq!(pretty[1], "0 days 0 hours 0 mins -1 secs");
        assert_eq!(iso[2], "PT1000S");
        assert_eq!(pretty[2], "0 days 0 hours 16 mins 40 secs");
        assert_eq!(iso[3], "-PT1000S");
        assert_eq!(pretty[3], "0 days 0 hours -16 mins -40 secs");
        assert_eq!(iso[4], "PT3938554S");
        assert_eq!(pretty[4], "45 days 14 hours 2 mins 34 secs");
        assert_eq!(iso[5], "-PT3938554S");
        assert_eq!(pretty[5], "-45 days -14 hours -2 mins -34 secs");
    }

    #[test]
    fn test_null() {
        let array = NullArray::new(2);
        let options = FormatOptions::new().with_null("NULL");
        let formatted = format_array(&array, &options);
        assert_eq!(formatted, &["NULL".to_string(), "NULL".to_string()])
    }

    #[test]
    fn test_string_run_arry_to_string() {
        let mut builder = StringRunBuilder::<Int32Type>::new();

        builder.append_value("input_value");
        builder.append_value("input_value");
        builder.append_value("input_value");
        builder.append_value("input_value1");

        let map_array = builder.finish();
        assert_eq!("input_value", array_value_to_string(&map_array, 1).unwrap());
        assert_eq!(
            "input_value1",
            array_value_to_string(&map_array, 3).unwrap()
        );
    }
}
