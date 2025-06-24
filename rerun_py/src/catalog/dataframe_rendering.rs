use arrow::datatypes::Schema;
use arrow::pyarrow::FromPyArrow;
use arrow::record_batch::RecordBatch;
use comfy_table::Table;
use pyo3::{Bound, PyAny, PyResult, pyclass, pymethods};
use re_arrow_util::format_data_type;
use re_format_arrow::{RecordBatchFormatOpts, format_record_batch_opts};

#[pyclass(name = "RerunHtmlTable")]
#[derive(Clone)]
pub struct PyRerunHtmlTable {
    max_width: Option<usize>,
    max_height: Option<usize>,
}

impl PyRerunHtmlTable {
    fn build_table_container_start(&self) -> String {
        let max_width = self
            .max_width
            .map(|mw| format!("max-width: {}px;", mw))
            .unwrap_or_default();
        let max_height = self
            .max_height
            .map(|mh| format!("max-height: {}px;", mh))
            .unwrap_or_default();

        format!(
            r#"
            <div style="width: 100%; {} {} overflow: auto; border: 1px solid #ccc;">
            <table style="border-collapse: collapse; min-width: 100%">
            "#,
            max_width, max_height
        )
    }

    fn build_table_container_end(&self) -> String {
        r#"
        </table>
        </div>
        "#
        .to_string()
    }

    fn build_table_header(&self, schema: &Schema) -> String {
        let cells = schema
            .fields()
            .iter()
            .map(|field| {
                format!(
                    "<th style=\"font-weight: normal;\"><strong>{}</strong><br>{}</th>",
                    field.name(),
                    format_data_type(field.data_type())
                )
            })
            .collect::<Vec<String>>();

        format!("<thead><tr>{}</tr></thead>", cells.join(""))
    }

    fn build_table_body(&self, tables: Vec<Table>) -> Vec<String> {
        let mut results = vec!["<tbody".to_string()];

        for table in tables {
            let rows = table
                .row_iter()
                .map(|row| {
                    let cells = row
                        .cell_iter()
                        .map(|cell| format!("<td>{}</td>", cell.content().replace("\n", "<br>")))
                        .collect::<Vec<_>>()
                        .join("");

                    format!("<tr>{}</tr>\n", cells)
                })
                .collect::<Vec<_>>();
            results.extend(rows);
        }

        results.push("</tbody>".to_string());

        results
    }
}

#[pymethods]
impl PyRerunHtmlTable {
    #[new]
    #[pyo3(signature = (max_width=None, max_height=None))]
    fn new(max_width: Option<usize>, max_height: Option<usize>) -> Self {
        Self {
            max_height: max_height,
            max_width: max_width,
        }
    }

    fn format_html<'py>(
        &self,
        batches: Vec<Bound<'py, PyAny>>,
        schema: &Bound<'py, PyAny>,
        has_more: bool,
        table_uuid: &str,
    ) -> PyResult<String> {
        let batch_opts = RecordBatchFormatOpts::default();

        let tables = batches
            .into_iter()
            .map(|batch| RecordBatch::from_pyarrow_bound(&batch))
            .collect::<PyResult<Vec<RecordBatch>>>()?
            .into_iter()
            .map(|batch| format_record_batch_opts(&batch, &batch_opts))
            .filter(|table| !table.is_empty())
            .collect::<Vec<_>>();

        let schema = Schema::from_pyarrow_bound(schema)?;

        if tables.is_empty() {
            return Ok("No data to display.".to_string());
        }

        let mut html_result = Vec::default();

        html_result.push(self.build_table_container_start());
        html_result.push(self.build_table_header(&schema));
        html_result.extend(self.build_table_body(tables));
        html_result.push(self.build_table_container_end());

        if has_more {
            html_result.push("<div>Data truncated due to size.</div>".to_string());
        }

        Ok(html_result.join("\n"))
    }
}
