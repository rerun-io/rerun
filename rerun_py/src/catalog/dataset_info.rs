use pyo3::pyclass;
use re_log_types::TimeType;
use re_redap_client::DatasetInfo;
use re_sorbet::BatchType;

/// `(name, kind, num_segments, min, max)`, with `min` and `max` in nanoseconds for temporal kinds.
type TimelineRow = (String, &'static str, u64, Option<i64>, Option<i64>);

/// Dataset overview transferred to the public `rerun.catalog.DatasetInfo`.
#[pyclass(
    frozen,
    get_all,
    name = "DatasetInfoInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyDatasetInfoInternal {
    dataset_name: String,
    num_segments: u64,
    size_bytes: u64,

    /// `(name, num_segments)`
    layers: Vec<(String, u64)>,

    timelines: Vec<TimelineRow>,

    num_entities: u64,
    num_component_columns: u64,

    /// Segment table column names.
    property_columns: Vec<String>,
}

impl PyDatasetInfoInternal {
    pub fn new(dataset_name: String, info: DatasetInfo) -> Self {
        let DatasetInfo {
            num_segments,
            size_bytes,
            layers,
            timelines,
            num_entities,
            num_component_columns,
            property_columns,
        } = info;

        Self {
            dataset_name,
            num_segments,
            size_bytes,
            layers: layers
                .into_iter()
                .map(|layer| (layer.name.into_string(), layer.num_segments))
                .collect(),
            timelines: timelines
                .into_iter()
                .map(|timeline| {
                    let kind = match timeline.typ {
                        TimeType::Sequence => "sequence",
                        TimeType::TimestampNs => "timestamp",
                        TimeType::DurationNs => "duration",
                    };
                    (
                        timeline.name.as_str().to_owned(),
                        kind,
                        timeline.num_segments,
                        timeline.range.map(|range| range.min().as_i64()),
                        timeline.range.map(|range| range.max().as_i64()),
                    )
                })
                .collect(),
            num_entities,
            num_component_columns,
            property_columns: property_columns
                .iter()
                .map(|column| column.column_name(BatchType::Dataframe))
                .collect(),
        }
    }
}
