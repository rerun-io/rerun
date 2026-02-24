use std::collections::{HashMap, HashSet};

use arrow::array::AsArray as _;
use itertools::Itertools as _;

use re_log_encoding::{Decodable as _, RawRrdManifest, RrdFooter};
use re_log_types::LogMsg;
use re_sdk_types::reflection::{ComponentDescriptorExt as _, Reflection};

use crate::commands::{read_rrd_streams_from_file_or_stdin, stdio::InputSource};

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct VerifyCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// If true, ensures that RRD footers are present and well formed.
    //
    // TODO(cmc): consider making this mandatory?
    #[clap(long, default_value_t=true, num_args=0..=1)]
    check_footers: bool,
}

impl VerifyCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let mut verifier = Verifier::new()?;

        let Self {
            path_to_input_rrds,
            check_footers,
        } = self;

        let (rx, rx_done) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        let mut seen_files: HashMap<InputSource, RrdFooter> = HashMap::new();

        for (source, res) in rx {
            verifier.verify_log_msg(&source.to_string(), res?);
            seen_files.insert(source, RrdFooter::default());
        }

        if *check_footers {
            for (_, rrd_manifests) in rx_done {
                for (source, rrd_manifest) in rrd_manifests {
                    let rrd_manifest = match rrd_manifest {
                        Ok(m) => m,

                        Err(err) => {
                            verifier
                                .errors
                                .insert(format!("{source}: Corrupt RRD manifest: {err:#}"));
                            continue;
                        }
                    };

                    let rrd_footer = seen_files.entry(source.clone()).or_default();
                    if rrd_footer
                        .manifests
                        .insert(rrd_manifest.store_id.clone(), rrd_manifest.clone())
                        .is_some()
                    {
                        verifier.errors.insert(format!(
                            "{source}/{}/{}: More than one RRD manifest",
                            rrd_manifest.store_id.application_id(),
                            rrd_manifest.store_id.recording_id(),
                        ));
                    }

                    if let InputSource::File(path) = &source
                        && let Err(err) =
                            load_from_rrd_filepath_with_rrd_manifest(path, &rrd_manifest)
                    {
                        verifier.errors.insert(format!(
                            "{source}: Corrupt RRD manifest, cannot deserialize chunk data: {err:#}"
                        ));
                    }
                }
            }

            for (source, rrd_footer) in &seen_files {
                if rrd_footer.manifests.is_empty() {
                    verifier
                        .errors
                        .insert(format!("{source}: Missing RRD footer / no RRD manifests"));
                }
            }
        }

        if verifier.errors.is_empty() {
            if seen_files.len() == 1 {
                eprintln!("1 file verified without error.");
            } else {
                eprintln!("{} files verified without error.", seen_files.len());
            }
            Ok(())
        } else {
            for err in verifier.errors.iter().sorted() {
                eprintln!("{err}");
            }
            Err(anyhow::anyhow!(
                "Verification failed with {} errors",
                verifier.errors.len()
            ))
        }
    }
}

// ---

struct Verifier {
    reflection: Reflection,
    errors: HashSet<String>,
}

