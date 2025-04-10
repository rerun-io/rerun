//! Formatting for tables of Arrow arrays

use std::fmt::Formatter;

use arrow::{
    array::{Array, ArrayRef, ListArray},
    datatypes::{DataType, Field, Fields, IntervalUnit, TimeUnit},
    util::display::{ArrayFormatter, FormatOptions},
};
use comfy_table::{presets, Cell, Row, Table};
use itertools::{Either, Itertools as _};

use re_arrow_util::ArrowArrayDowncastRef as _;
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
            // For example: `RowId` is a TUID that should be formatted with a `row_` prefix:
            let prefix = field
                .metadata()
                .get("ARROW:extension:metadata")
                .and_then(|metadata| serde_json::from_str::<Metadata>(metadata).ok())
                .and_then(|metadata| {
                    metadata
                        .get("namespace")
                        .map(|namespace| format!("{namespace}_"))
                })
                .unwrap_or_default();

            return Box::new(move |index| {
                if let Some(tuid) = parse_tuid(array, index) {
                    Ok(format!("{prefix}{tuid}"))
                } else {
                    Err("Invalid RowId".to_owned())
                }
            });
        }
    }

    match ArrayFormatter::try_new(array, &FormatOptions::default().with_null("null")) {
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
        DataType::List(_) => parse_inner(&array.downcast_array_ref::<ListArray>()?.value(index), 0),
        // New control columns: it's not a list to begin with!
        _ => parse_inner(array, index),
    }
}

// ---

