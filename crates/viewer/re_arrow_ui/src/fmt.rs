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
use chrono::{NaiveDate, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use egui::Ui;
use lexical_core::FormattedSize;
use re_ui::UiExt;
use re_ui::list_item::PropertyContent;
use std::fmt::{Display, Formatter, Write};
use std::ops::Range;

type TimeFormat<'a> = Option<&'a str>;

/// Format for displaying durations
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DurationFormat {
    /// ISO 8601 - `P198DT72932.972880S`
    ISO8601,
    /// A human readable representation - `198 days 16 hours 34 mins 15.407810000 secs`
    Pretty,
}

/// Options for formatting arrays
///
/// By default nulls are formatted as `""` and temporal types formatted
/// according to RFC3339
///
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormatOptions<'a> {
    /// If set to `true` any formatting errors will be written to the output
    /// instead of being converted into a [`std::fmt::Error`]
    safe: bool,
    /// Format string for nulls
    null: &'a str,
    /// Date format for date arrays
    date_format: TimeFormat<'a>,
    /// Format for DateTime arrays
    datetime_format: TimeFormat<'a>,
    /// Timestamp format for timestamp arrays
    timestamp_format: TimeFormat<'a>,
    /// Timestamp format for timestamp with timezone arrays
    timestamp_tz_format: TimeFormat<'a>,
    /// Time format for time arrays
    time_format: TimeFormat<'a>,
    /// Duration format
    duration_format: DurationFormat,
    /// Show types in visual representation batches
    types_info: bool,
}

impl Default for FormatOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> FormatOptions<'a> {
    /// Creates a new set of format options
    pub const fn new() -> Self {
        Self {
            safe: true,
            null: "",
            date_format: None,
            datetime_format: None,
            timestamp_format: None,
            timestamp_tz_format: None,
            time_format: None,
            duration_format: DurationFormat::ISO8601,
            types_info: false,
        }
    }

    /// If set to `true` any formatting errors will be written to the output
    /// instead of being converted into a [`std::fmt::Error`]
    pub const fn with_display_error(mut self, safe: bool) -> Self {
        self.safe = safe;
        self
    }

    /// Overrides the string used to represent a null
    ///
    /// Defaults to `""`
    pub const fn with_null(self, null: &'a str) -> Self {
        Self { null, ..self }
    }

    /// Overrides the format used for [`DataType::Date32`] columns
    pub const fn with_date_format(self, date_format: Option<&'a str>) -> Self {
        Self {
            date_format,
            ..self
        }
    }

    /// Overrides the format used for [`DataType::Date64`] columns
    pub const fn with_datetime_format(self, datetime_format: Option<&'a str>) -> Self {
        Self {
            datetime_format,
            ..self
        }
    }

    /// Overrides the format used for [`DataType::Timestamp`] columns without a timezone
    pub const fn with_timestamp_format(self, timestamp_format: Option<&'a str>) -> Self {
        Self {
            timestamp_format,
            ..self
        }
    }

    /// Overrides the format used for [`DataType::Timestamp`] columns with a timezone
    pub const fn with_timestamp_tz_format(self, timestamp_tz_format: Option<&'a str>) -> Self {
        Self {
            timestamp_tz_format,
            ..self
        }
    }

    /// Overrides the format used for [`DataType::Time32`] and [`DataType::Time64`] columns
    pub const fn with_time_format(self, time_format: Option<&'a str>) -> Self {
        Self {
            time_format,
            ..self
        }
    }

    /// Overrides the format used for duration columns
    ///
    /// Defaults to [`DurationFormat::ISO8601`]
    pub const fn with_duration_format(self, duration_format: DurationFormat) -> Self {
        Self {
            duration_format,
            ..self
        }
    }

    /// Overrides if types should be shown
    ///
    /// Defaults to [`false`]
    pub const fn with_types_info(self, types_info: bool) -> Self {
        Self { types_info, ..self }
    }

    /// Returns true if type info should be included in visual representation of batches
    pub const fn types_info(&self) -> bool {
        self.types_info
    }
}

/// Implements [`Display`] for a specific array value
pub struct ValueFormatter<'a> {
    idx: usize,
    formatter: &'a ArrayFormatter<'a>,
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
            Err(FormatError::Arrow(e)) if self.formatter.safe => {
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
pub struct ArrayFormatter<'a> {
    format: Box<dyn DisplayIndex + 'a>,
    safe: bool,
}