impl Verifier {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            reflection: re_sdk_types::reflection::generate_reflection()?,
            errors: HashSet::new(),
        })
    }

    fn verify_log_msg(&mut self, source: &str, msg: LogMsg) {
        match msg {
            LogMsg::SetStoreInfo { .. } | LogMsg::BlueprintActivationCommand { .. } => {}

            LogMsg::ArrowMsg(_store_id, arrow_msg) => {
                self.verify_record_batch(source, &arrow_msg.batch);
            }
        }
    }

    fn verify_record_batch(&mut self, source: &str, batch: &arrow::array::RecordBatch) {
        match re_sorbet::ChunkBatch::try_from(batch) {
            Ok(chunk_batch) => self.verify_chunk_batch(source, &chunk_batch),
            Err(err) => {
                self.errors
                    .insert(format!("{source}: Failed to parse batch: {err}"));
            }
        }
    }

    fn verify_chunk_batch(&mut self, source: &str, chunk_batch: &re_sorbet::ChunkBatch) {
        for (component_descriptor, column) in chunk_batch.component_columns() {
            if let Err(err) = self.verify_component_column(component_descriptor, column) {
                self.errors.insert(format!(
                    "{source}: Failed to deserialize column {}: {}. Column metadata: {:?}",
                    component_descriptor.column_name(re_sorbet::BatchType::Dataframe),
                    re_error::format(err),
                    chunk_batch.arrow_batch_metadata()
                ));
            }
        }
    }

    fn verify_component_column(
        &self,
        column_descriptor: &re_sorbet::ComponentColumnDescriptor,
        column: &dyn arrow::array::Array,
    ) -> anyhow::Result<()> {
        let re_sdk::ComponentDescriptor {
            component_type,
            archetype: archetype_name,
            component,
        } = column_descriptor.component_descriptor();

        let Some(component_type) = component_type else {
            re_log::debug_once!(
                "Encountered component descriptor without component type: '{}'",
                column_descriptor.component_descriptor()
            );
            return Ok(());
        };

        if !component_type.full_name().starts_with("rerun.") {
            re_log::debug_once!("Ignoring non-Rerun component {component_type:?}");
            return Ok(());
        }

        if component.starts_with("rerun.components.") && component.ends_with("Indicator") {
            // Lacks reflection and data
            anyhow::bail!(
                "Indicators are deprecated and should be removed on ingestion in re_sorbet."
            );
        } else {
            // Verify data
            let component_reflection = self
                .reflection
                .components
                .get(&component_type)
                .ok_or_else(|| anyhow::anyhow!("Unknown component"))?;

            if let Some(deprecation_summary) = component_reflection.deprecation_summary {
                anyhow::bail!(
                    "Component is deprecated. Deprecated types should be migrated on ingestion in re_sorbet. Deprecation notice: {deprecation_summary:?}"
                );
            }

            let list_array = column.as_list_opt::<i32>().ok_or_else(|| {
                anyhow::anyhow!("Expected list array, found {}", column.data_type())
            })?;

            assert_eq!(column.len() + 1, list_array.offsets().len());

            for i in 0..column.len() {
                let cell = list_array.value(i);
                (component_reflection.verify_arrow_array)(cell.as_ref())?;
            }
        }

        if let Some(archetype_name) = archetype_name {
            if archetype_name.full_name().starts_with("rerun.") {
                // Verify archetype.
                // We may want to have a flag to allow some of this?
                let archetype_reflection = self
                    .reflection
                    .archetypes
                    .get(&archetype_name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown archetype: {archetype_name:?}"))?;

                if let Some(deprecation_summary) = archetype_reflection.deprecation_summary {
                    anyhow::bail!(
                        "Archetype {archetype_name:?} is deprecated. Deprecated types should be migrated on ingestion in re_sorbet. Deprecation summary: {deprecation_summary:?}"
                    );
                }

                // Verify archetype field.
                // We may want to have a flag to allow some of this?
                let archetype_field_reflection = archetype_reflection
                    .field_by_name(column_descriptor.component_descriptor().archetype_field_name())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Input column referred to the component {component:?} of {archetype_name:?}, which only has the fields: {}",
                            archetype_reflection.fields.iter().map(|field| field.name).join(" ")
                        )
                    })?;

                let expected_component_type = &archetype_field_reflection.component_type;
                if &component_type != expected_component_type {
                    return Err(anyhow::anyhow!(
                        "Component {component:?} of {archetype_name:?} has type {expected_component_type:?} in this version of Rerun, but the data column has type {component_type:?}"
                    ));
                }
            } else {
                re_log::debug_once!("Ignoring non-Rerun archetype {archetype_name:?}");
            }
        }

        Ok(())
    }
}

// ---

/// Attempts to deserialize chunks according to the given RRD manifest.
///
/// Useful to check for corrupt manifests.
fn load_from_rrd_filepath_with_rrd_manifest(
    path_to_rrd: impl AsRef<std::path::Path>,
    rrd_manifest: &RawRrdManifest,
) -> anyhow::Result<()> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    let path_to_rrd = path_to_rrd.as_ref();

    re_tracing::profile_function!(path_to_rrd.to_string_lossy());

    let mut file = std::fs::File::open(path_to_rrd)?;

    let chunk_ids = rrd_manifest.col_chunk_id()?;
    let byte_offsets = rrd_manifest.col_chunk_byte_offset()?;
    let byte_sizes = rrd_manifest.col_chunk_byte_size()?;

    let mut buf = Vec::new();
    for (chunk_id, offset, size) in itertools::izip!(chunk_ids, byte_offsets, byte_sizes) {
        file.seek(SeekFrom::Start(offset))?;

        buf.resize(size as usize, 0u8);
        file.read_exact(&mut buf)?;

        let chunk = re_protos::log_msg::v1alpha1::ArrowMsg::from_rrd_bytes(&buf)?;
        anyhow::ensure!(
            chunk_id
                == re_chunk::ChunkId::from_tuid(
                    chunk
                        .chunk_id
                        .expect("must have chunk ID")
                        .try_into()
                        .expect("must be valid TUID")
                ),
            "corrupt RRD manifest, chunk IDs don't align"
        );
    }

    Ok(())
}