// arrow has `ToString` implemented, but it is way too verbose.
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
            DataType::List(ref field) => {
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
                .collect_vec()
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

#[derive(Clone, Debug)]
pub struct RecordBatchFormatOpts {
    /// If `true`, the dataframe will be transposed on its diagonal axis.
    ///
    /// This is particularly useful for wide (i.e. lots of columns), short (i.e. not many rows) datasets.
    ///
    /// Setting this to `true` will also disable all per-column metadata (`include_column_metadata=false`).
    pub transposed: bool,

    /// If specified, displays the dataframe with the given fixed width.
    ///
    /// Defaults to the terminal width if left unspecified.
    pub width: Option<usize>,

    /// If `true`, displays the dataframe's metadata too.
    pub include_metadata: bool,

    /// If `true`, displays the individual columns' metadata too.
    pub include_column_metadata: bool,
}

impl Default for RecordBatchFormatOpts {
    fn default() -> Self {
        Self {
            transposed: false,
            width: None,
            include_metadata: true,
            include_column_metadata: true,
        }
    }
}

/// Nicely format this record batch in a way that fits the terminal.
pub fn format_record_batch(batch: &arrow::array::RecordBatch) -> Table {
    format_record_batch_with_width(batch, None)
}

/// Nicely format this record batch using the specified options.
pub fn format_record_batch_opts(
    batch: &arrow::array::RecordBatch,
    opts: &RecordBatchFormatOpts,
) -> Table {
    format_dataframe_with_metadata(
        &batch.schema_ref().metadata.clone().into_iter().collect(), // HashMap -> BTreeMap
        &batch.schema_ref().fields,
        batch.columns(),
        opts,
    )
}

/// Nicely format this record batch, either with the given fixed width, or with the terminal width (`None`).
///
/// If `transposed` is `true`, the dataframe will be printed transposed on its diagonal axis.
/// This is very useful for wide (i.e. lots of columns), short (i.e. not many rows) datasets.
pub fn format_record_batch_with_width(
    batch: &arrow::array::RecordBatch,
    width: Option<usize>,
) -> Table {
    format_dataframe_with_metadata(
        &batch.schema_ref().metadata.clone().into_iter().collect(), // HashMap -> BTreeMap
        &batch.schema_ref().fields,
        batch.columns(),
        &RecordBatchFormatOpts {
            transposed: false,
            width,
            include_metadata: true,
            include_column_metadata: true,
        },
    )
}

fn format_dataframe_with_metadata(
    metadata: &Metadata,
    fields: &Fields,
    columns: &[ArrayRef],
    opts: &RecordBatchFormatOpts,
) -> Table {
    let &RecordBatchFormatOpts {
        transposed: _,
        width,
        include_metadata,
        include_column_metadata: _,
    } = opts;

    let (num_columns, table) = format_dataframe_without_metadata(fields, columns, opts);

    if include_metadata && !metadata.is_empty() {
        let mut outer_table = Table::new();
        outer_table.load_preset(presets::UTF8_FULL);

        if let Some(width) = width {
            outer_table.set_width(width as _);
            outer_table.set_content_arrangement(comfy_table::ContentArrangement::Disabled);
        } else {
            outer_table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
        }

        outer_table.add_row({
            let mut row = Row::new();
            row.add_cell(Cell::new(format!(
                "METADATA:\n{}",
                DisplayMetadata {
                    prefix: "* ",
                    metadata: metadata.clone()
                }
            )));
            row
        });

        outer_table.add_row(vec![table.trim_fmt()]);
        outer_table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
        outer_table.set_constraints(
            std::iter::repeat(comfy_table::ColumnConstraint::ContentWidth).take(num_columns),
        );
        outer_table
    } else {
        table
    }
}

fn format_dataframe_without_metadata(
    fields: &Fields,
    columns: &[ArrayRef],
    opts: &RecordBatchFormatOpts,
) -> (usize, Table) {
    const MAXIMUM_CELL_CONTENT_WIDTH: u16 = 100;

    let &RecordBatchFormatOpts {
        transposed,
        width,
        include_metadata: _,
        include_column_metadata,
    } = opts;

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);

    if let Some(width) = width {
        table.set_width(width as _);
        table.set_content_arrangement(comfy_table::ContentArrangement::Disabled);
    } else {
        table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    }

    let formatters = itertools::izip!(fields.iter(), columns.iter())
        .map(|(field, array)| custom_array_formatter(field, &**array))
        .collect_vec();

    let num_columns = if transposed {
        // Turns:
        // ```
        // resource_id     manifest_url
        // -----------     --------------
        // resource_1      resource_1_url
        // resource_2      resource_2_url
        // resource_3      resource_3_url
        // resource_4      resource_4_url
        // ```
        // into:
        // ```
        // resource_id       resource_1         resource_2         resource_3         resource_4
        // manifest_url      resource_1_url     resource_2_url     resource_3_url     resource_4_url
        // ```

        let mut headers = fields
            .iter()
            .map(|field| Cell::new(trim_name(field.name())))
            .collect_vec();
        headers.reverse();

        let mut columns = columns.to_vec();
        columns.reverse();

        for formatter in formatters {
            let mut cells = headers.pop().into_iter().collect_vec();

            let Some(col) = columns.pop() else {
                break;
            };

            for i in 0..col.len() {
                let cell = match formatter(i) {
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
                };
                cells.push(cell);
            }

            table.add_row(cells);
        }

        columns.first().map_or(0, |list_array| list_array.len())
    } else {
        let header = if include_column_metadata {
            Either::Left(fields.iter().map(|field| {
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
            }))
        } else {
            Either::Right(
                fields
                    .iter()
                    .map(|field| Cell::new(trim_name(field.name()).to_owned())),
            )
        };

        table.set_header(header);

        let num_rows = columns.first().map_or(0, |list_array| list_array.len());

        for row in 0..num_rows {
            let cells: Vec<_> = formatters
                .iter()
                .map(|formatter| match formatter(row) {
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

        columns.len()
    };

    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    // NOTE: `Percentage` only works for terminals that report their sizes.
    if table.width().is_some() {
        let percentage = comfy_table::Width::Percentage((100.0 / num_columns as f32) as u16);
        table.set_constraints(
            std::iter::repeat(comfy_table::ColumnConstraint::UpperBoundary(percentage))
                .take(num_columns),
        );
    }

    (num_columns, table)
}