impl<'a> ArrayFormatter<'a> {
    /// Returns an [`ArrayFormatter`] that can be used to format `array`
    ///
    /// This returns an error if an array of the given data type cannot be formatted
    pub fn try_new(array: &'a dyn Array, options: &FormatOptions<'a>) -> Result<Self, ArrowError> {
        Ok(Self {
            format: make_formatter(array, options)?,
            safe: options.safe,
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
) -> Result<Box<dyn DisplayIndex + 'a>, ArrowError> {
    downcast_primitive_array! {
        array => array_format(array, options),
        DataType::Null => array_format(as_null_array(array), options),
        DataType::Boolean => array_format(as_boolean_array(array), options),
        DataType::Utf8 => array_format(array.as_string::<i32>(), options),
        DataType::LargeUtf8 => array_format(array.as_string::<i64>(), options),
        DataType::Utf8View => array_format(array.as_string_view(), options),
        DataType::Binary => array_format(array.as_binary::<i32>(), options),
        DataType::BinaryView => array_format(array.as_binary_view(), options),
        DataType::LargeBinary => array_format(array.as_binary::<i64>(), options),
        DataType::FixedSizeBinary(_) => {
            let a = array.as_any().downcast_ref::<FixedSizeBinaryArray>().unwrap();
            array_format(a, options)
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
trait DisplayIndex {
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

/// [`DisplayIndex`] with additional state
trait DisplayIndexState<'a> {
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

impl<'a, T: DisplayIndex> DisplayIndexState<'a> for T {
    type State = ();

    fn prepare(&self, _options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok(())
    }

    fn write(&self, _: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        DisplayIndex::write(self, idx, f)
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        DisplayIndex::show(self, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        DisplayIndex::is_item_nested(self)
    }
}

struct ArrayFormat<'a, F: DisplayIndexState<'a>> {
    state: F::State,
    array: F,
    null: &'a str,
}

fn array_format<'a, F>(
    array: F,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn DisplayIndex + 'a>, ArrowError>
where
    F: DisplayIndexState<'a> + Array + 'a,
{
    let state = array.prepare(options)?;
    Ok(Box::new(ArrayFormat {
        state,
        array,
        null: options.null,
    }))
}

impl<'a, F: DisplayIndexState<'a> + Array> DisplayIndex for ArrayFormat<'a, F> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        if self.array.is_null(idx) {
            if !self.null.is_empty() {
                f.write_str(self.null)?
            }
            return Ok(());
        }
        DisplayIndexState::write(&self.array, &self.state, idx, f)
    }

    fn show(&self, idx: usize, ui: &mut Ui) {
        DisplayIndexState::show(&self.array, &self.state, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        DisplayIndexState::is_item_nested(&self.array)
    }
}

impl DisplayIndex for &BooleanArray {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

impl<'a> DisplayIndexState<'a> for &'a NullArray {
    type State = &'a str;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok(options.null)
    }

    fn write(&self, state: &Self::State, _idx: usize, f: &mut dyn Write) -> FormatResult {
        f.write_str(state)?;
        Ok(())
    }
}

macro_rules! primitive_display {
    ($($t:ty),+) => {
        $(impl<'a> DisplayIndex for &'a PrimitiveArray<$t>
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
        $(impl<'a> DisplayIndex for &'a PrimitiveArray<$t>
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

impl DisplayIndex for &PrimitiveArray<Float16Type> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

macro_rules! decimal_display {
    ($($t:ty),+) => {
        $(impl<'a> DisplayIndexState<'a> for &'a PrimitiveArray<$t> {
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

fn write_timestamp(
    f: &mut dyn Write,
    naive: NaiveDateTime,
    timezone: Option<Tz>,
    format: Option<&str>,
) -> FormatResult {
    match timezone {
        Some(tz) => {
            let date = Utc.from_utc_datetime(&naive).with_timezone(&tz);
            match format {
                Some(s) => write!(f, "{}", date.format(s))?,
                None => write!(f, "{}", date.to_rfc3339_opts(SecondsFormat::AutoSi, true))?,
            }
        }
        None => match format {
            Some(s) => write!(f, "{}", naive.format(s))?,
            None => write!(f, "{naive:?}")?,
        },
    }
    Ok(())
}

macro_rules! timestamp_display {
    ($($t:ty),+) => {
        $(impl<'a> DisplayIndexState<'a> for &'a PrimitiveArray<$t> {
            type State = (Option<Tz>, TimeFormat<'a>);

            fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
                match self.data_type() {
                    DataType::Timestamp(_, Some(tz)) => Ok((Some(tz.parse()?), options.timestamp_tz_format)),
                    DataType::Timestamp(_, None) => Ok((None, options.timestamp_format)),
                    _ => unreachable!(),
                }
            }

            fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
                let value = self.value(idx);
                let naive = as_datetime::<$t>(value).ok_or_else(|| {
                    ArrowError::CastError(format!(
                        "Failed to convert {} to datetime for {}",
                        value,
                        self.data_type()
                    ))
                })?;

                write_timestamp(f, naive, s.0, s.1.clone())
            }
        })+
    };
}

timestamp_display!(
    TimestampSecondType,
    TimestampMillisecondType,
    TimestampMicrosecondType,
    TimestampNanosecondType
);

macro_rules! temporal_display {
    ($convert:ident, $format:ident, $t:ty) => {
        impl<'a> DisplayIndexState<'a> for &'a PrimitiveArray<$t> {
            type State = TimeFormat<'a>;

            fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
                Ok(options.$format)
            }

            fn write(&self, fmt: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
                let value = self.value(idx);
                let naive = $convert(value as _).ok_or_else(|| {
                    ArrowError::CastError(format!(
                        "Failed to convert {} to temporal for {}",
                        value,
                        self.data_type()
                    ))
                })?;

                match fmt {
                    Some(s) => write!(f, "{}", naive.format(s))?,
                    None => write!(f, "{naive:?}")?,
                }
                Ok(())
            }
        }
    };
}

#[inline]
fn date32_to_date(value: i32) -> Option<NaiveDate> {
    Some(date32_to_datetime(value)?.date())
}

temporal_display!(date32_to_date, date_format, Date32Type);
temporal_display!(date64_to_datetime, datetime_format, Date64Type);
temporal_display!(time32s_to_time, time_format, Time32SecondType);
temporal_display!(time32ms_to_time, time_format, Time32MillisecondType);
temporal_display!(time64us_to_time, time_format, Time64MicrosecondType);
temporal_display!(time64ns_to_time, time_format, Time64NanosecondType);

/// Derive [`DisplayIndexState`] for `PrimitiveArray<$t>`
///
/// Arguments
/// * `$convert` - function to convert the value to an `Duration`
/// * `$t` - [`ArrowPrimitiveType`] of the array
/// * `$scale` - scale of the duration (passed to `duration_fmt`)
macro_rules! duration_display {
    ($convert:ident, $t:ty, $scale:tt) => {
        impl<'a> DisplayIndexState<'a> for &'a PrimitiveArray<$t> {
            type State = DurationFormat;

            fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
                Ok(options.duration_format)
            }

            fn write(&self, fmt: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
                let v = self.value(idx);
                match fmt {
                    DurationFormat::ISO8601 => write!(f, "{}", $convert(v))?,
                    DurationFormat::Pretty => duration_fmt!(f, v, $scale)?,
                }
                Ok(())
            }
        }
    };
}

/// Similar to [`duration_display`] but `$convert` returns an `Option`
macro_rules! duration_option_display {
    ($convert:ident, $t:ty, $scale:tt) => {
        impl<'a> DisplayIndexState<'a> for &'a PrimitiveArray<$t> {
            type State = DurationFormat;

            fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
                Ok(options.duration_format)
            }

            fn write(&self, fmt: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
                let v = self.value(idx);
                match fmt {
                    DurationFormat::ISO8601 => match $convert(v) {
                        Some(td) => write!(f, "{}", td)?,
                        None => write!(f, "<invalid>")?,
                    },
                    DurationFormat::Pretty => match $convert(v) {
                        Some(_) => duration_fmt!(f, v, $scale)?,
                        None => write!(f, "<invalid>")?,
                    },
                }
                Ok(())
            }
        }
    };
}

macro_rules! duration_fmt {
    ($f:ident, $v:expr, 0) => {{
        let secs = $v;
        let mins = secs / 60;
        let hours = mins / 60;
        let days = hours / 24;

        let secs = secs - (mins * 60);
        let mins = mins - (hours * 60);
        let hours = hours - (days * 24);
        write!($f, "{days} days {hours} hours {mins} mins {secs} secs")
    }};
    ($f:ident, $v:expr, $scale:tt) => {{
        let subsec = $v;
        let secs = subsec / 10_i64.pow($scale);
        let mins = secs / 60;
        let hours = mins / 60;
        let days = hours / 24;

        let subsec = subsec - (secs * 10_i64.pow($scale));
        let secs = secs - (mins * 60);
        let mins = mins - (hours * 60);
        let hours = hours - (days * 24);
        match subsec.is_negative() {
            true => {
                write!(
                    $f,
                    concat!("{} days {} hours {} mins -{}.{:0", $scale, "} secs"),
                    days,
                    hours,
                    mins,
                    secs.abs(),
                    subsec.abs()
                )
            }
            false => {
                write!(
                    $f,
                    concat!("{} days {} hours {} mins {}.{:0", $scale, "} secs"),
                    days, hours, mins, secs, subsec
                )
            }
        }
    }};
}

duration_option_display!(try_duration_s_to_duration, DurationSecondType, 0);
duration_option_display!(try_duration_ms_to_duration, DurationMillisecondType, 3);
duration_display!(duration_us_to_duration, DurationMicrosecondType, 6);
duration_display!(duration_ns_to_duration, DurationNanosecondType, 9);

impl DisplayIndex for &PrimitiveArray<IntervalYearMonthType> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let interval = self.value(idx) as f64;
        let years = (interval / 12_f64).floor();
        let month = interval - (years * 12_f64);

        write!(f, "{years} years {month} mons",)?;
        Ok(())
    }
}

impl DisplayIndex for &PrimitiveArray<IntervalDayTimeType> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value = self.value(idx);
        let mut prefix = "";

