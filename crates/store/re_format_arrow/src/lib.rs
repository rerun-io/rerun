//! Formatting for tables of Arrow arrays

use std::fmt::Formatter;

use arrow::{
    array::{Array, ArrayRef, ListArray},
    datatypes::{DataType, Field, Fields, IntervalUnit, TimeUnit},
    util::display::{ArrayFormatter, FormatOptions},
};
use comfy_table::{presets, Cell, Row, Table};

use re_tuid::Tuid;
use re_types_core::Loggable as _;

// ---

// TODO(#1775): Registering custom formatters should be done from other crates:
// A) Because `re_format` cannot depend on other crates (cyclic deps)
// B) Because how to deserialize and inspect some type is a private implementation detail of that
//    type, re_format shouldn't know how to deserialize a TUID…

/// Format the given row as a string
type CustomArrayFormatter<'a> = Box<dyn Fn(usize) -> Result<String, String> + 'a>;

/// This is a `BTreeMap`, and not a `HashMap`, because we want a predictable order.
type Metadata = std::collections::BTreeMap<String, String>;

fn custom_array_formatter<'a>(field: &Field, array: &'a dyn Array) -> CustomArrayFormatter<'a> {
    if let Some(extension_name) = field.metadata().get("ARROW:extension:name") {
        // TODO(#1775): This should be registered dynamically.
        if extension_name.as_str() == Tuid::ARROW_EXTENSION_NAME {
            return Box::new(|index| {
                if let Some(tuid) = parse_tuid(array, index) {
                    Ok(format!("{tuid}"))
                } else {
                    Err("Invalid RowId".to_owned())
                }
            });
        }
    }

    match ArrayFormatter::try_new(array, &FormatOptions::default()) {
        Ok(formatter) => Box::new(move |index| Ok(format!("{}", formatter.value(index)))),
        Err(err) => Box::new(move |_| Err(format!("Failed to format array: {err}"))),
    }
}

// TODO(#1775): This should be defined and registered by the `re_tuid` crate.
fn parse_tuid(array: &dyn Array, index: usize) -> Option<Tuid> {
    fn parse_inner(array: &dyn Array, index: usize) -> Option<Tuid> {
        let tuids = Tuid::from_arrow(array).ok()?;
        tuids.get(index).copied()
    }

    match array.data_type() {
        // Legacy MsgId lists: just grab the first value, they're all identical
        DataType::List(_) => {
            parse_inner(&array.as_any().downcast_ref::<ListArray>()?.value(index), 0)
        }
        // New control columns: it's not a list to begin with!
        _ => parse_inner(array, index),
    }
}

// ---

//TODO(john) move this and the Display impl upstream into arrow
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

//TODO(john) move this and the Display impl upstream into arrow
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

//TODO(john) move this and the Display impl upstream into arrow
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
                let s = format!("list[{}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::FixedSizeList(field, len) => {
                let s = format!("fixed-list[{}; {len}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::LargeList(field) => {
                let s = format!("large-list[{}]", Self(field.data_type()));
                return f.write_str(&s);
            }
            DataType::Struct(fields) => return write!(f, "struct[{}]", fields.len()),
            DataType::Union(fields, _) => return write!(f, "union[{}]", fields.len()),
            DataType::Map(field, _) => return write!(f, "map[{}]", Self(field.data_type())),
            DataType::Dictionary(_, _) => "dict",
            DataType::Decimal128(_, _) => "decimal128",
            DataType::Decimal256(_, _) => "decimal256",
            DataType::BinaryView => todo!(),
            DataType::Utf8View => todo!(),
            DataType::ListView(field) => todo!(),
            DataType::LargeListView(field) => todo!(),
            DataType::RunEndEncoded(field, field1) => todo!(),
        };
        f.write_str(s)
    }
}

struct DisplayMetadata {
    prefix: &'static str,
    metadata: Metadata,
}

impl std::fmt::Display for DisplayMetadata {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { prefix, metadata } = self;
        f.write_str(
            &metadata
                .iter()
                .map(|(key, value)| format!("{prefix}{}: {:?}", trim_name(key), trim_name(value)))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

fn trim_name(name: &str) -> &str {
    name.trim_start_matches("rerun.archetypes.")
        .trim_start_matches("rerun.components.")
        .trim_start_matches("rerun.datatypes.")
        .trim_start_matches("rerun.controls.")
        .trim_start_matches("rerun.blueprint.archetypes.")
        .trim_start_matches("rerun.blueprint.components.")
        .trim_start_matches("rerun.blueprint.datatypes.")
        .trim_start_matches("rerun.field.")
        .trim_start_matches("rerun.chunk.")
        .trim_start_matches("rerun.")
}

pub fn format_dataframe(metadata: &Metadata, fields: &Fields, columns: &[ArrayRef]) -> Table {
    const MAXIMUM_CELL_CONTENT_WIDTH: u16 = 100;

    let mut outer_table = Table::new();
    outer_table.load_preset(presets::UTF8_FULL);

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);

    outer_table.add_row({
        let mut row = Row::new();
        row.add_cell(Cell::new(format!(
            "CHUNK METADATA:\n{}",
            DisplayMetadata {
                prefix: "* ",
                metadata: metadata.clone()
            }
        )));
        row
    });

    let header = fields.iter().map(|field| {
        if field.metadata().is_empty() {
            Cell::new(format!(
                "{}\n---\ntype: \"{}\"", // NOLINT
                trim_name(field.name()),
                DisplayDatatype(field.data_type()),
            ))
        } else {
            Cell::new(format!(
                "{}\n---\ntype: \"{}\"\n{}", // NOLINT
                trim_name(field.name()),
                DisplayDatatype(field.data_type()),
                DisplayMetadata {
                    prefix: "",
                    metadata: field.metadata().clone().into_iter().collect()
                },
            ))
        }
    });
    table.set_header(header);

    let displays = itertools::izip!(fields.iter(), columns.iter())
        .map(|(field, array)| custom_array_formatter(field, &**array))
        .collect::<Vec<_>>();
    let num_rows = columns.first().map_or(0, |list_array| list_array.len());

    if displays.is_empty() || num_rows == 0 {
        return table;
    }

    for row in 0..num_rows {
        let cells: Vec<_> = displays
            .iter()
            .map(|disp| match disp(row) {
                Ok(string) => {
                    let chars: Vec<_> = string.chars().collect();
                    if chars.len() > MAXIMUM_CELL_CONTENT_WIDTH as usize {
                        Cell::new(
                            chars
                                .into_iter()
                                .take(MAXIMUM_CELL_CONTENT_WIDTH.saturating_sub(1).into())
                                .chain(['…'])
                                .collect::<String>(),
                        )
                    } else {
                        Cell::new(string)
                    }
                }
                Err(err) => Cell::new(err),
            })
            .collect();
        table.add_row(cells);
    }

    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    // NOTE: `Percentage` only works for terminals that report their sizes.
    if table.width().is_some() {
        let percentage = comfy_table::Width::Percentage((100.0 / columns.len() as f32) as u16);
        table.set_constraints(
            std::iter::repeat(comfy_table::ColumnConstraint::UpperBoundary(percentage))
                .take(columns.len()),
        );
    }

    outer_table.add_row(vec![table.trim_fmt()]);
    outer_table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    outer_table.set_constraints(
        std::iter::repeat(comfy_table::ColumnConstraint::ContentWidth).take(columns.len()),
    );

    outer_table
}
