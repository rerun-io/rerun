use std::collections::HashSet;

use arrow::array::AsArray as _;

use camino::Utf8PathBuf;
use itertools::Itertools as _;
use re_log_types::LogMsg;
use re_types::reflection::Reflection;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct VerifyCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<Utf8PathBuf>,
}

impl VerifyCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let mut verifier = Verifier::new()?;

        let Self { path_to_input_rrds } = self;

        let (rx, _) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        let mut seen_files = std::collections::HashSet::new();

        for (source, res) in rx {
            verifier.verify_log_msg(&source.to_string(), res?);
            seen_files.insert(source);
        }

        if verifier.errors.is_empty() {
            if seen_files.len() == 1 {
                eprintln!("1 file verified without error.");
            } else {
                eprintln!("{} files verified without error.", seen_files.len());
            }
            Ok(())
        } else {
            for err in &verifier.errors {
                eprintln!("{err}");
            }
            Err(anyhow::anyhow!(
                "Verification failed with {} errors",
                verifier.errors.len()
            ))
        }
    }
}

struct Verifier {
    reflection: Reflection,
    errors: HashSet<String>,
}

impl Verifier {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            reflection: re_types::reflection::generate_reflection()?,
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
                    "{source}: Failed to deserialize column {}: {}",
                    component_descriptor.component_name,
                    re_error::format(err)
                ));
            }
        }
    }

    fn verify_component_column(
        &self,
        component_descriptor: &re_sorbet::ComponentColumnDescriptor,
        column: &dyn arrow::array::Array,
    ) -> anyhow::Result<()> {
        let re_sorbet::ComponentColumnDescriptor {
            component_name,
            archetype_name,
            archetype_field_name,
            ..
        } = component_descriptor;

        if !component_name.full_name().starts_with("rerun.") {
            re_log::debug_once!("Ignoring non-Rerun component {component_name:?}");
            return Ok(());
        }

        if component_name.is_indicator_component() {
            // Lacks reflection and data
        } else {
            // Verify data
            let component_reflection = self
                .reflection
                .components
                .get(component_name)
                .ok_or_else(|| anyhow::anyhow!("Unknown component"))?;

            if let Some(deprecation_summary) = component_reflection.deprecation_summary {
                anyhow::bail!(
                    "Component is deprecated. Deprecated types should be migrated on ingestion in re_sorbet. Deprecation notice: {deprecation_summary:?}"
                );
            }

            let list_array = column.as_list_opt::<i32>().ok_or_else(|| {
                anyhow::anyhow!("Expected list array, found {:?}", column.data_type())
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
                    .get(archetype_name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown archetype: {archetype_name:?}"))?;

                if let Some(deprecation_summary) = archetype_reflection.deprecation_summary {
                    anyhow::bail!(
                        "Archetype {archetype_name:?} is deprecated. Deprecated types should be migrated on ingestion in re_sorbet. Deprecation summary: {deprecation_summary:?}"
                    );
                }

                if let Some(archetype_field_name) = archetype_field_name {
                    // Verify archetype field.
                    // We may want to have a flag to allow some of this?
                    let archetype_field_reflection = archetype_reflection
                        .get_field(archetype_field_name)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Input column referred to the archetype field name {archetype_field_name:?} of {archetype_name:?}, which only has the fields: {}",
                                archetype_reflection.fields.iter().map(|field| field.name).join(" ")
                            )
                        })?;

                    let expected_component_name = &archetype_field_reflection.component_name;
                    if component_name != expected_component_name {
                        return Err(anyhow::anyhow!(
                            "Archetype field {archetype_field_name:?} of {archetype_name:?} has component {expected_component_name:?} in this version of Rerun, but the data column has component {component_name:?}"
                        ));
                    }
                }
            } else {
                re_log::debug_once!("Ignoring non-Rerun archetype {archetype_name:?}");
            }
        }

        Ok(())
    }
}