        if value.days != 0 {
            write!(f, "{prefix}{} days", value.days)?;
            prefix = " ";
        }

        if value.milliseconds != 0 {
            let millis_fmt = MillisecondsFormatter {
                milliseconds: value.milliseconds,
                prefix,
            };

            f.write_fmt(format_args!("{millis_fmt}"))?;
        }

        Ok(())
    }
}

impl DisplayIndex for &PrimitiveArray<IntervalMonthDayNanoType> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value = self.value(idx);
        let mut prefix = "";

        if value.months != 0 {
            write!(f, "{prefix}{} mons", value.months)?;
            prefix = " ";
        }

        if value.days != 0 {
            write!(f, "{prefix}{} days", value.days)?;
            prefix = " ";
        }

        if value.nanoseconds != 0 {
            let nano_fmt = NanosecondsFormatter {
                nanoseconds: value.nanoseconds,
                prefix,
            };
            f.write_fmt(format_args!("{nano_fmt}"))?;
        }

        Ok(())
    }
}

struct NanosecondsFormatter<'a> {
    nanoseconds: i64,
    prefix: &'a str,
}

impl Display for NanosecondsFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut prefix = self.prefix;

        let secs = self.nanoseconds / 1_000_000_000;
        let mins = secs / 60;
        let hours = mins / 60;

        let secs = secs - (mins * 60);
        let mins = mins - (hours * 60);

        let nanoseconds = self.nanoseconds % 1_000_000_000;

        if hours != 0 {
            write!(f, "{prefix}{} hours", hours)?;
            prefix = " ";
        }

        if mins != 0 {
            write!(f, "{prefix}{} mins", mins)?;
            prefix = " ";
        }

        if secs != 0 || nanoseconds != 0 {
            let secs_sign = if secs < 0 || nanoseconds < 0 { "-" } else { "" };
            write!(
                f,
                "{prefix}{}{}.{:09} secs",
                secs_sign,
                secs.abs(),
                nanoseconds.abs()
            )?;
        }

        Ok(())
    }
}

