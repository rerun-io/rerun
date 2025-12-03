//! Formatting for tables of Arrow arrays

use std::fmt::Formatter;

use arrow::array::{Array, ArrayRef, AsArray as _, ListArray};
use arrow::datatypes::{DataType, Field, Fields};
use arrow::util::display::{ArrayFormatter, FormatOptions};
use comfy_table::{Cell, Row, Table, presets};
use itertools::{Either, Itertools as _};
use re_tuid::Tuid;

use crate::{ArrowArrayDowncastRef as _, format_field_datatype};

// ---

// TODO(#1775): Registering custom formatters should be done from other crates:
// A) Because `re_format` cannot depend on other crates (cyclic deps)
// B) Because how to deserialize and inspect some type is a private implementation detail of that
//    type, re_format shouldn't know how to deserialize a TUID…

/// Format the given row as a string
type CustomArrayFormatter<'a> = Box<dyn Fn(usize) -> Result<String, String> + 'a>;

/// This is a `BTreeMap`, and not a `HashMap`, because we want a predictable order.
type Metadata = std::collections::BTreeMap<String, String>;

/// The replacement string for non-deterministic values.
const REDACT_STRING: &str = "[**REDACTED**]";

/// Metadata fields that are non-deterministic.
const NON_DETERMINISTIC_METADATA: &[&str] =
    &["rerun:id", "rerun:heap_size_bytes", "sorbet:version"];

fn custom_array_formatter<'a>(
    field: &Field,
    array: &'a dyn Array,
    redact_non_deterministic: bool,
) -> CustomArrayFormatter<'a> {
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
                    if redact_non_deterministic {
                        Ok(format!("{prefix}{REDACT_STRING}"))
                    } else {
                        Ok(format!("{prefix}{tuid}"))
                    }
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
        let array = array.as_fixed_size_binary_opt()?;
        let tuids = Tuid::slice_from_bytes(array.value_data()).ok()?;
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

struct DisplayMetadata {
    prefix: &'static str,
    metadata: Metadata,
    trim_keys: bool,
    trim_values: bool,
    redact_non_deterministic: bool,
}

impl std::fmt::Display for DisplayMetadata {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            prefix,
            metadata,
            trim_keys,
            trim_values,
            redact_non_deterministic,
        } = self;
        f.write_str(
            &metadata
                .iter()
                .map(|(key, value)| {
                    let needs_redact = *redact_non_deterministic
                        && NON_DETERMINISTIC_METADATA.contains(&key.as_str());

                    let key = if *trim_keys { trim_key(key) } else { key };
                    let value = match (needs_redact, *trim_values) {
                        (true, _) => REDACT_STRING,
                        (false, true) => trim_name(value),
                        (false, false) => value,
                    };
                    format!("{prefix}{key}: {value}",)
                })
                .collect_vec()
                .join("\n"),
        )
    }
}

fn trim_key(name: &str) -> &str {
    name.trim()
        .trim_start_matches("rerun:")
        .trim_start_matches("sorbet:")
}

fn trim_name(name: &str) -> &str {
    name.trim()
        .trim_start_matches("rerun.archetypes.")
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

    /// Don't print more rows than this.
    pub max_rows: usize,

    /// Individual cells will never exceed this size.
    ///
    /// When they do, their contents will be truncated and replaced with ellipses instead.
    pub max_cell_content_width: usize,

    /// If `true`, displays the dataframe's metadata too.
    pub include_metadata: bool,

    /// If `true`, displays the individual columns' metadata too.
    pub include_column_metadata: bool,

    /// If `true`, trims the Rerun prefixes from field names.
    pub trim_field_names: bool,

    /// If `true`, trims the `rerun:` prefix from metadata values.
    pub trim_metadata_keys: bool,

    /// If `true`, trims known Rerun prefixes from metadata values.
    pub trim_metadata_values: bool,

    /// If `true`, redacts known non-deterministic values.
    pub redact_non_deterministic: bool,
}

impl Default for RecordBatchFormatOpts {
    fn default() -> Self {
        Self {
            transposed: false,
            width: None,
            max_rows: usize::MAX,
            max_cell_content_width: 100,
            include_metadata: true,
            include_column_metadata: true,
            trim_field_names: true,
            trim_metadata_keys: true,
            trim_metadata_values: true,
            redact_non_deterministic: false,
        }
    }
}

impl RecordBatchFormatOpts {
    /// Nicely format this record batch using the specified options.
    pub fn format(&self, batch: &arrow::array::RecordBatch) -> Table {
        format_record_batch_opts(batch, self)
    }
}

/// Nicely format this record batch in a way that fits the terminal.
#[must_use = "this merely formats, you need to print it yourself"]
pub fn format_record_batch(batch: &arrow::array::RecordBatch) -> Table {
    format_record_batch_with_width(batch, None, false)
}

