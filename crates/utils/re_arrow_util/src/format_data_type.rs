//! `arrow` has `ToString` implemented, but it is way too verbose.

use std::fmt::Formatter;

use arrow::datatypes::{DataType, IntervalUnit, TimeUnit};

/// Compact format of an arrow data type.
pub fn format_data_type(data_type: &DataType) -> String {
    DisplayDatatype(data_type).to_string()
}

#[repr(transparent)]
struct DisplayTimeUnit(TimeUnit);

impl std::fmt::Display for DisplayTimeUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            TimeUnit::Second => "s",
            TimeUnit::Millisecond => "ms",
            TimeUnit::Microsecond => "us",
            TimeUnit::Nanosecond => "ns",
        };
        f.write_str(s)
    }
}

// arrow has `ToString` implemented, but it is way too verbose.
#[repr(transparent)]
struct DisplayIntervalUnit(IntervalUnit);

impl std::fmt::Display for DisplayIntervalUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            IntervalUnit::YearMonth => "year/month",
            IntervalUnit::DayTime => "day/time",
            IntervalUnit::MonthDayNano => "month/day/nano",
        };
        f.write_str(s)
    }
}

// arrow has `ToString` implemented, but it is way too verbose.
#[repr(transparent)]
struct DisplayDatatype<'a>(&'a DataType);

impl std::fmt::Display for DisplayDatatype<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match &self.0 {
            DataType::Null => "null",
            DataType::Boolean => "bool",
            DataType::Int8 => "i8",
            DataType::Int16 => "i16",
            DataType::Int32 => "i32",
            DataType::Int64 => "i64",
            DataType::UInt8 => "u8",
            DataType::UInt16 => "u16",
            DataType::UInt32 => "u32",
            DataType::UInt64 => "u64",
            DataType::Float16 => "f16",
            DataType::Float32 => "f32",
            DataType::Float64 => "f64",
            DataType::Timestamp(unit, timezone) => {
                let s = if let Some(tz) = timezone {
                    format!("Timestamp({}, {tz})", DisplayTimeUnit(*unit))
                } else {
                    format!("Timestamp({})", DisplayTimeUnit(*unit))
                };
                return f.write_str(&s);
            }
            DataType::Date32 => "Date32",
            DataType::Date64 => "Date64",
            DataType::Time32(unit) => {
                let s = format!("Time32({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Time64(unit) => {
                let s = format!("Time64({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Duration(unit) => {
                let s = format!("Duration({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Interval(unit) => {
                let s = format!("Interval({})", DisplayIntervalUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Binary => "Binary",
            DataType::FixedSizeBinary(size) => return write!(f, "FixedSizeBinary[{size}]"),
            DataType::LargeBinary => "LargeBinary",
            DataType::Utf8 => "Utf8",
            DataType::LargeUtf8 => "LargeUtf8",
            DataType::List(field) => {
                let s = format!("List[{}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::FixedSizeList(field, len) => {
                let s = format!("FixedSizeList[{}; {len}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::LargeList(field) => {
                let s = format!("LargeList[{}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::Struct(fields) => return write!(f, "Struct[{}]", fields.len()),
            DataType::Union(fields, _) => return write!(f, "Union[{}]", fields.len()),
            DataType::Map(field, _) => return write!(f, "Map[{}]", Self(field.data_type())),
            DataType::Dictionary(key, value) => {
                return write!(f, "Dictionary{{{}: {}}}", Self(key), Self(value));
            }
            DataType::Decimal128(_, _) => "Decimal128",
            DataType::Decimal256(_, _) => "Decimal256",
            DataType::BinaryView => "BinaryView",
            DataType::Utf8View => "Utf8View",
            DataType::ListView(field) => return write!(f, "ListView[{}]", Self(field.data_type())),
            DataType::LargeListView(field) => {
                return write!(f, "LargeListView[{}]", Self(field.data_type()));
            }
            DataType::RunEndEncoded(_run_ends, values) => {
                return write!(f, "RunEndEncoded[{}]", Self(values.data_type()));
            }
        };
        f.write_str(s)
    }
}