struct MillisecondsFormatter<'a> {
    milliseconds: i32,
    prefix: &'a str,
}

impl Display for MillisecondsFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut prefix = self.prefix;

        let secs = self.milliseconds / 1_000;
        let mins = secs / 60;
        let hours = mins / 60;

        let secs = secs - (mins * 60);
        let mins = mins - (hours * 60);

        let milliseconds = self.milliseconds % 1_000;

        if hours != 0 {
            write!(f, "{prefix}{} hours", hours,)?;
            prefix = " ";
        }

        if mins != 0 {
            write!(f, "{prefix}{} mins", mins,)?;
            prefix = " ";
        }

        if secs != 0 || milliseconds != 0 {
            let secs_sign = if secs < 0 || milliseconds < 0 {
                "-"
            } else {
                ""
            };

            write!(
                f,
                "{prefix}{}{}.{:03} secs",
                secs_sign,
                secs.abs(),
                milliseconds.abs()
            )?;
        }

        Ok(())
    }
}

impl<O: OffsetSizeTrait> DisplayIndex for &GenericStringArray<O> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

impl DisplayIndex for &StringViewArray {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        write!(f, "{}", self.value(idx))?;
        Ok(())
    }
}

impl<O: OffsetSizeTrait> DisplayIndex for &GenericBinaryArray<O> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let v = self.value(idx);
        for byte in v {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl DisplayIndex for &BinaryViewArray {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let v = self.value(idx);
        for byte in v {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl DisplayIndex for &FixedSizeBinaryArray {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        let v = self.value(idx);
        for byte in v {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<'a, K: ArrowDictionaryKeyType> DisplayIndexState<'a> for &'a DictionaryArray<K> {
    type State = Box<dyn DisplayIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_formatter(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.keys().values()[idx].as_usize();
        s.as_ref().write(value_idx, f)
    }
}

impl<'a, K: RunEndIndexType> DisplayIndexState<'a> for &'a RunArray<K> {
    type State = Box<dyn DisplayIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_formatter(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.get_physical_index(idx);
        s.as_ref().write(value_idx, f)
    }
}

fn write_list(
    f: &mut dyn Write,
    mut range: Range<usize>,
    values: &dyn DisplayIndex,
) -> FormatResult {
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
    values: &'a dyn DisplayIndex,
}

impl<'a> ArrowNode<'a> {
    pub fn name(name: &str, values: &'a dyn DisplayIndex) -> Self {
        Self {
            label: NodeLabel::Name(name.to_string()),
            values,
        }
    }

    pub fn index(idx: usize, values: &'a dyn DisplayIndex) -> Self {
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

fn list_ui(ui: &mut Ui, mut range: Range<usize>, values: &dyn DisplayIndex) {
    let mut label_index = 0;
    for idx in range {
        let node = ArrowNode::index(label_index, values);
        node.show(ui, idx);
        label_index += 1;
    }
}

impl<'a, O: OffsetSizeTrait> DisplayIndexState<'a> for &'a GenericListArray<O> {
    type State = Box<dyn DisplayIndex + 'a>;

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

impl<'a> DisplayIndexState<'a> for &'a FixedSizeListArray {
    type State = (usize, Box<dyn DisplayIndex + 'a>);

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

/// Pairs a boxed [`DisplayIndex`] with its field name
type FieldDisplay<'a> = (&'a str, Box<dyn DisplayIndex + 'a>);

impl<'a> DisplayIndexState<'a> for &'a StructArray {
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

impl<'a> DisplayIndexState<'a> for &'a MapArray {
    type State = (Box<dyn DisplayIndex + 'a>, Box<dyn DisplayIndex + 'a>);

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

            ArrowNode::name(&key_string, state.0.as_ref()).show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
        data_type.is_nested()
    }
}

impl<'a> DisplayIndexState<'a> for &'a UnionArray {
    type State = (
        Vec<Option<(&'a str, Box<dyn DisplayIndex + 'a>)>>,
        UnionMode,
    );

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
/// Please see [`ArrayFormatter`] for a more performant interface
pub fn array_value_to_string(column: &dyn Array, row: usize) -> Result<String, ArrowError> {
    let options = FormatOptions::default().with_display_error(true);
    let formatter = ArrayFormatter::try_new(column, &options)?;
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
        let fmt = ArrayFormatter::try_new(array, fmt).unwrap();
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