/// Nicely format this record batch using the specified options.
#[must_use = "this merely formats, you need to print it yourself"]
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
#[must_use = "this merely formats, you need to print it yourself"]
pub fn format_record_batch_with_width(
    batch: &arrow::array::RecordBatch,
    width: Option<usize>,
    redact_non_deterministic: bool,
) -> Table {
    format_dataframe_with_metadata(
        &batch.schema_ref().metadata.clone().into_iter().collect(), // HashMap -> BTreeMap
        &batch.schema_ref().fields,
        batch.columns(),
        &RecordBatchFormatOpts {
            width,
            redact_non_deterministic,
            ..Default::default()
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
        max_rows: _,
        max_cell_content_width: _,
        include_metadata,
        include_column_metadata: _,
        trim_field_names: _, // passed as part of `opts` below
        trim_metadata_keys: trim_keys,
        trim_metadata_values: trim_values,
        redact_non_deterministic,
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
                    metadata: metadata.clone(),
                    trim_keys,
                    trim_values,
                    redact_non_deterministic,
                }
            )));
            row
        });

        outer_table.add_row(vec![table.trim_fmt()]);
        outer_table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
        outer_table.set_constraints(std::iter::repeat_n(
            comfy_table::ColumnConstraint::ContentWidth,
            num_columns,
        ));
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
    let &RecordBatchFormatOpts {
        transposed,
        width,
        max_rows,
        max_cell_content_width,
        include_metadata: _,
        include_column_metadata,
        trim_field_names,
        trim_metadata_keys: trim_keys,
        trim_metadata_values: trim_values,
        redact_non_deterministic,
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
        .map(|(field, array)| custom_array_formatter(field, &**array, redact_non_deterministic))
        .collect_vec();

    let total_rows = columns.first().map_or(0, |list_array| list_array.len());
    let num_rows_shown = usize::min(total_rows, max_rows);
    let hidden_rows = total_rows - num_rows_shown;

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
            .map(|field| {
                let name = if trim_field_names {
                    trim_name(field.name())
                } else {
                    field.name()
                };
                Cell::new(name)
            })
            .collect_vec();
        headers.reverse();

        let mut columns = columns.to_vec();
        columns.reverse();

        for formatter in formatters {
            let mut cells = headers.pop().into_iter().collect_vec();

            for i in 0..num_rows_shown {
                let cell = match formatter(i) {
                    Ok(string) => format_cell(string, max_cell_content_width),
                    Err(err) => Cell::new(err),
                };
                cells.push(cell);
            }
            if 0 < hidden_rows {
                cells.push(Cell::new(format!("… + {hidden_rows} more")));
            }

            table.add_row(cells);
        }

        columns.first().map_or(0, |list_array| list_array.len())
    } else {
        let header = if include_column_metadata {
            Either::Left(fields.iter().map(|field| {
                if field.metadata().is_empty() {
                    Cell::new(format!(
                        "{}\n---\ntype: {}",
                        if trim_field_names {
                            trim_name(field.name())
                        } else {
                            field.name()
                        },
                        format_field_datatype(field),
                    ))
                } else {
                    Cell::new(format!(
                        "{}\n---\ntype: {}\n{}",
                        if trim_field_names {
                            trim_name(field.name())
                        } else {
                            field.name()
                        },
                        format_field_datatype(field),
                        DisplayMetadata {
                            prefix: "",
                            metadata: field.metadata().clone().into_iter().collect(),
                            trim_keys,
                            trim_values,
                            redact_non_deterministic,
                        },
                    ))
                }
            }))
        } else {
            Either::Right(fields.iter().map(|field| {
                let name = if trim_field_names {
                    trim_name(field.name())
                } else {
                    field.name()
                };
                Cell::new(name.to_owned())
            }))
        };

        table.set_header(header);

        for row in 0..num_rows_shown {
            let cells: Vec<_> = formatters
                .iter()
                .map(|formatter| match formatter(row) {
                    Ok(string) => format_cell(string, max_cell_content_width),
                    Err(err) => Cell::new(err),
                })
                .collect();
            table.add_row(cells);
        }

        if 0 < hidden_rows {
            table.add_row([format!("… + {hidden_rows} more row(s)")]);
        }

        columns.len()
    };

    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    // NOTE: `Percentage` only works for terminals that report their sizes.
    if table.width().is_some() {
        let percentage = comfy_table::Width::Percentage((100.0 / num_columns as f32) as u16);
        table.set_constraints(std::iter::repeat_n(
            comfy_table::ColumnConstraint::UpperBoundary(percentage),
            num_columns,
        ));
    }

    (num_columns, table)
}

fn format_cell(string: String, max_cell_content_width: usize) -> Cell {
    let chars: Vec<_> = string.chars().collect();
    if chars.len() > max_cell_content_width {
        Cell::new(
            chars
                .into_iter()
                .take(max_cell_content_width.saturating_sub(1))
                .chain(['…'])
                .collect::<String>(),
        )
    } else {
        Cell::new(string)
    }
}
