//! Formatting for tables of Arrow arrays

use std::{borrow::Borrow, fmt::Formatter};

use arrow2::{
    array::{get_display, Array, ListArray},
    datatypes::{DataType, Field, IntervalUnit, Metadata, TimeUnit},
};
use comfy_table::{presets, Cell, Row, Table};

use re_tuid::Tuid;
use re_types_core::Loggable as _;

// ---

// TODO(#1775): Registering custom formatters should be done from other crates:
// A) Because `re_format` cannot depend on other crates (cyclic deps)
// B) Because how to deserialize and inspect some type is a private implementation detail of that
//    type, re_format shouldn't know how to deserialize a TUID…

type CustomFormatter<'a, F> = Box<dyn Fn(&mut F, usize) -> std::fmt::Result + 'a>;

fn get_custom_display<'a, F: std::fmt::Write + 'a>(
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
        if name.as_str() == Tuid::NAME {
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

    let tuids = Tuid::from_arrow2(array.as_ref()).ok()?;
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
            DataType::Union(fields, _, _) => return write!(f, "union[{}]", fields.len()),
            DataType::Map(field, _) => return write!(f, "map[{}]", Self(field.data_type())),
            DataType::Dictionary(_, _, _) => "dict",
            DataType::Decimal(_, _) => "decimal",
            DataType::Decimal256(_, _) => "decimal256",
            DataType::Extension(name, _, _) => {
                return f.write_str(trim_name(name));
            }
        };
        f.write_str(s)
    }
}

struct DisplayMetadata<'a>(&'a Metadata, &'a str);

impl std::fmt::Display for DisplayMetadata<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self(metadata, prefix) = self;
        f.write_str(
            &metadata
                .iter()
                .map(|(key, value)| format!("{prefix}{}: {value:?}", trim_name(key)))
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

pub fn format_dataframe<F, C>(
    metadata: impl Borrow<Metadata>,
    fields: impl IntoIterator<Item = F>,
    columns: impl IntoIterator<Item = C>,
) -> Table
where
    F: Borrow<Field>,
    C: Borrow<dyn Array>,
{
    let metadata = metadata.borrow();

    let fields = fields.into_iter().collect::<Vec<_>>();
    let fields = fields
        .iter()
        .map(|field| field.borrow())
        .collect::<Vec<_>>();

    let columns = columns.into_iter().collect::<Vec<_>>();
    let columns = columns
        .iter()
        .map(|column| column.borrow())
        .collect::<Vec<_>>();

    const MAXIMUM_CELL_CONTENT_WIDTH: u16 = 100;

    let mut outer_table = Table::new();
    outer_table.load_preset(presets::UTF8_FULL);

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);

    outer_table.add_row({
        let mut row = Row::new();
        row.add_cell({
            let cell = Cell::new(format!(
                "CHUNK METADATA:\n{}",
                DisplayMetadata(metadata, "* ")
            ));

            #[cfg(not(target_arch = "wasm32"))] // requires TTY
            {
                cell.add_attribute(comfy_table::Attribute::Italic)
            }
            #[cfg(target_arch = "wasm32")]
            {
                cell
            }
        });
        row
    });

    let header = fields.iter().map(|field| {
        if field.metadata.is_empty() {
            Cell::new(format!(
                "{}\n---\ntype: \"{}\"", // NOLINT
                trim_name(&field.name),
                DisplayDatatype(field.data_type()),
            ))
        } else {
            Cell::new(format!(
                "{}\n---\ntype: \"{}\"\n{}", // NOLINT
                trim_name(&field.name),
                DisplayDatatype(field.data_type()),
                DisplayMetadata(&field.metadata, ""),
            ))
        }
    });
    table.set_header(header);

    let displays = columns
        .iter()
        .map(|array| get_custom_display(&**array, "-"))
        .collect::<Vec<_>>();
    let num_rows = columns.first().map_or(0, |list_array| list_array.len());

    if displays.is_empty() || num_rows == 0 {
        return table;
    }

    for row in 0..num_rows {
        let cells: Vec<_> = displays
            .iter()
            .map(|disp| {
                let mut string = String::new();
                if (disp)(&mut string, row).is_err() {
                    // Seems to be okay to silently ignore errors here, but reset the string just in case
                    string.clear();
                }
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
