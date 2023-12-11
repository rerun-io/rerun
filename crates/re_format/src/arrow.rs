//! Formatting for tables of Arrow arrays

use std::fmt::Formatter;

use arrow2::{
    array::{get_display, Array, ListArray},
    datatypes::{DataType, IntervalUnit, TimeUnit},
};
use comfy_table::{presets, Cell, Table};

use re_tuid::Tuid;
use re_types_core::Loggable as _;

// ---

// TODO(#1775): Registering custom formatters should be done from other crates:
// A) Because `re_format` cannot depend on other crates (cyclic deps)
// B) Because how to deserialize and inspect some type is a private implementation detail of that
//    type, re_format shouldn't know how to deserialize a TUID…

type CustomFormatter<'a, F> = Box<dyn Fn(&mut F, usize) -> std::fmt::Result + 'a>;

fn get_custom_display<'a, F: std::fmt::Write + 'a>(
    _column_name: &'a str,
    array: &'a dyn Array,
    null: &'static str,
) -> CustomFormatter<'a, F> {
    // NOTE: If the top-level array is a list, it's probably not the type we're looking for: we're
    // interested in the type of the array that's underneath.
    let datatype = (|| match array.data_type().to_logical_type() {
        DataType::List(_) => array
            .as_any()
            .downcast_ref::<ListArray<i32>>()?
            .iter()
            .next()?
            .map(|array| array.data_type().clone()),
        _ => Some(array.data_type().clone()),
    })();

    if let Some(DataType::Extension(name, _, _)) = datatype {
        // TODO(#1775): This should be registered dynamically.
        if name.as_str() == Tuid::name() {
            return Box::new(|w, index| {
                if let Some(tuid) = parse_tuid(array, index) {
                    w.write_fmt(format_args!("{tuid}"))
                } else {
                    w.write_str("<ERR>")
                }
            });
        }
    }

    get_display(array, null)
}

// TODO(#1775): This should be defined and registered by the `re_tuid` crate.
fn parse_tuid(array: &dyn Array, index: usize) -> Option<Tuid> {
    let (array, index) = match array.data_type().to_logical_type() {
        // Legacy MsgId lists: just grab the first value, they're all identical
        DataType::List(_) => (
            array
                .as_any()
                .downcast_ref::<ListArray<i32>>()?
                .value(index),
            0,
        ),
        // New control columns: it's not a list to begin with!
        _ => (array.to_boxed(), index),
    };

    let tuids = Tuid::from_arrow(array.as_ref()).ok()?;
    tuids.get(index).copied()
}

// ---

//TODO(john) move this and the Display impl upstream into arrow2
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

//TODO(john) move this and the Display impl upstream into arrow2
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

//TODO(john) move this and the Display impl upstream into arrow2
#[repr(transparent)]
struct DisplayDataType(DataType);

impl std::fmt::Display for DisplayDataType {
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
                    format!("timestamp({}, {tz})", DisplayTimeUnit(*unit))
                } else {
                    format!("timestamp({})", DisplayTimeUnit(*unit))
                };
                return f.write_str(&s);
            }
            DataType::Date32 => "date32",
            DataType::Date64 => "date64",
            DataType::Time32(unit) => {
                let s = format!("time32({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Time64(unit) => {
                let s = format!("time64({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Duration(unit) => {
                let s = format!("duration({})", DisplayTimeUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Interval(unit) => {
                let s = format!("interval({})", DisplayIntervalUnit(*unit));
                return f.write_str(&s);
            }
            DataType::Binary => "bin",
            DataType::FixedSizeBinary(size) => return write!(f, "fixed-bin[{size}]"),
            DataType::LargeBinary => "large-bin",
            DataType::Utf8 => "str",
            DataType::LargeUtf8 => "large-string",
            DataType::List(ref field) => {
                let s = format!("list[{}]", DisplayDataType(field.data_type().clone()));
                return f.write_str(&s);
            }
            DataType::FixedSizeList(field, len) => {
                let s = format!(
                    "fixed-list[{}; {len}]",
                    DisplayDataType(field.data_type().clone())
                );
                return f.write_str(&s);
            }
            DataType::LargeList(field) => {
                let s = format!("large-list[{}]", DisplayDataType(field.data_type().clone()));
                return f.write_str(&s);
            }
            DataType::Struct(fields) => return write!(f, "struct[{}]", fields.len()),
            DataType::Union(fields, _, _) => return write!(f, "union[{}]", fields.len()),
            DataType::Map(field, _) => {
                return write!(f, "map[{}]", DisplayDataType(field.data_type().clone()))
            }
            DataType::Dictionary(_, _, _) => "dict",
            DataType::Decimal(_, _) => "decimal",
            DataType::Decimal256(_, _) => "decimal256",
            DataType::Extension(name, data_type, _) => {
                let s = format!("extension<{name}>[{}]", DisplayDataType(*data_type.clone()));
                return f.write_str(&s);
            }
        };
        f.write_str(s)
    }
}

/// Format `columns` into a [`Table`] using `names` as headers.
pub fn format_table<A, Ia, N, In>(columns: Ia, names: In) -> Table
where
    A: AsRef<dyn Array>,
    Ia: IntoIterator<Item = A>,
    N: AsRef<str>,
    In: IntoIterator<Item = N>,
{
    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);

    const WIDTH_UPPER_BOUNDARY: u16 = 100;

    let names = names
        .into_iter()
        .map(|name| name.as_ref().to_owned())
        .collect::<Vec<_>>();
    let arrays = columns.into_iter().collect::<Vec<_>>();

    let (displayers, lengths): (Vec<_>, Vec<_>) = arrays
        .iter()
        .zip(names.iter())
        .map(|(array, name)| {
            let formatter = get_custom_display(name, array.as_ref(), "-");
            (formatter, array.as_ref().len())
        })
        .unzip();

    if displayers.is_empty() {
        return table;
    }

    let header = names
        .iter()
        .zip(arrays.iter().map(|array| array.as_ref().data_type()))
        .map(|(name, data_type)| {
            Cell::new(format!(
                "{}\n---\n{}",
                name.trim_start_matches("rerun.archetypes.")
                    .trim_start_matches("rerun.components.")
                    .trim_start_matches("rerun.datatypes.")
                    .trim_start_matches("rerun.controls.")
                    .trim_start_matches("rerun."),
                DisplayDataType(data_type.clone())
            ))
        });
    table.set_header(header);

    for row in 0..lengths[0] {
        let cells: Vec<_> = displayers
            .iter()
            .map(|disp| {
                let mut string = String::new();
                (disp)(&mut string, row).unwrap();
                let chars: Vec<_> = string.chars().collect();
                if chars.len() > WIDTH_UPPER_BOUNDARY as usize {
                    Cell::new(
                        chars
                            .into_iter()
                            .take(WIDTH_UPPER_BOUNDARY.saturating_sub(1).into())
                            .chain(['…'])
                            .collect::<String>(),
                    )
                } else {
                    Cell::new(string)
                }
            })
            .collect();
        table.add_row(cells);
    }

    table.set_content_arrangement(comfy_table::ContentArrangement::DynamicFullWidth);
    // NOTE: `Percentage` only works for terminals that report their sizes.
    let width = if table.width().is_some() {
        comfy_table::Width::Percentage((100.0 / arrays.len() as f32) as u16)
    } else {
        comfy_table::Width::Fixed(WIDTH_UPPER_BOUNDARY)
    };
    table.set_constraints(
        std::iter::repeat(comfy_table::ColumnConstraint::UpperBoundary(width)).take(arrays.len()),
    );

    table
}
